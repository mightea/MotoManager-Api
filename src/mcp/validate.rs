//! Input validation for MCP tool arguments. Tools call these before touching
//! a handler, so a model cannot pass through unbounded strings, absurd
//! numbers, unknown enum values, or currencies the system does not know.

use chrono::NaiveDate;
use sqlx::SqlitePool;

use crate::models::CurrencySetting;

pub const MAINTENANCE_TYPES: &[&str] = &[
    "tire",
    "battery",
    "brakepad",
    "brakerotor",
    "chain",
    "fluid",
    "general",
    "repair",
    "service",
    "inspection",
];
pub const FLUID_TYPES: &[&str] = &[
    "engineoil",
    "gearboxoil",
    "finaldriveoil",
    "finaldrivegearboxoil",
    "forkoil",
    "brakefluid",
    "coolant",
];
pub const TIRE_POSITIONS: &[&str] = &["front", "rear", "sidecar"];
pub const BATTERY_TYPES: &[&str] = &["lead-acid", "gel", "agm", "lithium-ion", "other"];
pub const OIL_TYPES: &[&str] = &["synthetic", "semi-synthetic", "mineral"];
pub const FUEL_TYPES: &[&str] = &["95E10", "98E5", "Diesel"];
pub const ISSUE_PRIORITIES: &[&str] = &["low", "medium", "high"];
pub const ISSUE_STATUSES: &[&str] = &["new", "in_progress", "done"];
pub const EXPENSE_CATEGORIES: &[&str] = &[
    "Versicherung",
    "Steuern",
    "Vignette",
    "Parkplatz",
    "Ausrüstung",
    "Sonstiges",
];

pub const MAX_ODO: i64 = 9_999_999;
pub const MAX_AMOUNT: f64 = 10_000_000.0;
pub const MAX_QUANTITY: i64 = 10_000;
pub const MAX_FUEL_LITERS: f64 = 500.0;
pub const SHORT_TEXT: usize = 200;
pub const LONG_TEXT: usize = 4000;

/// A user-facing validation failure. Rendered as a tool-level error so the
/// model can correct its call.
#[derive(Debug)]
pub struct Invalid(pub String);

impl Invalid {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

pub type Validated<T> = Result<T, Invalid>;

/// `YYYY-MM-DD` only; also caps the year to a plausible range so a typo like
/// `20250-01-01` is caught.
pub fn date(field: &str, value: &str) -> Validated<String> {
    let value = value.trim();
    let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| Invalid::new(format!("{field} must be a date in YYYY-MM-DD format")))?;
    let year = chrono::Datelike::year(&parsed);
    if !(1900..=2100).contains(&year) {
        return Err(Invalid::new(format!("{field} has an implausible year")));
    }
    Ok(parsed.format("%Y-%m-%d").to_string())
}

pub fn odo(value: i64) -> Validated<i64> {
    if !(0..=MAX_ODO).contains(&value) {
        return Err(Invalid::new(format!(
            "odo must be between 0 and {MAX_ODO} kilometres"
        )));
    }
    Ok(value)
}

/// Non-negative, finite money amount rounded to cents.
pub fn amount(field: &str, value: f64) -> Validated<f64> {
    if !value.is_finite() || !(0.0..=MAX_AMOUNT).contains(&value) {
        return Err(Invalid::new(format!(
            "{field} must be a non-negative number up to {MAX_AMOUNT}"
        )));
    }
    Ok((value * 100.0).round() / 100.0)
}

pub fn quantity(field: &str, value: i64) -> Validated<i64> {
    if !(1..=MAX_QUANTITY).contains(&value) {
        return Err(Invalid::new(format!(
            "{field} must be between 1 and {MAX_QUANTITY}"
        )));
    }
    Ok(value)
}

pub fn liters(value: f64) -> Validated<f64> {
    if !value.is_finite() || value <= 0.0 || value > MAX_FUEL_LITERS {
        return Err(Invalid::new(format!(
            "liters must be a positive number up to {MAX_FUEL_LITERS}"
        )));
    }
    Ok((value * 1000.0).round() / 1000.0)
}

/// Trims, strips control characters (newlines are kept for long text), and
/// enforces a length cap. `None`/blank input yields `None`.
pub fn text(field: &str, value: Option<&str>, max_len: usize) -> Validated<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let cleaned: String = value
        .chars()
        .filter(|c| !c.is_control() || (max_len > SHORT_TEXT && (*c == '\n' || *c == '\t')))
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        return Ok(None);
    }
    if cleaned.chars().count() > max_len {
        return Err(Invalid::new(format!(
            "{field} must be at most {max_len} characters"
        )));
    }
    Ok(Some(cleaned))
}

pub fn required_text(field: &str, value: &str, max_len: usize) -> Validated<String> {
    text(field, Some(value), max_len)?.ok_or_else(|| Invalid::new(format!("{field} is required")))
}

