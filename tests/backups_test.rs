//! End-to-end exercise of the in-process backup pipeline: VACUUM INTO snapshot
//! → integrity check → tar.gz of DB + upload dirs → history row. Also proves the
//! snapshot captured committed data by re-opening the archived db.sqlite.

use std::io::Read;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use moto_manager_api::{backup::perform_backup, config::Config};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use sqlx::ConnectOptions;

fn test_config(data_dir: &std::path::Path) -> Config {
    Config {
        database_url: "sqlite::memory:".to_string(),
        port: 3001,
        rp_id: "localhost".to_string(),
        rp_name: "Test".to_string(),
        origin: "http://localhost:5173".to_string(),
        enable_registration: true,
        app_version: "backend-1.2.3".to_string(),
        data_dir: data_dir.to_string_lossy().to_string(),
        cache_dir: data_dir.join("cache").to_string_lossy().to_string(),
        llm_base_url: None,
        llm_model: "test".to_string(),
        llm_api_key: "test".to_string(),
        backup_enabled: false,
        backup_interval_hours: 24,
        backup_keep: 2,
        frontend_version: Some("frontend-9.9.9".to_string()),
    }
}

/// Unique temp dir per test run (no shared state / collisions between tests).
fn temp_data_dir(tag: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mm_backup_test_{tag}_{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A file-backed WAL pool mirroring production. `VACUUM INTO` is a no-op on an
/// in-memory source, so backups must be exercised against a real DB file.
async fn make_pool(data_dir: &std::path::Path) -> SqlitePool {
    let db_path = data_dir.join("db.sqlite");
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.to_string_lossy()))
        .unwrap()
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn backup_snapshots_db_and_files() {
    let data_dir = temp_data_dir("full");
    let config = test_config(&data_dir);

    // File-backed DB with a marker row to prove the snapshot captured committed
    // data.
    let pool = make_pool(&data_dir).await;
    sqlx::query("INSERT INTO currencies (code, symbol, conversionFactor, createdAt) VALUES ('ZZZ', 'Z', 1.0, '2026-01-01')")
        .execute(&pool)
        .await
        .unwrap();

    // Uploaded files the DB references and the backup must bundle.
    std::fs::create_dir_all(config.images_dir()).unwrap();
    std::fs::create_dir_all(config.documents_dir()).unwrap();
    std::fs::write(config.images_dir().join("bike.jpg"), b"fake-image").unwrap();
    std::fs::write(config.documents_dir().join("invoice.pdf"), b"fake-pdf").unwrap();

    // Run the real pipeline.
    let id = perform_backup(&pool, &config, "manual", Some("frontend-from-client".to_string()))
        .await
        .expect("backup should succeed");

    // History row is marked success with size, file path, and versions. The
    // client-supplied frontend version wins over the FRONTEND_VERSION config.
    let (status, size, file_path, backend_v, frontend_v): (
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT status, sizeBytes, filePath, backendVersion, frontendVersion FROM backups WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "success");
    assert!(size.unwrap() > 0, "archive should be non-empty");
    assert_eq!(backend_v.as_deref(), Some("backend-1.2.3"));
    assert_eq!(frontend_v.as_deref(), Some("frontend-from-client"));
    let file_path = file_path.expect("filePath recorded");

    // Archive exists on disk, transient snapshot cleaned up.
    let archive = config.backup_dir().join(&file_path);
    assert!(archive.exists(), "archive file should exist");
    let leftover_snapshots = std::fs::read_dir(config.backup_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".snapshot-"))
        .count();
    assert_eq!(leftover_snapshots, 0, "transient snapshot should be removed");

    // Archive contains the DB snapshot, the manifest, and both upload dirs.
    let mut entries = archive_entry_names(&archive);
    entries.sort();
    assert!(entries.iter().any(|n| n == "db.sqlite"), "entries: {entries:?}");
    assert!(entries.iter().any(|n| n == "manifest.json"), "entries: {entries:?}");
    assert!(entries.iter().any(|n| n.contains("images/bike.jpg")), "entries: {entries:?}");
    assert!(entries.iter().any(|n| n.contains("documents/invoice.pdf")), "entries: {entries:?}");

    // manifest.json documents the versions.
    let manifest: serde_json::Value =
        serde_json::from_slice(&extract_entry(&archive, "manifest.json")).unwrap();
    assert_eq!(manifest["backendVersion"], "backend-1.2.3");
    assert_eq!(manifest["frontendVersion"], "frontend-from-client");
    assert!(manifest["createdAt"].is_string());

    // Extract db.sqlite and confirm the committed marker row is present — proves
    // VACUUM INTO produced a real, consistent snapshot (not an empty file).
    let db_bytes = extract_entry(&archive, "db.sqlite");
    let snap_path = data_dir.join("extracted.sqlite");
    std::fs::write(&snap_path, &db_bytes).unwrap();
    let mut conn = SqliteConnectOptions::new()
        .filename(&snap_path)
        .read_only(true)
        .connect()
        .await
        .unwrap();
    let code: String = sqlx::query_scalar("SELECT code FROM currencies WHERE code = 'ZZZ'")
        .fetch_one(&mut conn)
        .await
        .expect("marker row must be in the snapshot");
    assert_eq!(code, "ZZZ");

    std::fs::remove_dir_all(&data_dir).ok();
}

#[tokio::test]
async fn backup_prunes_to_retention_limit() {
    let data_dir = temp_data_dir("prune");
    let config = test_config(&data_dir); // backup_keep = 2
    let pool = make_pool(&data_dir).await;

    // Three runs; retention should leave only the newest two archives. Timestamps
    // are second-resolution, so space the runs out to get distinct filenames.
    for _ in 0..3 {
        perform_backup(&pool, &config, "scheduled", None)
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }

    let archives = std::fs::read_dir(config.backup_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.starts_with("motomanager-") && n.ends_with(".tar.gz")
        })
        .count();
    assert_eq!(archives, 2, "retention should keep only backup_keep archives");

    std::fs::remove_dir_all(&data_dir).ok();
}

fn archive_entry_names(archive: &std::path::Path) -> Vec<String> {
    let file = std::fs::File::open(archive).unwrap();
    let mut tar = tar::Archive::new(GzDecoder::new(file));
    tar.entries()
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path().unwrap().to_string_lossy().to_string())
        .collect()
}

fn extract_entry(archive: &std::path::Path, name: &str) -> Vec<u8> {
    let file = std::fs::File::open(archive).unwrap();
    let mut tar = tar::Archive::new(GzDecoder::new(file));
    for entry in tar.entries().unwrap() {
        let mut entry = entry.unwrap();
        if entry.path().unwrap().to_string_lossy() == name {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).unwrap();
            return buf;
        }
    }
    panic!("entry {name} not found in archive");
}
