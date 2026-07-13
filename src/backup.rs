//! Database + uploaded-file backups, run in-process so an admin can monitor them
//! from the webapp.
//!
//! The DB snapshot uses SQLite's `VACUUM INTO`, which produces a single,
//! transactionally-consistent copy while the server keeps serving — no external
//! `sqlite3` binary and WAL-safe (unlike copying `db.sqlite`, whose latest
//! commits live in the `-wal` sibling). The snapshot plus the `images/` and
//! `documents/` upload dirs (which DB rows reference) are bundled into one
//! `.tar.gz` under `data/backups/`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, SqlitePool};

use crate::config::Config;
use crate::error::{AppError, AppResult};

/// Serializes backup runs so a scheduled run and a manual "Back up now" can't
/// overlap (two `VACUUM INTO`s racing, double disk use). Held across the whole
/// run; callers acquire it with `try_lock` and treat contention as "busy".
pub type BackupGuard = Arc<tokio::sync::Mutex<()>>;

/// Floor between scheduler wake-ups. Stops a hot loop when runs keep failing
/// (last success stays old, so the next run is perpetually "due") or when the
/// lock was busy.
const MIN_POLL: Duration = Duration::from_secs(300);

/// Run a full backup: snapshot → verify → archive → prune. Records the attempt
/// in the `backups` table (a `running` row up front, updated on completion) so
/// failures are visible to admins. Returns the new row id.
///
/// The caller must already hold the [`BackupGuard`].
pub async fn perform_backup(pool: &SqlitePool, config: &Config, trigger: &str) -> AppResult<i64> {
    let started = Utc::now();
    let id = sqlx::query("INSERT INTO backups (startedAt, status, trigger) VALUES (?, 'running', ?)")
        .bind(started.to_rfc3339())
        .bind(trigger)
        .execute(pool)
        .await?
        .last_insert_rowid();

    match run_inner(pool, config, &started.format("%Y%m%d-%H%M%S").to_string()).await {
        Ok((file_name, size)) => {
            sqlx::query(
                "UPDATE backups SET status='success', finishedAt=?, sizeBytes=?, filePath=? WHERE id=?",
            )
            .bind(Utc::now().to_rfc3339())
            .bind(size as i64)
            .bind(&file_name)
            .bind(id)
            .execute(pool)
            .await?;
            tracing::info!("Backup #{id} succeeded: {file_name} ({size} bytes)");
            Ok(id)
        }
        Err(e) => {
            let msg = e.to_string();
            // Best-effort: if even this UPDATE fails the startup sweep will later
            // reset the stranded `running` row.
            let _ = sqlx::query("UPDATE backups SET status='failed', finishedAt=?, error=? WHERE id=?")
                .bind(Utc::now().to_rfc3339())
                .bind(&msg)
                .bind(id)
                .execute(pool)
                .await;
            tracing::error!("Backup #{id} failed: {msg}");
            Err(e)
        }
    }
}

async fn run_inner(pool: &SqlitePool, config: &Config, stamp: &str) -> AppResult<(String, u64)> {
    let dir = config.backup_dir();
    tokio::fs::create_dir_all(&dir).await?;

    let snapshot = dir.join(format!(".snapshot-{stamp}.sqlite"));
    let file_name = format!("motomanager-{stamp}.tar.gz");
    let archive = dir.join(&file_name);

    // 1. Consistent online snapshot. The timestamped name is a fresh path, which
    //    VACUUM INTO requires (it refuses to overwrite an existing file). The
    //    path can't be a bind parameter here (VACUUM takes a literal), so single
    //    quotes are doubled and the string is asserted safe — the value is
    //    server-generated (backup dir + timestamp), never user input.
    let target = snapshot.to_string_lossy().replace('\'', "''");
    sqlx::query(sqlx::AssertSqlSafe(format!("VACUUM INTO '{target}'")))
        .execute(pool)
        .await?;

    // 2. Verify before trusting it — a backup that won't open is worse than none.
    verify_snapshot(&snapshot).await?;

    // 3. Bundle snapshot + upload dirs. Compression/IO is blocking → off-thread.
    let images = config.images_dir();
    let documents = config.documents_dir();
    let (snap, arch) = (snapshot.clone(), archive.clone());
    let size = tokio::task::spawn_blocking(move || build_archive(&arch, &snap, &images, &documents))
        .await
        .map_err(|e| AppError::Internal(format!("backup archive task panicked: {e}")))?
        .map_err(|e| AppError::Internal(format!("failed to write backup archive: {e}")))?;

    // 4. The archive holds a copy of the snapshot; drop the transient file.
    let _ = tokio::fs::remove_file(&snapshot).await;

    // 5. Retention.
    prune_old_archives(&dir, config.backup_keep);

    Ok((file_name, size))
}

