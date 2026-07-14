//! Supplier-invoice import: parse an uploaded PDF (Mark Huggett GmbH order
//! invoices and similar) into structured line items ready for review in the
//! client. This endpoint only PARSES — nothing is written to the database;
//! the client commits confirmed rows through the normal part/stock endpoints.
//!
//! Extraction strategy: pdfium pulls the text layer, then a local LLM
//! (OpenAI-compatible vLLM, reachable only from this server — see
//! `Config::llm_base_url`) structures it under a strict JSON schema. A
//! deterministic line parser for the known invoice layout doubles as the
//! fallback when the LLM is unreachable or returns something that fails
//! validation, so the feature degrades gracefully.

use axum::{
    extract::{Multipart, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    config::Config,
    error::{AppError, AppResult},
};

/// Rappen rounding on the invoices (4.41 × 1 is billed as 4.40) — arithmetic
/// checks must tolerate a nickel per line.
const LINE_TOTAL_TOLERANCE: f64 = 0.051;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceItem {
    pub quantity: i64,
    /// Part number as printed, e.g. "61 31 2 300 383".
    pub part_number: String,
    pub name: String,
    pub unit_price: Option<f64>,
    pub line_total: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ParsedInvoice {
    pub supplier: Option<String>,
    pub invoice_number: Option<String>,
    /// ISO date (YYYY-MM-DD).
    pub invoice_date: Option<String>,
    pub currency: Option<String>,
    pub items: Vec<InvoiceItem>,
}

/// One reviewed line as returned to the client: the parsed item plus how it
/// relates to the user's existing inventory.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewItem {
    #[serde(flatten)]
    item: InvoiceItem,
    /// Existing part with the same (normalized) part number, if any.
    matched_part_id: Option<i64>,
    matched_part_name: Option<String>,
    warnings: Vec<String>,
}

pub async fn parse_invoice(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    AuthUser(user): AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    let mut pdf_data: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        if field.name() == Some("file") {
            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("Upload error: {}", e)))?;
            pdf_data = Some(data.to_vec());
        }
    }
    let pdf_data =
        pdf_data.ok_or_else(|| AppError::BadRequest("Keine PDF-Datei erhalten".to_string()))?;
    if !pdf_data.starts_with(b"%PDF") {
        return Err(AppError::BadRequest(
            "Datei ist kein PDF (nur PDF-Rechnungen werden unterstützt)".to_string(),
        ));
    }

    // Pdfium is CPU-bound and its bindings are not Send-friendly: extract on a
    // blocking thread, same as document previews.
    let text = tokio::task::spawn_blocking(move || extract_pdf_text(&pdf_data))
        .await
        .map_err(|e| AppError::Internal(format!("PDF task panicked: {}", e)))??;

    if text.trim().is_empty() {
        return Err(AppError::BadRequest(
            "PDF enthält keinen Text (gescannte Rechnungen werden nicht unterstützt)".to_string(),
        ));
    }

    // Deterministic parse always runs: it is the fallback result and the
    // yardstick the LLM output is validated against.
    let fallback = parse_invoice_text(&text);

    let (mut parsed, source) = match structure_with_llm(&config, &text).await {
        Ok(llm) if is_plausible(&llm, &fallback) => (llm, "llm"),
        Ok(_) => {
            tracing::warn!("LLM invoice parse failed validation; using fallback parser");
            (fallback.clone(), "fallback")
        }
        Err(e) => {
            tracing::warn!(
                "LLM invoice parse unavailable ({}); using fallback parser",
                e
            );
            (fallback.clone(), "fallback")
        }
    };

    // Header metadata: where the deterministic parser matched, its values are
    // exact — small-model output is only trusted to FILL the gaps it left
    // (observed failure: the LLM returning the literal word "RECHNUNG" as the
    // invoice number).
    parsed.supplier = fallback.supplier.clone().or(parsed.supplier);
    parsed.invoice_number = fallback.invoice_number.clone().or(parsed.invoice_number);
    parsed.invoice_date = fallback.invoice_date.clone().or(parsed.invoice_date);
    parsed.currency = fallback.currency.clone().or(parsed.currency);

    if parsed.items.is_empty() {
        return Err(AppError::BadRequest(
            "Keine Positionen in der Rechnung gefunden".to_string(),
        ));
    }

    // Match items against the user's live parts by normalized part number.
    let rows = sqlx::query(
        "SELECT id, partNumber, name FROM parts WHERE userId = ? AND deletedAt IS NULL",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    let existing: Vec<(i64, String, String)> = rows
        .iter()
        .map(|r| (r.get("id"), r.get("partNumber"), r.get("name")))
        .collect();

    let items: Vec<ReviewItem> = parsed
        .items
        .iter()
        .map(|item| {
            let normalized = normalize_part_number(&item.part_number);
            let matched = existing
                .iter()
                .find(|(_, pn, _)| normalize_part_number(pn) == normalized);
            let mut warnings = Vec::new();
            if !is_bmw_part_number(&item.part_number) {
                warnings.push("Teilenummer hat kein BMW-Format".to_string());
            }
            if let (Some(unit), Some(total)) = (item.unit_price, item.line_total) {
                if (unit * item.quantity as f64 - total).abs() > LINE_TOTAL_TOLERANCE {
                    warnings.push(format!(
                        "Betrag {:.2} passt nicht zu {} × {:.2}",
                        total, item.quantity, unit
                    ));
                }
            }
            ReviewItem {
                item: item.clone(),
                matched_part_id: matched.map(|(id, _, _)| *id),
                matched_part_name: matched.map(|(_, _, name)| name.clone()),
                warnings,
            }
        })
        .collect();

    // Duplicate guard: committed rows carry the invoice number in their notes.
    let already_imported = match &parsed.invoice_number {
        Some(no) if !no.is_empty() => {
            let cnt: i64 = sqlx::query(
                "SELECT COUNT(*) AS cnt FROM partStocks s JOIN parts p ON p.id = s.partId \
                 WHERE p.userId = ? AND s.deletedAt IS NULL AND s.notes LIKE ?",
            )
            .bind(user.id)
            .bind(format!("%Rechnung {}%", no))
            .fetch_one(&pool)
            .await?
            .get("cnt");
            cnt > 0
        }
        _ => false,
    };

    Ok(Json(json!({
        "invoice": {
            "supplier": parsed.supplier,
            "invoiceNumber": parsed.invoice_number,
            "invoiceDate": parsed.invoice_date,
            "currency": parsed.currency.unwrap_or_else(|| "CHF".to_string()),
        },
        "items": items,
        "source": source,
        "alreadyImported": already_imported,
    })))
}

