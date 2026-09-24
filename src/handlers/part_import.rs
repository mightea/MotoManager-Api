//! Supplier-document import: parse an uploaded PDF (Mark Huggett GmbH
//! invoices, boxxerparts.de order confirmations and similar) into structured
//! line items ready for review in the client. This endpoint only PARSES —
//! nothing is written to the database; the client commits confirmed rows
//! through the normal part/stock endpoints.
//!
//! Extraction strategy: pdfium pulls the text layer, then a deterministic
//! layout parser for each known supplier runs. For unknown layouts (and as a
//! second opinion on Huggett invoices) a local LLM (OpenAI-compatible vLLM,
//! reachable only from this server — see `Config::llm_base_url`) structures
//! the text under a strict JSON schema; the layout parser doubles as the
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

/// Part numbers of boxxerparts.de articles are 5-digit shop numbers that
/// would collide with other suppliers' short numbers, so they are stored
/// namespaced: article 44555 becomes `BXP-44555`.
pub const BOXXERPARTS_PREFIX: &str = "BXP-";

/// Supplier whose document layout the text was recognized as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Supplier {
    Huggett,
    Boxxerparts,
    #[default]
    Unknown,
}

impl Supplier {
    /// Stable key the client dispatches its enrichment on.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Supplier::Huggett => Some("huggett"),
            Supplier::Boxxerparts => Some("boxxerparts"),
            Supplier::Unknown => None,
        }
    }
}

/// What kind of document was parsed — decides the wording of the stock note
/// ("Rechnung 242511" vs "Bestellung 72669") and thereby the duplicate guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DocumentKind {
    #[default]
    Invoice,
    Order,
}