/// Open the snapshot read-only on its own connection and run `PRAGMA
/// integrity_check`. A corrupt snapshot is deleted and reported as an error.
async fn verify_snapshot(path: &Path) -> AppResult<()> {
    let opts = SqliteConnectOptions::new().filename(path).read_only(true);
    let mut conn = sqlx::sqlite::SqliteConnection::connect_with(&opts)
        .await
        .map_err(|e| AppError::Internal(format!("cannot open snapshot to verify: {e}")))?;

    let result: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut conn)
        .await
        .map_err(|e| AppError::Internal(format!("integrity_check failed to run: {e}")))?;
    let _ = conn.close().await;

    if result != "ok" {
        let _ = tokio::fs::remove_file(path).await;
        return Err(AppError::Internal(format!(
            "snapshot integrity_check reported: {result}"
        )));
    }
    Ok(())
}

/// Synchronous tar+gzip build. `images`/`documents` are stored under their own
/// top-level dirs; a missing upload dir (fresh install) is simply skipped.
fn build_archive(
    archive: &Path,
    snapshot: &Path,
    images: &Path,
    documents: &Path,
) -> std::io::Result<u64> {
    let file = std::fs::File::create(archive)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut tar = tar::Builder::new(encoder);

    tar.append_path_with_name(snapshot, "db.sqlite")?;
    if images.is_dir() {
        tar.append_dir_all("images", images)?;
    }
    if documents.is_dir() {
        tar.append_dir_all("documents", documents)?;
    }

    let file = tar.into_inner()?.finish()?;
    file.sync_all()?;
    Ok(file.metadata()?.len())
}

/// Keep the newest `keep` archives, delete the rest. Timestamped names sort
/// lexicographically in chronological order.
fn prune_old_archives(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut archives: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("motomanager-") && n.ends_with(".tar.gz"))
        })
        .collect();
    archives.sort();

    if archives.len() > keep {
        for path in &archives[..archives.len() - keep] {
            if let Err(e) = std::fs::remove_file(path) {
                tracing::warn!("Failed to prune old backup {}: {e}", path.display());
            }
        }
    }
}

/// Seconds until the next scheduled backup is due, based on the last *successful*
/// run. `0` means overdue (or none yet). Basing this on success — not attempts —
/// means a failed run stays due and is retried on the next poll.
async fn seconds_until_due(pool: &SqlitePool, interval: Duration) -> u64 {
    let last: Option<String> = sqlx::query_scalar(
        "SELECT finishedAt FROM backups \
         WHERE status='success' AND finishedAt IS NOT NULL \
         ORDER BY finishedAt DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let Some(last) = last else {
        return 0;
    };
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&last) else {
        return 0;
    };
    let elapsed = Utc::now()
        .signed_duration_since(ts.with_timezone(&Utc))
        .num_seconds();
    (interval.as_secs() as i64 - elapsed).max(0) as u64
}

/// Background loop that fires a backup every `backup_interval_hours`. Survives
/// restarts gracefully: schedule is derived from the last success in the DB, so
/// a redeploy won't re-snapshot if one just ran, and a long downtime triggers a
/// catch-up backup shortly after boot.
pub fn spawn_scheduler(pool: SqlitePool, config: Config, lock: BackupGuard) {
    let interval = Duration::from_secs(config.backup_interval_hours * 3600);
    tokio::spawn(async move {
        tracing::info!(
            "Backup scheduler running (every {}h, keep {})",
            config.backup_interval_hours,
            config.backup_keep
        );
        // Let a fresh boot/deploy settle before the first (possibly catch-up) run.
        tokio::time::sleep(Duration::from_secs(60)).await;

        loop {
            let due = seconds_until_due(&pool, interval).await;
            if due > 0 {
                tokio::time::sleep(Duration::from_secs(due)).await;
            }

            if let Ok(_guard) = lock.try_lock() {
                if let Err(e) = perform_backup(&pool, &config, "scheduled").await {
                    tracing::error!("Scheduled backup failed: {e}");
                }
            } else {
                tracing::debug!("Skipping scheduled backup — a backup is already running");
            }

            // Always pause before re-evaluating so a failing/skipped run can't
            // spin (it would otherwise remain immediately "due").
            tokio::time::sleep(MIN_POLL).await;
        }
    });
}

/// Reset rows left `running` by a previous process that crashed mid-backup, so
/// the UI doesn't show a phantom in-progress run forever. Call once at startup.
pub async fn reset_stale_running(pool: &SqlitePool) -> AppResult<()> {
    let affected = sqlx::query(
        "UPDATE backups SET status='failed', finishedAt=?, error='interrupted (server restarted)' \
         WHERE status='running'",
    )
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?
    .rows_affected();
    if affected > 0 {
        tracing::warn!("Reset {affected} interrupted backup run(s) from a previous process");
    }
    Ok(())
}