fn extract_pdf_text(data: &[u8]) -> AppResult<String> {
    let pdfium = crate::pdfium_lib::shared_pdfium().map_err(AppError::Image)?;
    let document = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| AppError::BadRequest(format!("PDF konnte nicht gelesen werden: {:?}", e)))?;

    let mut text = String::new();
    for page in document.pages().iter() {
        let page_text = page
            .text()
            .map_err(|e| AppError::Image(format!("PDF text extraction failed: {:?}", e)))?;
        text.push_str(&page_text.all());
        text.push('\n');
    }
    Ok(text)
}

// MARK: - LLM structuring

/// JSON schema the LLM output is constrained to (vLLM guided decoding).
fn invoice_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "supplier": {"type": ["string", "null"]},
            "invoiceNumber": {"type": ["string", "null"]},
            "invoiceDate": {
                "type": ["string", "null"],
                "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
            },
            "currency": {"type": ["string", "null"]},
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "quantity": {"type": "integer", "minimum": 1},
                        "partNumber": {"type": "string"},
                        "name": {"type": "string"},
                        "unitPrice": {"type": ["number", "null"]},
                        "lineTotal": {"type": ["number", "null"]}
                    },
                    "required": ["quantity", "partNumber", "name", "unitPrice", "lineTotal"]
                }
            }
        },
        "required": ["supplier", "invoiceNumber", "invoiceDate", "currency", "items"]
    })
}