impl DocumentKind {
    /// German label used in stock notes, e.g. "Bestellung 72669".
    pub fn note_label(self) -> &'static str {
        match self {
            DocumentKind::Invoice => "Rechnung",
            DocumentKind::Order => "Bestellung",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceItem {
    pub quantity: i64,
    /// Part number as printed, e.g. "61 31 2 300 383" — or namespaced for
    /// suppliers with their own numbering ("BXP-44555").
    pub part_number: String,
    pub name: String,
    pub unit_price: Option<f64>,
    pub line_total: Option<f64>,
    /// Descriptive text printed under the line (order confirmations carry
    /// the shop's product description).
    #[serde(default)]
    pub description: Option<String>,
    /// The supplier's own article number without namespace prefix, for
    /// catalog lookups ("44555").
    #[serde(default)]
    pub supplier_article_no: Option<String>,
    /// BMW part numbers cited in the description ("12 32 1 244 409") — used
    /// to match aftermarket lines against parts stored under the OEM number.
    #[serde(default)]
    pub oem_part_numbers: Vec<String>,
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
    #[serde(default)]
    pub supplier_kind: Supplier,
    #[serde(default)]
    pub document_kind: DocumentKind,
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
    /// "partNumber" for a direct hit, "oemPartNumber" when an OEM number in
    /// the description matched an existing part.
    matched_via: Option<&'static str>,
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
    let data = pdf_data.ok_or_else(|| AppError::BadRequest("Keine Datei erhalten".to_string()))?;
    let (text, text_source) = document_text(data).await?;

    // Deterministic parse always runs: it is the fallback result and the
    // yardstick the LLM output is validated against.
    let fallback = parse_layout(&text);

    let (mut parsed, source) =
        if fallback.supplier_kind == Supplier::Boxxerparts && !fallback.items.is_empty() {
            // The boxxerparts layout is fully deterministic (one closing line
            // with article number and price per item) and the LLM prompt is
            // tuned to BMW numbers — the model could only add noise here.
            (fallback.clone(), "layout")
        } else {
            match structure_with_llm(&config, &text).await {
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
    parsed.supplier_kind = fallback.supplier_kind;
    parsed.document_kind = fallback.document_kind;
    prefer_layout_line_values(&mut parsed, &fallback);

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
    let find_existing = |number: &str| {
        let normalized = normalize_part_number(number);
        existing
            .iter()
            .find(|(_, pn, _)| normalize_part_number(pn) == normalized)
    };

    let items: Vec<ReviewItem> = parsed
        .items
        .iter()
        .map(|item| {
            let mut warnings = Vec::new();
            let mut matched_via = None;
            let mut matched = None;
            if !item.part_number.trim().is_empty() {
                matched = find_existing(&item.part_number);
                if matched.is_some() {
                    matched_via = Some("partNumber");
                }
            } else {
                warnings.push("Keine Artikelnummer erkannt".to_string());
            }
            // Aftermarket lines citing the OEM number: book the stock onto
            // the part the user already keeps under that BMW number.
            if matched.is_none() {
                for oem in &item.oem_part_numbers {
                    if let Some(hit) = find_existing(oem) {
                        matched = Some(hit);
                        matched_via = Some("oemPartNumber");
                        warnings.push(format!("Über BMW-Nr. {} zugeordnet", oem));
                        break;
                    }
                }
            }
            if parsed.supplier_kind != Supplier::Boxxerparts
                && !item.part_number.trim().is_empty()
                && !is_bmw_part_number(&item.part_number)
            {
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
                matched_via,
                warnings,
            }
        })
        .collect();

    // Duplicate guard: committed rows carry the document number in their
    // notes ("… Rechnung 242511" / "… Bestellung 72669").
    let already_imported = match &parsed.invoice_number {
        Some(no) if !no.is_empty() => {
            let cnt: i64 = sqlx::query(
                "SELECT COUNT(*) AS cnt FROM partStocks s JOIN parts p ON p.id = s.partId \
                 WHERE p.userId = ? AND s.deletedAt IS NULL AND s.notes LIKE ?",
            )
            .bind(user.id)
            .bind(format!("%{} {}%", parsed.document_kind.note_label(), no))
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
            "supplierKey": parsed.supplier_kind.key(),
            "documentKind": parsed.document_kind,
            "invoiceNumber": parsed.invoice_number,
            "invoiceDate": parsed.invoice_date,
            "currency": parsed.currency.unwrap_or_else(|| "CHF".to_string()),
        },
        "items": items,
        "source": source,
        // "pdf" = embedded text layer, "ocr" = server-side text recognition
        // (scans/photos) — the client asks for a closer review then.
        "textSource": text_source,
        "alreadyImported": already_imported,
    })))
}

/// Text of an uploaded supplier document: the PDF's own text layer, or —
/// for image-only PDFs (raw scans) and photos — server-side OCR.
async fn document_text(data: Vec<u8>) -> AppResult<(String, &'static str)> {
    let is_pdf = data.starts_with(b"%PDF");
    if !is_pdf && !is_supported_image(&data) {
        return Err(AppError::BadRequest(
            "Datei ist weder PDF noch Bild (unterstützt: PDF, JPEG, PNG, WebP)".to_string(),
        ));
    }

    let pages = if is_pdf {
        // Pdfium is CPU-bound and its bindings are not Send-friendly: extract
        // on a blocking thread, same as document previews.
        let (text, data) =
            tokio::task::spawn_blocking(move || extract_pdf_text(&data).map(|text| (text, data)))
                .await
                .map_err(|e| AppError::Internal(format!("PDF task panicked: {}", e)))??;
        if has_text_layer(&text) {
            return Ok((text, "pdf"));
        }
        tokio::task::spawn_blocking(move || crate::ocr::render_pdf_pages(&data))
    } else {
        tokio::task::spawn_blocking(move || crate::ocr::decode_image(&data).map(|p| vec![p]))
    }
    .await
    .map_err(|e| AppError::Internal(format!("OCR task panicked: {}", e)))?
    .map_err(ocr_error)?;

    let text = crate::ocr::recognize_pages(pages)
        .await
        .map_err(ocr_error)?;
    if !has_text_layer(&text) {
        return Err(AppError::BadRequest(
            "Keine Schrift erkannt — ist der Scan scharf und richtig herum?".to_string(),
        ));
    }
    Ok((crate::ocr::clean_ocr_text(&text), "ocr"))
}

fn ocr_error(e: crate::ocr::OcrError) -> AppError {
    use crate::ocr::OcrError;
    match e {
        OcrError::BadInput(msg) => AppError::BadRequest(msg),
        OcrError::Unavailable(msg) => {
            tracing::error!("OCR unavailable: {}", msg);
            AppError::Internal("Texterkennung (OCR) ist auf dem Server nicht verfügbar".to_string())
        }
        OcrError::Failed(msg) => {
            AppError::Internal(format!("Texterkennung fehlgeschlagen: {}", msg))
        }
    }
}

/// A scanner's image-only PDF has no text at all, but some add a stray
/// producer string — a few characters are not a document.
fn has_text_layer(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphanumeric()).count() >= 20
}

