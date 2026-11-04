//! SEC-02: session tokens are stored as SHA-256 hashes, never in plaintext.

use moto_manager_api::auth::session::{
    create_session, delete_session, generate_session_token, get_user_from_token,
    hash_legacy_session_tokens, hash_session_token,
};
use sqlx::sqlite::SqlitePoolOptions;

async fn pool_with_user() -> (sqlx::SqlitePool, i64) {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let user_id = sqlx::query(
        "INSERT INTO users (email, username, name, passwordHash, role) VALUES ('a@x.ch', 'a', 'A', 'x', 'user')",
    )
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    (pool, user_id)
}

#[tokio::test]
async fn sessions_store_only_the_token_hash() {
    let (pool, user_id) = pool_with_user().await;
    let token = create_session(&pool, user_id).await.unwrap();

    let stored: Vec<String> = sqlx::query_scalar("SELECT token FROM sessions")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(stored, vec![hash_session_token(&token)]);
    assert!(!stored.contains(&token), "raw token must not be stored");

    // The raw token still authenticates; the stored hash does not.
    assert_eq!(
        get_user_from_token(&pool, &token).await.unwrap().id,
        user_id
    );
    assert!(get_user_from_token(&pool, &stored[0]).await.is_err());

    delete_session(&pool, &token).await.unwrap();
    assert!(get_user_from_token(&pool, &token).await.is_err());
}

#[tokio::test]
async fn legacy_plaintext_sessions_are_hashed_in_place() {
    let (pool, user_id) = pool_with_user().await;
    let legacy = generate_session_token();
    sqlx::query(
        "INSERT INTO sessions (token, userId, expiresAt, createdAt) VALUES (?, ?, '2999-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
    )
    .bind(&legacy)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    // A modern (already hashed) session must be left alone.
    let modern = create_session(&pool, user_id).await.unwrap();

    assert_eq!(hash_legacy_session_tokens(&pool).await.unwrap(), 1);
    assert_eq!(hash_legacy_session_tokens(&pool).await.unwrap(), 0);

    let plaintext_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE token = ?")
        .bind(&legacy)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(plaintext_rows, 0);
    // Both logins keep working.
    assert_eq!(
        get_user_from_token(&pool, &legacy).await.unwrap().id,
        user_id
    );
    assert_eq!(
        get_user_from_token(&pool, &modern).await.unwrap().id,
        user_id
    );
}