async fn structure_with_llm(config: &Config, text: &str) -> Result<ParsedInvoice, String> {
    let base_url = config
        .llm_base_url
        .as_deref()
        .ok_or_else(|| "LLM_BASE_URL not configured".to_string())?;

    // The model context is small (2048 tokens on the deployed Qwen 1.5B), so
    // send only plausibly relevant lines and cap the total size.
    let condensed = condense_invoice_text(text);

    let request = json!({
        "model": config.llm_model,
        "temperature": 0,
        "max_tokens": 900,
        "messages": [
            {
                "role": "system",
                "content": "Du extrahierst Rechnungsdaten. Antworte nur mit JSON. \
                            items = alle Bestellpositionen (quantity, partNumber wie gedruckt, \
                            name, unitPrice, lineTotal). Eine Positionszeile sieht so aus: \
                            '3 12 11 1 351 564 Kondensator R50/5 13.70 41.10' ergibt \
                            {\"quantity\":3,\"partNumber\":\"12 11 1 351 564\",\
                            \"name\":\"Kondensator R50/5\",\"unitPrice\":13.70,\"lineTotal\":41.10}. \
                            Ersatzteilnummern haben 11 Ziffern in Gruppen (2-2-1-3-3); die \
                            Rechnungsnummer ist KEINE Ersatzteilnummer. invoiceDate im Format \
                            YYYY-MM-DD. Versand, Porto, Verpackung und Totale sind KEINE Positionen."
            },
            { "role": "user", "content": condensed }
        ],
        // Schema-constrained decoding (vLLM structured output). The older
        // top-level `guided_json` param is silently ignored by current vLLM;
        // the OpenAI-style response_format is the one that binds.
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "invoice", "schema": invoice_schema() }
        },
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(format!(
            "{}/chat/completions",
            base_url.trim_end_matches('/')
        ))
        .bearer_auth(&config.llm_api_key)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("LLM request failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("LLM returned HTTP {}", response.status()));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("LLM response not JSON: {}", e))?;
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "LLM response missing content".to_string())?;

    serde_json::from_str::<ParsedInvoice>(content)
        .map_err(|e| format!("LLM content failed schema parse: {}", e))
}

/// Keep only lines that can matter for extraction: drop bank footers, empty
/// lines and legalese so the prompt fits the small context window.
fn condense_invoice_text(text: &str) -> String {
    let noise = [
        "Bankkonto",
        "IBAN",
        "SWIFT",
        "Registergericht",
        "Postanschrift",
        "Telefon",
        "Internet",
        "bmwbike.com",
        "Keine Garantie",
    ];
    let mut out = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || noise.iter().any(|n| trimmed.contains(n)) {
            continue;
        }
        out.push_str(trimmed);
        out.push('\n');
        if out.len() > 3500 {
            break;
        }
    }
    out
}

/// Sanity-check the LLM result before trusting it over the deterministic
/// parser. When the known-layout parser found line items, its part numbers
/// are exact — the LLM result must reproduce every one of them (it may add
/// items the regex missed, e.g. wrapped lines). On unknown layouts the
/// fallback finds nothing and the LLM only has to be internally consistent.
fn is_plausible(llm: &ParsedInvoice, fallback: &ParsedInvoice) -> bool {
    if llm.items.is_empty() {
        return false;
    }
    let well_formed = llm
        .items
        .iter()
        .all(|i| i.quantity >= 1 && !i.part_number.trim().is_empty() && !i.name.trim().is_empty());
    if !well_formed {
        return false;
    }
    let llm_numbers: std::collections::HashSet<String> = llm
        .items
        .iter()
        .map(|i| normalize_part_number(&i.part_number))
        .collect();
    fallback
        .items
        .iter()
        .all(|i| llm_numbers.contains(&normalize_part_number(&i.part_number)))
}

// MARK: - Deterministic fallback parser (Mark Huggett GmbH layout)