fn is_supported_image(data: &[u8]) -> bool {
    matches!(
        image::guess_format(data),
        Ok(image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::WebP)
    )
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

    crate::install_crypto_provider();
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
        "Registriergericht",
        "Geschäftsführer",
        "Postanschrift",
        "Telefon",
        "Internet",
        "bmwbike.com",
        "Keine Garantie",
        "Widerruf",
        "Reply-To:",
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

/// Same rule for line items: where the layout parser matched a line, its
/// quantity and prices are read straight off the row and win over the LLM's
/// (observed failure: the model returning the invoice total incl. shipping
/// as a line amount). The LLM keeps only what the regex could not see.
fn prefer_layout_line_values(parsed: &mut ParsedInvoice, fallback: &ParsedInvoice) {
    for item in &mut parsed.items {
        let number = normalize_part_number(&item.part_number);
        if let Some(exact) = fallback
            .items
            .iter()
            .find(|f| normalize_part_number(&f.part_number) == number)
        {
            item.part_number = exact.part_number.clone();
            item.quantity = exact.quantity;
            item.unit_price = exact.unit_price;
            item.line_total = exact.line_total;
        }
    }
}

// MARK: - Deterministic layout parsers

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

/// Find BMW part numbers cited in free text ("BMW 12321244409", "12 32 1 244
/// 409") and return them in canonical 2-2-1-3-3 spacing, deduplicated.
pub fn extract_oem_part_numbers(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r"(?:^|[^0-9])(\d{2}) ?(\d{2}) ?(\d) ?(\d{3}) ?(\d{3})(?:[^0-9]|$)")
        .expect("static regex");
    let mut out: Vec<String> = Vec::new();
    for cap in re.captures_iter(text) {
        let number = format!(
            "{} {} {} {} {}",
            &cap[1], &cap[2], &cap[3], &cap[4], &cap[5]
        );
        if !out.contains(&number) {
            out.push(number);
        }
    }
    out
}

/// Recognize the supplier from the text layer.
pub fn detect_supplier(text: &str) -> Supplier {
    if text.contains("boxxerparts") || text.contains("KEC GmbH") {
        Supplier::Boxxerparts
    } else if text.contains("Mark Huggett") || text.contains("CHE-102.220.642") {
        Supplier::Huggett
    } else {
        Supplier::Unknown
    }
}

/// Dispatch to the layout parser of the recognized supplier. Unknown layouts
/// go through the Huggett parser, whose line regex is strict enough to find
/// nothing rather than something wrong.
pub fn parse_layout(text: &str) -> ParsedInvoice {
    match detect_supplier(text) {
        Supplier::Boxxerparts => parse_boxxerparts_text(text),
        Supplier::Huggett | Supplier::Unknown => parse_invoice_text(text),
    }
}

/// Parse the extracted text of a Huggett invoice. Line items look like
/// `3 12 11 1 351 564 Kondensator R50/5 - R100RS, 1969 - 1980 13.70 41.10`
/// (qty, 11-digit part number in 2-2-1-3-3 groups, name, unit price, total).
pub fn parse_invoice_text(text: &str) -> ParsedInvoice {
    let supplier_kind = match detect_supplier(text) {
        Supplier::Huggett => Supplier::Huggett,
        _ => Supplier::Unknown,
    };
    let mut invoice = ParsedInvoice {
        // Huggett bills in CHF; an OCR'd paper invoice can lose the footer
        // line that names the currency.
        currency: (text.contains("CHF") || supplier_kind == Supplier::Huggett)
            .then(|| "CHF".to_string()),
        // The letterhead is vector graphics — the company name never appears
        // in the text layer. Their VAT id does, and identifies the supplier
        // unambiguously.
        supplier: (supplier_kind == Supplier::Huggett).then(|| "Mark Huggett GmbH".to_string()),
        supplier_kind,
        document_kind: DocumentKind::Invoice,
        ..Default::default()
    };

    // Tolerant of OCR'd paper invoices: the table rule between quantity and
    // part number is read as "|", "[", "!" or nothing at all ("183 30 0 401
    // 758" = qty 1 + 83 30 0 401 758 — the rigid 2-2-1-3-3 groups leave only
    // one split), the number's group spacing can collapse, and thousands
    // separators vary.
    let item_re = regex::Regex::new(
        r"(?m)^\s*(\d{1,3})(?:\s*[|\[\]!]\s*|\s*)(\d{2}) ?(\d{2}) ?(\d) ?(\d{3}) ?(\d{3}) (.+?) (\d+(?:['’`]\d{3})*\.\d{2}) (\d+(?:['’`]\d{3})*\.\d{2})\s*$",
    )
    .expect("static regex");
    for cap in item_re.captures_iter(text) {
        let unit_price = parse_amount(&cap[8]);
        let line_total = parse_amount(&cap[9]);
        invoice.items.push(InvoiceItem {
            quantity: consistent_quantity(cap[1].parse().unwrap_or(1), unit_price, line_total),
            part_number: format!(
                "{} {} {} {} {}",
                &cap[2], &cap[3], &cap[4], &cap[5], &cap[6]
            ),
            name: cap[7].trim().to_string(),
            unit_price,
            line_total,
            description: None,
            supplier_article_no: None,
            oem_part_numbers: Vec::new(),
        });
    }

    // The invoice number is a standalone 6-digit line near the top of the
    // text stream (customer number is 5 digits, the order number is dashed,
    // the tracking number is dotted — none of them match).
    // OCR'd scans keep the label on the same line: "RECHNUNG 262462".
    let number_re =
        regex::Regex::new(r"(?m)^\s*(?:RECHNUNG\s+)?(\d{6})\s*$").expect("static regex");
    invoice.invoice_number = number_re.captures(text).map(|cap| cap[1].to_string());

    // "Holderbank, den 23.9.2024" → 2024-09-23
    let date_re = regex::Regex::new(r"den (\d{1,2})\.(\d{1,2})\.(\d{4})").expect("static regex");
    if let Some(cap) = date_re.captures(text) {
        invoice.invoice_date = Some(iso_date(&cap[3], &cap[2], &cap[1]));
    }

    invoice
}

