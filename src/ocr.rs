//! Text recognition for scanned supplier documents (paper invoices scanned to
//! an image-only PDF, or photographed). Pages are rendered/decoded to
//! grayscale at a normalized size and handed to the Tesseract CLI.
//!
//! No image cleanup on purpose: erasing the invoice table rules was tried and
//! made recognition worse (spacing and decimal points got lost). The OCR
//! quirks that remain — a rule read as "|" or an extra "1" — are handled by
//! the layout parser instead.
//!
//! Tesseract is an external binary (installed in the Docker image via
//! `tesseract-ocr` + `tesseract-ocr-deu`); `TESSERACT_CMD` overrides the
//! executable and `OCR_LANGS` the language models (default `deu+eng`). A
//! missing binary is reported as "OCR unavailable", never a panic.

use std::{path::PathBuf, process::Stdio, sync::OnceLock, time::Duration};

use image::{DynamicImage, GrayImage, ImageDecoder, ImageReader};
use pdfium_render::prelude::*;

/// Render resolution for PDF pages: Tesseract's sweet spot for body text.
const RENDER_DPI: f32 = 300.0;
/// Scans longer than this are almost certainly not a single invoice, and each
/// page costs seconds of CPU.
pub const MAX_OCR_PAGES: usize = 5;
const TESSERACT_TIMEOUT: Duration = Duration::from_secs(90);
/// Documents are A4/letter; page images are scaled into this width band
/// (≈200-420 dpi) so Tesseract sees glyphs at a size it handles well no
/// matter whether the input was a 72-dpi phone JPEG or an oversized render.
const MIN_PAGE_WIDTH: u32 = 1650;
const MAX_PAGE_WIDTH: u32 = 3500;
const TARGET_PAGE_WIDTH: u32 = 2480;
const A4_WIDTH_INCHES: f32 = 8.27;

/// A page bitmap ready for Tesseract plus its effective resolution.
pub struct OcrPage {
    image: GrayImage,
    dpi: u32,
}

#[derive(Debug)]
pub enum OcrError {
    /// Tesseract is not installed (or `TESSERACT_CMD` points nowhere).
    Unavailable(String),
    /// The input could not be decoded/rendered.
    BadInput(String),
    /// Tesseract ran but failed or timed out.
    Failed(String),
}

/// Render the pages of a PDF to grayscale bitmaps ready for OCR. Blocking
/// (pdfium) — call from `spawn_blocking`.
pub fn render_pdf_pages(data: &[u8]) -> Result<Vec<OcrPage>, OcrError> {
    let pdfium = crate::pdfium_lib::shared_pdfium().map_err(OcrError::Unavailable)?;
    let document = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| OcrError::BadInput(format!("PDF konnte nicht gelesen werden: {:?}", e)))?;
    let mut pages = Vec::new();
    for page in document.pages().iter().take(MAX_OCR_PAGES) {
        let width_px = ((page.width().value / 72.0 * RENDER_DPI).round() as u32)
            .clamp(MIN_PAGE_WIDTH, MAX_PAGE_WIDTH) as i32;
        let bitmap = page
            .render_with_config(&PdfRenderConfig::new().set_target_width(width_px))
            .map_err(|e| OcrError::BadInput(format!("PDF-Seite nicht darstellbar: {:?}", e)))?;
        let image = bitmap
            .as_image()
            .map_err(|e| OcrError::BadInput(format!("PDF-Seite nicht darstellbar: {:?}", e)))?;
        pages.push(prepare(image));
    }
    Ok(pages)
}

/// Decode an uploaded photo/scan (JPEG, PNG, WebP), honouring the EXIF
/// orientation phones write instead of rotating pixels. Blocking.
pub fn decode_image(data: &[u8]) -> Result<OcrPage, OcrError> {
    let bad = |e: image::ImageError| OcrError::BadInput(format!("Bild nicht lesbar: {}", e));
    let mut decoder = ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| OcrError::BadInput(format!("Bild nicht lesbar: {}", e)))?
        .into_decoder()
        .map_err(bad)?;
    let orientation = decoder.orientation().map_err(bad)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(bad)?;
    image.apply_orientation(orientation);
    Ok(prepare(image))
}

fn prepare(image: DynamicImage) -> OcrPage {
    let mut gray = image.into_luma8();
    if !(MIN_PAGE_WIDTH..=MAX_PAGE_WIDTH).contains(&gray.width()) {
        let height = (gray.height() as u64 * TARGET_PAGE_WIDTH as u64 / gray.width() as u64) as u32;
        gray = image::imageops::resize(
            &gray,
            TARGET_PAGE_WIDTH,
            height.max(1),
            image::imageops::FilterType::Triangle,
        );
    }
    OcrPage {
        dpi: (gray.width() as f32 / A4_WIDTH_INCHES).round() as u32,
        image: gray,
    }
}

/// OCR prepared pages in order and join their text with newlines.
pub async fn recognize_pages(pages: Vec<OcrPage>) -> Result<String, OcrError> {
    let mut text = String::new();
    for page in pages {
        text.push_str(&recognize(page).await?);
        text.push('\n');
    }
    Ok(text)
}

