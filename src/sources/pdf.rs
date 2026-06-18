//! PDF text extraction via `pdf-extract` (pure Rust, no native dependencies).
//!
//! Returns the raw extracted text; normalization and the "no usable text" gate live in
//! `sources::load_document`.

use std::path::Path;

use anyhow::{Context, Result};

/// Extract the plain-text layer of a PDF. Scanned/image PDFs yield little or no text — that case is
/// detected (and reported) by the alpha-ratio gate in `load_document`, not here.
pub fn extract(path: &Path) -> Result<String> {
    pdf_extract::extract_text(path)
        .with_context(|| format!("failed to read text from PDF {}", path.display()))
}