/// Parse a boxxerparts.de order confirmation (the "Auftragsbestätigung"
/// e-mail, typically printed to PDF from the mail client). Each item is a
/// block of lines:
///
/// ```text
/// 2 x Winkelventil 8,3 mm. für BMW R 100 / 80 GS R Kreuzspeichen Felgen
/// ALU Winkel Ventil 90 Grad, Durchmesser 8,3 mm (bitte den Lochdurchmesser …
/// Lieferzeit: 3-4 Tage
/// 96065 4,12 EUR 8
/// ```
///
/// The trailing line total is clipped by the mail client's print layout
/// (only its first digit survives), so totals are computed from quantity ×
/// unit price instead of read.
pub fn parse_boxxerparts_text(text: &str) -> ParsedInvoice {
    let mut invoice = ParsedInvoice {
        supplier: Some("Boxxerparts".to_string()),
        supplier_kind: Supplier::Boxxerparts,
        document_kind: DocumentKind::Order,
        currency: text.contains(" EUR").then(|| "EUR".to_string()),
        ..Default::default()
    };

    let header_re = regex::Regex::new(r"^(\d{1,3}) x (.+)$").expect("static regex");
    let closing_re = regex::Regex::new(r"^(\d{4,6}) (\d{1,3}(?:\.\d{3})*,\d{2}) EUR(?:\s.*)?$")
        .expect("static regex");

    struct Open {
        quantity: i64,
        name: String,
        description: Vec<String>,
    }
    let mut open: Option<Open> = None;
    let finish =
        |open: Open, article: Option<(&str, Option<f64>)>, items: &mut Vec<InvoiceItem>| {
            let description = open.description.join(" ");
            let description = description.trim();
            let oem_part_numbers = extract_oem_part_numbers(description);
            let (part_number, supplier_article_no, unit_price) = match article {
                Some((no, price)) => (
                    format!("{}{}", BOXXERPARTS_PREFIX, no),
                    Some(no.to_string()),
                    price,
                ),
                None => (String::new(), None, None),
            };
            items.push(InvoiceItem {
                quantity: open.quantity,
                part_number,
                name: open.name,
                unit_price,
                line_total: unit_price.map(|p| (p * open.quantity as f64 * 100.0).round() / 100.0),
                description: (!description.is_empty()).then(|| description.to_string()),
                supplier_article_no,
                oem_part_numbers,
            });
        };

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("Lieferzeit:") || line == "[...]" {
            continue;
        }
        if let Some(cap) = header_re.captures(line) {
            if let Some(previous) = open.take() {
                // Item without a closing line — keep it visible for review
                // instead of silently dropping it.
                finish(previous, None, &mut invoice.items);
            }
            open = Some(Open {
                quantity: cap[1].parse().unwrap_or(1),
                name: cap[2].trim().to_string(),
                description: Vec::new(),
            });
            continue;
        }
        if let Some(cap) = closing_re.captures(line) {
            if let Some(current) = open.take() {
                let price = parse_amount(&cap[2].replace('.', "").replace(',', "."));
                finish(current, Some((&cap[1], price)), &mut invoice.items);
            }
            continue;
        }
        if let Some(current) = open.as_mut() {
            current
                .description
                .push(line.trim_end_matches("[...]").trim().to_string());
        }
    }
    if let Some(previous) = open.take() {
        finish(previous, None, &mut invoice.items);
    }

    let number_re = regex::Regex::new(r"(?i)Bestellung\s+Nr\.?:?\s*(\d+)").expect("static regex");
    invoice.invoice_number = number_re.captures(text).map(|cap| cap[1].to_string());

    let date_re =
        regex::Regex::new(r"Bestelldatum:\s*(\d{1,2})\.(\d{1,2})\.(\d{4})").expect("static regex");
    if let Some(cap) = date_re.captures(text) {
        invoice.invoice_date = Some(iso_date(&cap[3], &cap[2], &cap[1]));
    }

    invoice
}

fn iso_date(year: &str, month: &str, day: &str) -> String {
    format!(
        "{}-{:02}-{:02}",
        year,
        month.parse::<u32>().unwrap_or(1),
        day.parse::<u32>().unwrap_or(1)
    )
}