pub fn one_of(field: &str, value: &str, allowed: &[&str]) -> Validated<String> {
    let value = value.trim();
    if allowed.contains(&value) {
        return Ok(value.to_string());
    }
    Err(Invalid::new(format!(
        "{field} must be one of: {}",
        allowed.join(", ")
    )))
}

pub fn optional_one_of(
    field: &str,
    value: Option<&str>,
    allowed: &[&str],
) -> Validated<Option<String>> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => one_of(field, v, allowed).map(Some),
    }
}

/// Optional client idempotency key, forwarded as the record's `clientId` so
/// a retried tool call cannot duplicate a record.
pub fn idempotency_key(value: Option<&str>) -> Validated<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let ok = (8..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !ok {
        return Err(Invalid::new(
            "idempotency_key must be 8-64 characters of letters, digits, '-' or '_'",
        ));
    }
    Ok(Some(value.to_string()))
}

/// A known currency plus its conversion factor to the base currency (CHF),
/// so tools can fill `normalizedCost` exactly like the webapp does.
pub struct Currency {
    pub code: String,
    pub conversion_factor: f64,
}

pub async fn currency(pool: &SqlitePool, value: &str) -> Result<Currency, ToolFailure> {
    let code = value.trim().to_ascii_uppercase();
    if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(Invalid::new("currency must be a three-letter ISO code").into());
    }
    let known = sqlx::query_as::<_, CurrencySetting>("SELECT * FROM currencies ORDER BY code")
        .fetch_all(pool)
        .await?;
    if let Some(c) = known.iter().find(|c| c.code.eq_ignore_ascii_case(&code)) {
        return Ok(Currency {
            code: c.code.clone(),
            conversion_factor: c.conversion_factor,
        });
    }
    let codes: Vec<&str> = known.iter().map(|c| c.code.as_str()).collect();
    Err(Invalid::new(format!(
        "currency {code} is not configured; available: {}",
        codes.join(", ")
    ))
    .into())
}

/// Either a validation failure (shown to the model) or an application error
/// from a handler (mapped in `server.rs`).
#[derive(Debug)]
pub enum ToolFailure {
    Invalid(String),
    App(crate::error::AppError),
}

impl From<Invalid> for ToolFailure {
    fn from(value: Invalid) -> Self {
        ToolFailure::Invalid(value.0)
    }
}

impl From<crate::error::AppError> for ToolFailure {
    fn from(value: crate::error::AppError) -> Self {
        ToolFailure::App(value)
    }
}

impl From<sqlx::Error> for ToolFailure {
    fn from(value: sqlx::Error) -> Self {
        ToolFailure::App(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_must_be_iso_and_plausible() {
        assert_eq!(date("date", " 2026-03-05 ").unwrap(), "2026-03-05");
        assert!(date("date", "05.03.2026").is_err());
        assert!(date("date", "2026-13-01").is_err());
        assert!(date("date", "0999-01-01").is_err());
    }

    #[test]
    fn numbers_are_bounded() {
        assert!(odo(-1).is_err());
        assert!(odo(MAX_ODO + 1).is_err());
        assert_eq!(odo(12_345).unwrap(), 12_345);
        assert_eq!(amount("cost", 12.345).unwrap(), 12.35);
        assert!(amount("cost", f64::NAN).is_err());
        assert!(amount("cost", -0.01).is_err());
        assert!(quantity("quantity", 0).is_err());
        assert!(liters(0.0).is_err());
    }

    #[test]
    fn text_is_trimmed_capped_and_cleaned() {
        assert_eq!(
            text("d", Some("  hi\u{0} there "), 200).unwrap().as_deref(),
            Some("hi there")
        );
        assert_eq!(text("d", Some("   "), 200).unwrap(), None);
        assert!(text("d", Some(&"x".repeat(201)), 200).is_err());
        assert_eq!(
            text("d", Some("a\nb"), 4000).unwrap().as_deref(),
            Some("a\nb")
        );
        assert_eq!(text("d", Some("a\nb"), 200).unwrap().as_deref(), Some("ab"));
        assert!(required_text("title", " ", 200).is_err());
    }

    #[test]
    fn enums_and_keys_are_strict() {
        assert!(one_of("type", "fuel", MAINTENANCE_TYPES).is_err());
        assert_eq!(
            one_of("type", " service ", MAINTENANCE_TYPES).unwrap(),
            "service"
        );
        assert!(idempotency_key(Some("short")).is_err());
        assert!(idempotency_key(Some("has space 123")).is_err());
        assert_eq!(idempotency_key(None).unwrap(), None);
        assert!(idempotency_key(Some("0f3c8d2a-1b2c-4d5e"))
            .unwrap()
            .is_some());
    }
}
