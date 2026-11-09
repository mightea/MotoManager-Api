//! Exchange rates for the admin-managed currencies.
//!
//! `currencies.conversionFactor` is **CHF per one unit of the currency**: every
//! consumer multiplies a foreign amount by it to get CHF (`normalizedPrice`,
//! `normalizedCost`, fleet stats, the MCP tools, the webapp). Frankfurter quotes
//! `from=CHF` as units of the currency per CHF, so the stored factor is the
//! reciprocal of the quoted rate.
//!
//! Rates are fetched when a currency is created, on demand from the admin page
//! (`POST /api/admin/currencies/refresh`) and once a day by a background task
//! (`CURRENCY_REFRESH_ENABLED=false` turns the task off). Each fetch or manual
//! edit stamps `rateUpdatedAt`.

use std::future::Future;
use std::time::Duration;

use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppResult;

/// The currency every amount is normalised to; its factor is always 1.
pub const BASE_CURRENCY: &str = "CHF";

const RATE_API: &str = "https://api.frankfurter.dev/v1/latest";

/// Rates older than this are refreshed by the background task.
const MAX_RATE_AGE: Duration = Duration::from_secs(24 * 3600);

/// Pause between scheduler checks after a run, so a failing source (offline,
/// rate-limited) is retried hourly instead of in a hot loop.
const RETRY_INTERVAL: Duration = Duration::from_secs(3600);

/// `CURRENCY_REFRESH_ENABLED` (default `true`) gates the daily background
/// refresh only — creating a currency and the admin refresh button always fetch.
pub fn refresh_enabled_from_env() -> bool {
    std::env::var("CURRENCY_REFRESH_ENABLED")
        .map(|v| v.trim().to_lowercase() != "false")
        .unwrap_or(true)
}

/// Fetch the current conversion factor (CHF per one unit of `code`).
pub async fn fetch_rate(code: String) -> Result<f64, String> {
    crate::install_crypto_provider();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("MotoManagerApi/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let url = url::Url::parse_with_params(RATE_API, [("from", BASE_CURRENCY), ("to", &code)])
        .map_err(|e| format!("invalid URL: {e}"))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("rate source answered {}", response.status()));
    }
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("invalid response: {e}"))?;
    let rate = json
        .get("rates")
        .and_then(|r| r.get(&code))
        .and_then(|r| r.as_f64())
        .ok_or_else(|| format!("no rate for {code}"))?;
    factor_from_quote(rate).ok_or_else(|| format!("invalid rate {rate} for {code}"))
}

/// Turn a `from=CHF` quote (units of the currency per CHF) into the stored
/// factor (CHF per unit of the currency).
pub fn factor_from_quote(units_per_chf: f64) -> Option<f64> {
    (units_per_chf.is_finite() && units_per_chf > 0.0).then(|| 1.0 / units_per_chf)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RefreshFailure {
    pub code: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RefreshReport {
    /// Codes whose factor and `rateUpdatedAt` were written.
    pub updated: Vec<String>,
    /// Codes left unchanged because their rate could not be fetched.
    pub failed: Vec<RefreshFailure>,
}

/// Refetch every non-base rate with `fetch` and store factor + timestamp. A
/// currency whose fetch fails keeps its row untouched and is reported.
pub async fn refresh_all<F, Fut>(pool: &SqlitePool, fetch: F) -> AppResult<RefreshReport>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<f64, String>>,
{
    let codes: Vec<String> =
        sqlx::query_scalar("SELECT code FROM currencies WHERE code != ? ORDER BY code")
            .bind(BASE_CURRENCY)
            .fetch_all(pool)
            .await?;

    let mut report = RefreshReport::default();
    for code in codes {
        match fetch(code.clone()).await {
            Ok(factor) if factor.is_finite() && factor > 0.0 => {
                sqlx::query(
                    "UPDATE currencies SET conversionFactor = ?, rateUpdatedAt = ? WHERE code = ?",
                )
                .bind(factor)
                .bind(Utc::now().to_rfc3339())
                .bind(&code)
                .execute(pool)
                .await?;
                report.updated.push(code);
            }
            Ok(factor) => report.failed.push(RefreshFailure {
                code,
                error: format!("invalid factor {factor}"),
            }),
            Err(error) => report.failed.push(RefreshFailure { code, error }),
        }
    }
    Ok(report)
}

/// Seconds until the next automatic refresh is due: `MAX_RATE_AGE` after the
/// newest non-base `rateUpdatedAt`; 0 when it is already due or unknown.
/// `None` when there is nothing to refresh (only the base currency).
pub async fn seconds_until_due(pool: &SqlitePool) -> AppResult<Option<u64>> {
    let rows: Vec<Option<String>> =
        sqlx::query_scalar("SELECT rateUpdatedAt FROM currencies WHERE code != ?")
            .bind(BASE_CURRENCY)
            .fetch_all(pool)
            .await?;
    if rows.is_empty() {
        return Ok(None);
    }
    let newest = rows
        .iter()
        .flatten()
        .filter_map(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .max();
    let Some(newest) = newest else {
        return Ok(Some(0));
    };
    let elapsed = Utc::now()
        .signed_duration_since(newest.with_timezone(&Utc))
        .num_seconds();
    Ok(Some((MAX_RATE_AGE.as_secs() as i64 - elapsed).max(0) as u64))
}

/// Background task: refresh all rates once they are a day old. Mirrors the
/// backup scheduler — settle after boot, then sleep until due.
pub fn spawn_scheduler(pool: SqlitePool) {
    tokio::spawn(async move {
        tracing::info!("Currency rate refresh running (daily)");
        tokio::time::sleep(Duration::from_secs(60)).await;

        loop {
            match seconds_until_due(&pool).await {
                Ok(Some(0)) => match refresh_all(&pool, fetch_rate).await {
                    Ok(report) => {
                        if !report.updated.is_empty() {
                            tracing::info!(
                                "Refreshed currency rates: {}",
                                report.updated.join(", ")
                            );
                        }
                        for failure in &report.failed {
                            tracing::warn!(
                                "Currency rate refresh for {} failed: {}",
                                failure.code,
                                failure.error
                            );
                        }
                    }
                    Err(e) => tracing::error!("Currency rate refresh failed: {e}"),
                },
                Ok(Some(due)) => {
                    tokio::time::sleep(Duration::from_secs(due)).await;
                    continue;
                }
                Ok(None) => {}
                Err(e) => tracing::error!("Currency rate refresh check failed: {e}"),
            }
            tokio::time::sleep(RETRY_INTERVAL).await;
        }
    });
}