pub fn normalize_part_number(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

/// BMW part numbers are 11 digits, printed as "12 11 1 351 564".
fn is_bmw_part_number(raw: &str) -> bool {
    let normalized = normalize_part_number(raw);
    normalized.len() == 11 && normalized.chars().all(|c| c.is_ascii_digit())
}

/// Parse the extracted text of a Huggett invoice. Line items look like
/// `3 12 11 1 351 564 Kondensator R50/5 - R100RS, 1969 - 1980 13.70 41.10`
/// (qty, 11-digit part number in 2-2-1-3-3 groups, name, unit price, total).
pub fn parse_invoice_text(text: &str) -> ParsedInvoice {
    let mut invoice = ParsedInvoice {
        currency: text.contains("CHF").then(|| "CHF".to_string()),
        // The letterhead is vector graphics — the company name never appears
        // in the text layer. Their VAT id does, and identifies the supplier
        // unambiguously.
        supplier: (text.contains("Mark Huggett") || text.contains("CHE-102.220.642"))
            .then(|| "Mark Huggett GmbH".to_string()),
        ..Default::default()
    };

    let item_re = regex::Regex::new(
        r"(?m)^\s*(\d{1,3}) (\d{2} \d{2} \d \d{3} \d{3}) (.+?) (\d+(?:'\d{3})*\.\d{2}) (\d+(?:'\d{3})*\.\d{2})\s*$",
    )
    .expect("static regex");
    for cap in item_re.captures_iter(text) {
        invoice.items.push(InvoiceItem {
            quantity: cap[1].parse().unwrap_or(1),
            part_number: cap[2].to_string(),
            name: cap[3].trim().to_string(),
            unit_price: parse_amount(&cap[4]),
            line_total: parse_amount(&cap[5]),
        });
    }

    // The invoice number is a standalone 6-digit line near the top of the
    // text stream (customer number is 5 digits, the order number is dashed,
    // the tracking number is dotted — none of them match).
    let number_re = regex::Regex::new(r"(?m)^\s*(\d{6})\s*$").expect("static regex");
    invoice.invoice_number = number_re.captures(text).map(|cap| cap[1].to_string());

    // "Holderbank, den 23.9.2024" → 2024-09-23
    let date_re = regex::Regex::new(r"den (\d{1,2})\.(\d{1,2})\.(\d{4})").expect("static regex");
    if let Some(cap) = date_re.captures(text) {
        invoice.invoice_date = Some(format!(
            "{}-{:02}-{:02}",
            &cap[3],
            cap[2].parse::<u32>().unwrap_or(1),
            cap[1].parse::<u32>().unwrap_or(1)
        ));
    }

    invoice
}

fn parse_amount(raw: &str) -> Option<f64> {
    raw.replace('\'', "").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Text layer of the two real Huggett invoices (addresses trimmed), as
    // pdfium extracts them — the acceptance fixtures for the fallback parser.
    const INVOICE_242511: &str = "11995\n242511\n2024-09-23-00002\n99.37.126432.00026680\nHalil Kimsesiz\nKunde/Customer:\nHolderbank, den 23.9.2024 Seite 1\nBst-Nr./Order-no.:\nRECHNUNG\nAnz. Ersatzteilnummer Artikelbezeichnung Stk/Preis Rab Betrag\nTrackingnummer:\nBearbeiter/Processor:\nShipping: Priority Gew./kg: 0.530\n3 12 11 1 351 564 Kondensator R50/5 - R100RS, 1969 - 1980 (NORIS Fabrikat) 13.70 41.10\n1 61 13 8 080 160 Tachowelle Gummitülle am Getriebe 2.10 2.10\n1 62 12 1 351 554 Gummitülle zur Drehzahlmesserwelle, R50/5 - R100RT 9.70 9.70\n1 62 12 1 357 731 Tachowelle, R60/6 - R100RT, R45 - R65, R80 - R100MYS 22.85 22.85\nMWSt. %\n75.75\nWarenwert\n6.95\nMWST: CHE-102.220.642 MWST\nEORI: DE714612052877641\n8.1\nTWINT\n92.90\nKeine Garantie auf elektronische Bauteile.\n0.00\nVerpackung\n10.20\nPorto\n85.95\nTotal vor MWSt Rechnungstotal in CHF\n";

    const INVOICE_242312: &str = "11995\n242312\n2024-08-27-00012\n99.37.126432.00026577\nHalil Kimsesiz\nKunde/Customer:\nHolderbank, den 30.8.2024 Seite 1\nBst-Nr./Order-no.:\nRECHNUNG\nAnz. Ersatzteilnummer Artikelbezeichnung Stk/Preis Rab Betrag\n2 13 11 1 260 874 Dellorto Gummitülle zu Gaszug, R90S 3.57 7.15\n1 46 63 2 315 304 Zylinderschraube mit Innensechskant M10 x 90 4.41 4.40\n4 51 18 1 823 474 Abdeckkappe 0.47 1.90\n1 61 31 1 244 708 Schalter Warnblinke 57.80 57.80\n1 61 31 2 300 383 Nachrüstsatz Griff beheizt 231.16 231.15\nMWSt. %\n302.40\nWarenwert\n25.30\nMWST: CHE-102.220.642 MWST\nEORI: DE714612052877641\n8.1\nKartenzahlung\n337.90\nKeine Garantie auf elektronische Bauteile.\n0.00\nVerpackung\n10.20\nPorto\n312.60\nTotal vor MWSt Rechnungstotal in CHF\n";

    #[test]
    fn parses_all_line_items_of_242511() {
        let parsed = parse_invoice_text(INVOICE_242511);
        assert_eq!(parsed.items.len(), 4);
        assert_eq!(parsed.items[0].quantity, 3);
        assert_eq!(parsed.items[0].part_number, "12 11 1 351 564");
        assert_eq!(
            parsed.items[0].name,
            "Kondensator R50/5 - R100RS, 1969 - 1980 (NORIS Fabrikat)"
        );
        assert_eq!(parsed.items[0].unit_price, Some(13.70));
        assert_eq!(parsed.items[0].line_total, Some(41.10));
        assert_eq!(parsed.items[3].part_number, "62 12 1 357 731");
    }

    #[test]
    fn parses_metadata_of_242511() {
        let parsed = parse_invoice_text(INVOICE_242511);
        assert_eq!(parsed.invoice_number.as_deref(), Some("242511"));
        assert_eq!(parsed.invoice_date.as_deref(), Some("2024-09-23"));
        assert_eq!(parsed.currency.as_deref(), Some("CHF"));
        assert_eq!(parsed.supplier.as_deref(), Some("Mark Huggett GmbH"));
    }

    #[test]
    fn parses_all_line_items_of_242312() {
        let parsed = parse_invoice_text(INVOICE_242312);
        assert_eq!(parsed.items.len(), 5);
        // Rappen rounding: 4.41 × 1 billed as 4.40 must survive parsing …
        assert_eq!(parsed.items[1].unit_price, Some(4.41));
        assert_eq!(parsed.items[1].line_total, Some(4.40));
        // … and 4 × 0.47 = 1.88 billed as 1.90.
        assert_eq!(parsed.items[2].quantity, 4);
        assert_eq!(parsed.items[2].line_total, Some(1.90));
        assert_eq!(parsed.invoice_number.as_deref(), Some("242312"));
        assert_eq!(parsed.invoice_date.as_deref(), Some("2024-08-30"));
    }

    #[test]
    fn normalizes_part_numbers() {
        assert_eq!(normalize_part_number("61 31 2 300 383"), "61312300383");
        assert_eq!(normalize_part_number("61-31-2-300-383"), "61312300383");
        assert!(is_bmw_part_number("61 31 2 300 383"));
        assert!(!is_bmw_part_number("12345"));
    }

    #[test]
    fn condense_drops_footer_noise() {
        let text = "1 61 31 1 244 708 Schalter 57.80 57.80\nBankkonto PostFinance\nIBAN CH49\nTelefon +41\n";
        let condensed = condense_invoice_text(text);
        assert!(condensed.contains("Schalter"));
        assert!(!condensed.contains("IBAN"));
        assert!(!condensed.contains("Telefon"));
    }

    /// Opt-in integration test against the real vLLM instance:
    /// `LLM_BASE_URL=http://10.0.0.2:8542/v1 cargo test llm_extracts -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn llm_extracts_real_invoice() {
        let config = Config::from_env().expect("config");
        assert!(config.llm_base_url.is_some(), "set LLM_BASE_URL");
        let parsed = structure_with_llm(&config, INVOICE_242511)
            .await
            .expect("LLM call");
        let fallback = parse_invoice_text(INVOICE_242511);
        assert!(
            is_plausible(&parsed, &fallback),
            "items: {:?}",
            parsed.items
        );
        assert_eq!(parsed.items.len(), 4);
        assert_eq!(
            normalize_part_number(&parsed.items[0].part_number),
            "12111351564"
        );
    }
}
