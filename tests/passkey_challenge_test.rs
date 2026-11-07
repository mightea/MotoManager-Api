//! Passkey challenges are single-use, expire, and are bound to their purpose.

use chrono::{Duration, Utc};
use moto_manager_api::{error::AppError, handlers::passkey::take_challenge};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn setup() -> (SqlitePool, i64) {
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

async fn insert(pool: &SqlitePool, id: &str, user_id: Option<i64>, expires_in: Duration) {
    sqlx::query("INSERT INTO challenges (id, userId, challenge, expiresAt) VALUES (?, ?, '{}', ?)")
        .bind(id)
        .bind(user_id)
        .bind((Utc::now() + expires_in).to_rfc3339())
        .execute(pool)
        .await
        .unwrap();
}

fn message(err: AppError) -> String {
    match err {
        AppError::BadRequest(msg) => msg,
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

#[tokio::test]
async fn login_challenge_can_be_used_only_once() {
    let (pool, _) = setup().await;
    insert(&pool, "c1", None, Duration::minutes(5)).await;

    assert_eq!(take_challenge(&pool, "c1", None).await.unwrap().id, "c1");
    let again = take_challenge(&pool, "c1", None).await.unwrap_err();
    assert_eq!(message(again), "Challenge not found");
}

#[tokio::test]
async fn expired_challenge_is_rejected_and_removed() {
    let (pool, _) = setup().await;
    insert(&pool, "old", None, Duration::minutes(-1)).await;

    let err = take_challenge(&pool, "old", None).await.unwrap_err();
    assert_eq!(message(err), "Challenge expired");
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM challenges")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

#[tokio::test]
async fn challenges_are_bound_to_their_user() {
    let (pool, user_id) = setup().await;
    insert(&pool, "reg", Some(user_id), Duration::minutes(5)).await;
    insert(&pool, "login", None, Duration::minutes(5)).await;

    // A login cannot claim a registration challenge, nor another user's.
    assert!(take_challenge(&pool, "reg", None).await.is_err());
    assert!(take_challenge(&pool, "reg", Some(user_id + 1))
        .await
        .is_err());
    // A registration cannot claim a login challenge.
    assert!(take_challenge(&pool, "login", Some(user_id)).await.is_err());
    // The rightful owners still can.
    assert!(take_challenge(&pool, "reg", Some(user_id)).await.is_ok());
    assert!(take_challenge(&pool, "login", None).await.is_ok());
}