/// The quantity printed on the row, unless the row's own prices say
/// otherwise: OCR can read the table rule next to the quantity as an extra
/// "1" ("1183 30 0 401 758" → qty 11), while total ÷ unit price is exact.
fn consistent_quantity(printed: i64, unit_price: Option<f64>, line_total: Option<f64>) -> i64 {
    let (Some(unit), Some(total)) = (unit_price, line_total) else {
        return printed;
    };
    if unit <= 0.0 || (unit * printed as f64 - total).abs() <= LINE_TOTAL_TOLERANCE {
        return printed;
    }
    let derived = (total / unit).round();
    if derived >= 1.0 && (unit * derived - total).abs() <= LINE_TOTAL_TOLERANCE {
        derived as i64
    } else {
        printed
    }
}

fn parse_amount(raw: &str) -> Option<f64> {
    raw.replace(['\'', '’', '`'], "").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Text layer of the two real Huggett invoices (addresses trimmed), as
    // pdfium extracts them — the acceptance fixtures for the fallback parser.
    const INVOICE_242511: &str = "11995\n242511\n2024-09-23-00002\n99.37.126432.00026680\nHalil Kimsesiz\nKunde/Customer:\nHolderbank, den 23.9.2024 Seite 1\nBst-Nr./Order-no.:\nRECHNUNG\nAnz. Ersatzteilnummer Artikelbezeichnung Stk/Preis Rab Betrag\nTrackingnummer:\nBearbeiter/Processor:\nShipping: Priority Gew./kg: 0.530\n3 12 11 1 351 564 Kondensator R50/5 - R100RS, 1969 - 1980 (NORIS Fabrikat) 13.70 41.10\n1 61 13 8 080 160 Tachowelle Gummitülle am Getriebe 2.10 2.10\n1 62 12 1 351 554 Gummitülle zur Drehzahlmesserwelle, R50/5 - R100RT 9.70 9.70\n1 62 12 1 357 731 Tachowelle, R60/6 - R100RT, R45 - R65, R80 - R100MYS 22.85 22.85\nMWSt. %\n75.75\nWarenwert\n6.95\nMWST: CHE-102.220.642 MWST\nEORI: DE714612052877641\n8.1\nTWINT\n92.90\nKeine Garantie auf elektronische Bauteile.\n0.00\nVerpackung\n10.20\nPorto\n85.95\nTotal vor MWSt Rechnungstotal in CHF\n";

    const INVOICE_242312: &str = "11995\n242312\n2024-08-27-00012\n99.37.126432.00026577\nHalil Kimsesiz\nKunde/Customer:\nHolderbank, den 30.8.2024 Seite 1\nBst-Nr./Order-no.:\nRECHNUNG\nAnz. Ersatzteilnummer Artikelbezeichnung Stk/Preis Rab Betrag\n2 13 11 1 260 874 Dellorto Gummitülle zu Gaszug, R90S 3.57 7.15\n1 46 63 2 315 304 Zylinderschraube mit Innensechskant M10 x 90 4.41 4.40\n4 51 18 1 823 474 Abdeckkappe 0.47 1.90\n1 61 31 1 244 708 Schalter Warnblinke 57.80 57.80\n1 61 31 2 300 383 Nachrüstsatz Griff beheizt 231.16 231.15\nMWSt. %\n302.40\nWarenwert\n25.30\nMWST: CHE-102.220.642 MWST\nEORI: DE714612052877641\n8.1\nKartenzahlung\n337.90\nKeine Garantie auf elektronische Bauteile.\n0.00\nVerpackung\n10.20\nPorto\n312.60\nTotal vor MWSt Rechnungstotal in CHF\n";

    // Text layer of a real boxxerparts.de order confirmation (Gmail print to
    // PDF, addresses trimmed), as pdfium extracts it — note the clipped line
    // totals ("8" instead of "8,24") and the CRLF line endings.
    const ORDER_72669: &str = "Auftragsbestätigung Nr:72669 / 21.09.2026\r\n1 message\r\nboxxerparts.de Onlineshop <shop@boxxerparts.com> 21 September 2026 at 21:55\r\nReply-To: \"boxxerparts.de Onlineshop\" <shop@boxxerparts.com>\r\nIHRE BESTELLUNG NR. 72669\r\nSehr geehrter Herr Tobias Herrmann,\r\nIhre Bestellung ist bei uns eingegangen. Sie erhalten von uns diese Auftragsbestätigung, mit der wir Ihr Vertragsangebo\r\nannehmen. Der Vertrag zwischen Ihnen und uns ist daher zustande gekommen.\r\nVersandart: Paketversand\r\nZahlungsmethode: PayPal\r\nBestellung Nr: 72669\r\nBestelldatum: 21.09.2026\r\nRechnungs-/Lieferadresse\r\nIhre bestellten Produkte nochmals zur Kontrolle:\r\nStk. Produkt Art.-Nr. Einzelpreis\r\n2 x Winkelventil 8,3 mm. für BMW R 100 / 80 GS R Kreuzspeichen Felgen\r\nALU Winkel Ventil 90 Grad, Durchmesser 8,3 mm (bitte den Lochdurchmesser an der eigenen Felge\r\nmessen). Geeignet für Speichenfelgen mit schlauchlosen Reifen. Erleichtert den Umgang mit den\r\nReifenluftdruckgeräten an den Tankstellen. Anzugs-Drehmoment: 7 - 10 [...]\r\nLieferzeit: 3-4 Tage\r\n96065 4,12 EUR 8\r\n1 x Regler Wehrle für alle R2V Boxer ab 69\r\nRegler passend für alle 2 V Boxer ab Baujahr 1969 vom Erstausrüster BMW 12321244409 Wehrle\r\n[...]\r\nLieferzeit: 3-4 Tage\r\n44555 49,16 EUR 49\r\n2 x Auspuff Sternmutter für die 2V Boxer\r\nFür alle BMW 2 V 80/100 Modelle ab /7 Ausnahme: R 45 und R 65 Preis je Stück BMW Teilenummer:\r\n[...]\r\n44545 24,79 EUR 49\nLieferzeit: 3-4 Tage\r\n2 x Benzinleitung Schnellverschluss für 6 mm Benzinleitung\r\nSchnellkupplung für 6 mm Benzinschlauch zum Bespiel für die BMW 2 V Boxer Ideal zum Trennen von\r\nBenzinleitungen zwischen Vergaser und [...]\r\nLieferzeit: 3-4 Tage\r\n91095 16,30 EUR 32\r\n2 x Neopren Kraftstoffschlauch 6.0mm. - 1m.\r\nKraftstoffschlauch aus Neopren - Besteht aus 2 Materialien (Außen: Neoprene, Innen: Gummi) - Das\r\nInnere Gummi ist hitzebeständig (max. 120 Grad) - Das äußere Neoprene ist resistent gegenüber Öl\r\n(max. 100 Grad) - Benzinresistent, nicht E10 tauglich - Verstärkte Ausführung [...]\r\nLieferzeit: 3-4 Tage\r\n44245 10,00 EUR 20\r\n1 x Stahlflex Bremsleitung mit ABE für BMW R 100/80 GS, ab Sep 90\r\nStahlflexbremsleitungen mit ABE / Teilegutachten Konstanter Druckpunkt der Bremse Langjährig\r\ngleichbleibende Bremsleistung Bewährt auch unter härtesten Bedingungen. Bestehend aus: 1 Leitung\r\n800mm, Dichtringe. Für BMW R 2V Boxer Modelle R 80GS, R 80GS PD, R 100GS, R 100GS PD, R\r\n80GS [...]\r\nLieferzeit: 3-4 Tage\r\n44064 39,41 EUR 39\r\nZwischensumme: 198\r\nPaketversand (Versand nach CH: (2.62 kg)): 35\r\nSumme, netto: 233,\r\nSumme: 233,\r\nmit freundlichen Grüßen\r\nKEC GmbH\r\nBoxxerparts\r\nPoststr. 2\r\nD-35794 Mengerskirchen\r\nWiderrufsbelehrung und Muster-Widerrufsformular für Verbraucher\r\n";

    // Text layer of a scanned paper invoice after OCRmyPDF/Tesseract, as
    // pdfium extracts it (customer address, bank and footer trimmed). Note
    // the "1|83" table rule, the invoice number sharing the RECHNUNG line and
    // no "CHF" anywhere.
    const SCANNED_262462: &str = "Mark Huggett GmbH \nBMW Motorrad Classic \nLieferadresse/Dellvery address \nBMW \nCLASSIC \nRECHNUNG 262462 \nBst-Nr./Order-no.: 2026-09-21-10016 \nKunde/Customer: 8810 \nTrackingnummer: 99.37.126432.00029845 \nShipping: Priority Gew./kg: 1.620 \nBearbeiter/Processor: Susan Vardi Holderbank, den 22.9.2026 Seite 1 \nAnz, Ersatzteilnummer Artikelbezeichnung StW/Preis | Rab Betrag \n1|83 30 0 401 758 Sternmutterschlüssel (Nr. 180600) 51.62 51.62 \nWarenwert Verpackung Porto Total vorMWSt . MWSt. 8.1 % \n51.62 0.00 10.22 61.84 5.01 \nKeine Garantie auf elektronische Bauteile. MWST: CHE-102.220.642 MWST \nGP-ID: 1000943745 \nEORI: DE714612052877641 \nFax CHE-102.220.642 MWST CHE-102.220.842 MWST CHE-102.220.842 MWST \n+41 82 887 60 21 0E714612052877641 0E714612052877841 DE714812052877641 \n";

    #[test]
    fn parses_ocr_scanned_invoice() {
        let parsed = parse_layout(SCANNED_262462);
        assert_eq!(parsed.supplier_kind, Supplier::Huggett);
        assert_eq!(parsed.items.len(), 1);
        let item = &parsed.items[0];
        assert_eq!(item.quantity, 1);
        assert_eq!(item.part_number, "83 30 0 401 758");
        assert_eq!(item.name, "Sternmutterschlüssel (Nr. 180600)");
        assert_eq!(item.unit_price, Some(51.62));
        assert_eq!(item.line_total, Some(51.62));
        assert_eq!(parsed.invoice_number.as_deref(), Some("262462"));
        assert_eq!(parsed.invoice_date.as_deref(), Some("2026-09-22"));
        assert_eq!(parsed.currency.as_deref(), Some("CHF"));
    }

    // Tesseract output (deu+eng, --psm 4) of the same invoice scanned
    // without a text layer, as the server-side OCR produces it: the table
    // rule vanished between qty and part number, or became an extra "1".
    #[test]
    fn parses_server_ocr_output() {
        for row in [
            "183 30 0 401 758 Sternmutterschlissel (Nr. 180600) 51.62 51.62",
            "1183 30 0 401 758 Sternmutterschliissel (Nr. 180600) 51.62 51.62",
            "183300401758  Sternmutterschlissel (Nr. 180600) 51.62 51.62",
        ] {
            let text = format!(
                "RECHNUNG 262462\nHolderbank den 22.9.2026 Seite 1\n{}\nMWST: CHE-102.220.642 MWST\n",
                row
            );
            let parsed = parse_layout(&text);
            assert_eq!(parsed.items.len(), 1, "{}", row);
            assert_eq!(parsed.items[0].quantity, 1, "{}", row);
            assert_eq!(parsed.items[0].part_number, "83 30 0 401 758", "{}", row);
            assert_eq!(parsed.items[0].line_total, Some(51.62));
        }
    }

    #[test]
    fn layout_line_values_override_llm_amounts() {
        let fallback = parse_layout(SCANNED_262462);
        let mut llm = fallback.clone();
        llm.items[0].part_number = "83300401758".to_string();
        llm.items[0].line_total = Some(61.84);
        llm.items[0].name = "Sternmutterschlüssel".to_string();
        prefer_layout_line_values(&mut llm, &fallback);
        assert_eq!(llm.items[0].line_total, Some(51.62));
        assert_eq!(llm.items[0].part_number, "83 30 0 401 758");
        // Names stay the model's — only the row's numbers are authoritative.
        assert_eq!(llm.items[0].name, "Sternmutterschlüssel");
    }

    #[test]
    fn tolerates_ocr_noise_in_item_lines() {
        let text = "RECHNUNG 262462\n2 [ 8330 0 401758 Sternmutter 1’051.62 2’103.24 \n3 ! 12 11 1 351 564 Kondensator 13.70 41.10\n";
        let parsed = parse_invoice_text(text);
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].quantity, 2);
        assert_eq!(parsed.items[0].part_number, "83 30 0 401 758");
        assert_eq!(parsed.items[0].unit_price, Some(1051.62));
        assert_eq!(parsed.items[1].part_number, "12 11 1 351 564");
    }

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
        assert_eq!(parsed.supplier_kind, Supplier::Huggett);
        assert_eq!(parsed.document_kind, DocumentKind::Invoice);
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
    fn layout_dispatch_recognizes_both_suppliers() {
        assert_eq!(detect_supplier(INVOICE_242511), Supplier::Huggett);
        assert_eq!(detect_supplier(ORDER_72669), Supplier::Boxxerparts);
        assert_eq!(detect_supplier("Irgendeine Rechnung"), Supplier::Unknown);
        assert_eq!(parse_layout(INVOICE_242511).items.len(), 4);
        assert_eq!(parse_layout(ORDER_72669).items.len(), 6);
    }

    #[test]
    fn parses_all_boxxerparts_order_lines() {
        let parsed = parse_boxxerparts_text(ORDER_72669);
        assert_eq!(parsed.items.len(), 6);

        let numbers: Vec<&str> = parsed
            .items
            .iter()
            .map(|i| i.part_number.as_str())
            .collect();
        assert_eq!(
            numbers,
            [
                "BXP-96065",
                "BXP-44555",
                "BXP-44545",
                "BXP-91095",
                "BXP-44245",
                "BXP-44064"
            ]
        );
        let quantities: Vec<i64> = parsed.items.iter().map(|i| i.quantity).collect();
        assert_eq!(quantities, [2, 1, 2, 2, 2, 1]);

        let valve = &parsed.items[0];
        assert_eq!(
            valve.name,
            "Winkelventil 8,3 mm. für BMW R 100 / 80 GS R Kreuzspeichen Felgen"
        );
        assert_eq!(valve.supplier_article_no.as_deref(), Some("96065"));
        assert_eq!(valve.unit_price, Some(4.12));
        // Line totals are clipped in the print — computed, not read.
        assert_eq!(valve.line_total, Some(8.24));
        let description = valve.description.as_deref().unwrap();
        assert!(description.starts_with("ALU Winkel Ventil 90 Grad"));
        assert!(description.ends_with("Anzugs-Drehmoment: 7 - 10"));
        assert!(!description.contains("Lieferzeit"));

        // The closing line of the Sternmutter precedes its Lieferzeit line.
        let nut = &parsed.items[2];
        assert_eq!(nut.unit_price, Some(24.79));
        assert_eq!(nut.line_total, Some(49.58));

        let hose = &parsed.items[4];
        assert_eq!(hose.name, "Neopren Kraftstoffschlauch 6.0mm. - 1m.");
        assert_eq!(hose.line_total, Some(20.00));
    }

    #[test]
    fn boxxerparts_order_metadata_and_oem_numbers() {
        let parsed = parse_boxxerparts_text(ORDER_72669);
        assert_eq!(parsed.supplier.as_deref(), Some("Boxxerparts"));
        assert_eq!(parsed.supplier_kind, Supplier::Boxxerparts);
        assert_eq!(parsed.document_kind, DocumentKind::Order);
        assert_eq!(parsed.invoice_number.as_deref(), Some("72669"));
        assert_eq!(parsed.invoice_date.as_deref(), Some("2026-09-21"));
        assert_eq!(parsed.currency.as_deref(), Some("EUR"));

        // The Wehrle regulator cites its OEM number in the description.
        let regulator = &parsed.items[1];
        assert_eq!(
            regulator.oem_part_numbers,
            vec!["12 32 1 244 409".to_string()]
        );
        assert!(parsed.items[0].oem_part_numbers.is_empty());
    }

    #[test]
    fn item_without_closing_line_is_kept_for_review() {
        let text =
            "boxxerparts\n1 x Erstes Teil\nBeschreibung\n2 x Zweites Teil\n12345 1,00 EUR 1\n";
        let parsed = parse_boxxerparts_text(text);
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].part_number, "");
        assert_eq!(parsed.items[0].supplier_article_no, None);
        assert_eq!(parsed.items[0].description.as_deref(), Some("Beschreibung"));
        assert_eq!(parsed.items[1].part_number, "BXP-12345");
    }

    #[test]
    fn extracts_oem_numbers_in_any_spacing() {
        assert_eq!(
            extract_oem_part_numbers("vom Erstausrüster BMW 12321244409 Wehrle 55990002"),
            vec!["12 32 1 244 409".to_string()]
        );
        assert_eq!(
            extract_oem_part_numbers("BMW Teilenummer: 18 12 1 234 567 und 18121234567"),
            vec!["18 12 1 234 567".to_string()]
        );
        // Order numbers and phone numbers are not 11 digits.
        assert!(extract_oem_part_numbers("Bestellung Nr: 72669 Fon: +49 6476 419401").is_empty());
    }

    #[test]
    fn normalizes_part_numbers() {
        assert_eq!(normalize_part_number("61 31 2 300 383"), "61312300383");
        assert_eq!(normalize_part_number("61-31-2-300-383"), "61312300383");
        assert_eq!(normalize_part_number("BXP-44555"), "BXP44555");
        assert!(is_bmw_part_number("61 31 2 300 383"));
        assert!(!is_bmw_part_number("12345"));
        assert!(!is_bmw_part_number("BXP-44555"));
    }

    #[test]
    fn condense_drops_footer_noise() {
        let text = "1 61 31 1 244 708 Schalter 57.80 57.80\nBankkonto PostFinance\nIBAN CH49\nTelefon +41\nWiderrufsbelehrung\n";
        let condensed = condense_invoice_text(text);
        assert!(condensed.contains("Schalter"));
        assert!(!condensed.contains("IBAN"));
        assert!(!condensed.contains("Telefon"));
        assert!(!condensed.contains("Widerruf"));
    }

    #[test]
    fn llm_json_without_new_fields_still_parses() {
        // The LLM schema predates description/oem fields — they must default.
        let json = r#"{"supplier":null,"invoiceNumber":"1","invoiceDate":null,"currency":"CHF","items":[{"quantity":1,"partNumber":"12 11 1 351 564","name":"Kondensator","unitPrice":1.0,"lineTotal":1.0}]}"#;
        let parsed: ParsedInvoice = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.items[0].description, None);
        assert!(parsed.items[0].oem_part_numbers.is_empty());
        assert_eq!(parsed.supplier_kind, Supplier::Unknown);
        assert_eq!(parsed.document_kind, DocumentKind::Invoice);
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