async fn recognize(page: OcrPage) -> Result<String, OcrError> {
    // Tesseract reads files, not stdin bytes, reliably across versions.
    let path: PathBuf = std::env::temp_dir().join(format!("ocr-{}.png", uuid::Uuid::new_v4()));
    let write_path = path.clone();
    let OcrPage { image, dpi } = page;
    tokio::task::spawn_blocking(move || image.save(&write_path))
        .await
        .map_err(|e| OcrError::Failed(format!("OCR task panicked: {}", e)))?
        .map_err(|e| OcrError::Failed(format!("OCR-Zwischenbild: {}", e)))?;

    let result = run_tesseract(&path, dpi).await;
    let _ = tokio::fs::remove_file(&path).await;
    result
}

async fn run_tesseract(path: &std::path::Path, dpi: u32) -> Result<String, OcrError> {
    let command = std::env::var("TESSERACT_CMD").unwrap_or_else(|_| "tesseract".to_string());
    let langs = std::env::var("OCR_LANGS").unwrap_or_else(|_| "deu+eng".to_string());
    let child = tokio::process::Command::new(&command)
        .arg(path)
        .arg("stdout")
        .args(["-l", &langs])
        // psm 4: one column of variable-size text, read line by line — keeps
        // each invoice row (qty, number, name, prices) on one line, where the
        // default auto layout tends to split table columns into blocks.
        .args(["--psm", "4"])
        .args(["--dpi", &dpi.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| OcrError::Unavailable(format!("{}: {}", command, e)))?;
    let output = tokio::time::timeout(TESSERACT_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| OcrError::Failed("Texterkennung hat zu lange gedauert".to_string()))?
        .map_err(|e| OcrError::Failed(format!("tesseract: {}", e)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // A missing language model is a deployment problem, not bad input.
        if stderr.contains("Failed loading language") || stderr.contains("Error opening data file")
        {
            return Err(OcrError::Unavailable(stderr.trim().to_string()));
        }
        return Err(OcrError::Failed(format!(
            "tesseract exit {}: {}",
            output.status,
            stderr.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Undo the typical OCR artefacts of invoice tables, line by line. Only
/// applied to recognized text (never to a PDF's own text layer):
/// - table rules read as stand-alone "|", "I", "l", "[", "]", "!", "{", "}";
/// - a rule glued to a cell: "|83 30 …", "lSternmutter…" (rule + capital),
///   "51.62|" / "51.62'";
/// - a decimal point split off: "51 .62".
pub fn clean_ocr_text(text: &str) -> String {
    static RULES: OnceLock<[regex::Regex; 6]> = OnceLock::new();
    let [split_decimal, standalone, before_digit, before_capital, after_amount, spaces] = RULES
        .get_or_init(|| {
            [
                regex::Regex::new(r"(\d) \.(\d{2})\b").expect("static regex"),
                // Stand-alone rule glyphs between whitespace (or line edges).
                regex::Regex::new(r"(^|\s)[|Il\[\]!{}]+(\s|$)").expect("static regex"),
                regex::Regex::new(r"(^|\s)[|\[\]!{}](\d)").expect("static regex"),
                regex::Regex::new(r"(^|\s)[|l\[\]!{}]([A-ZÄÖÜ][a-zäöüß])").expect("static regex"),
                regex::Regex::new(r"(\d\.\d{2})[|'’\]!]+(\s|$)").expect("static regex"),
                regex::Regex::new(r" {2,}").expect("static regex"),
            ]
        });
    text.lines()
        .map(|line| {
            let mut line = split_decimal.replace_all(line, "$1.$2").into_owned();
            // Applied twice: adjacent matches share their whitespace.
            for _ in 0..2 {
                line = standalone.replace_all(&line, "$1$2").into_owned();
            }
            line = before_digit.replace_all(&line, "$1$2").into_owned();
            line = before_capital.replace_all(&line, "$1$2").into_owned();
            line = after_amount.replace_all(&line, "$1$2").into_owned();
            spaces.replace_all(&line, " ").into_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_table_rule_artefacts() {
        // Actual Tesseract outputs of the same Huggett row in different
        // page contexts.
        let cases = [
            "1 |83 30 0 401 758 lSternmutterschlüssel (Nr. 180600} I 51 .62' I 51.62",
            "1 |83 30 0 401 758 ISternmutterschlüssel (Nr. 180600) | 51 .62| I 51.62",
            "1 |83 30 0 401 758 |3temmutterschlüssel (Nr. 180600) | 51 .62| | 51.62",
        ];
        for case in cases {
            let cleaned = clean_ocr_text(case);
            assert!(cleaned.starts_with("1 83 30 0 401 758 "), "{}", cleaned);
            assert!(cleaned.ends_with(" 51.62 51.62"), "{}", cleaned);
            assert!(!cleaned.contains('|'), "{}", cleaned);
        }
        assert_eq!(
            clean_ocr_text(
                "1 |83 30 0 401 758 lSternmutterschlüssel (Nr. 180600} I 51 .62' I 51.62"
            ),
            "1 83 30 0 401 758 Sternmutterschlüssel (Nr. 180600} 51.62 51.62"
        );
        // Ordinary text is left alone.
        let plain = "RECHNUNG 262462\nHolderbank, den 22.9.2026 Seite 1\nIch bin l33t";
        assert_eq!(clean_ocr_text(plain), plain);
    }
}
