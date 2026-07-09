//! Process-wide Pdfium instance. Pdfium's library binding is global and can
//! only be initialized ONCE per process — a second `Pdfium::bind_to_library`
//! fails with `PdfiumLibraryBindingsAlreadyInitialized`, even after the first
//! instance is dropped. Every consumer (document previews, invoice parsing)
//! must therefore share this lazily-initialized singleton. The crate's
//! default `thread_safe` feature serializes the actual FPDF calls.

use std::sync::{Mutex, OnceLock};

use pdfium_render::prelude::*;

static PDFIUM: OnceLock<Pdfium> = OnceLock::new();
static INIT_LOCK: Mutex<()> = Mutex::new(());

pub fn shared_pdfium() -> Result<&'static Pdfium, String> {
    if let Some(pdfium) = PDFIUM.get() {
        return Ok(pdfium);
    }
    // Serialize initialization: a lost race on bind_to_library would report
    // AlreadyInitialized even though a usable instance exists.
    let _guard = INIT_LOCK.lock().expect("pdfium init lock poisoned");
    if let Some(pdfium) = PDFIUM.get() {
        return Ok(pdfium);
    }
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
        .or_else(|_| Pdfium::bind_to_system_library())
        .map_err(|e| format!("Could not bind to Pdfium library: {}", e))?;
    let _ = PDFIUM.set(Pdfium::new(bindings));
    Ok(PDFIUM.get().expect("PDFIUM was just set"))
}
