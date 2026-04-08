pub mod parser;

use std::path::Path;
use tracing::{error, info};

use crate::error::AppError;

pub fn get_pdf_page_count(path: &Path) -> Result<usize, AppError> {
    let doc = pdf_extract::Document::load(path)?;
    let pages = doc.get_pages();
    let count = pages.keys().len();
    info!("PDF {} has {} pages", path.display(), count);
    Ok(count)
}

pub fn extract_page_text(
    path: &Path,
    page_numbers: &[i32],
) -> Result<Vec<(i32, String)>, AppError> {
    let doc = pdf_extract::Document::load(path)?;
    let mut results = Vec::new();

    for &page_num in page_numbers {
        match doc.extract_text(&[page_num.cast_unsigned()]) {
            Ok(text) => {
                let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
                results.push((page_num, cleaned));
                info!("Extracted text from page {}", page_num);
            }
            Err(e) => {
                error!("Failed to extract page {}: {}", page_num, e);
                return Err(AppError::PdfExtract(format!("Page {page_num}: {e}")));
            }
        }
    }

    Ok(results)
}
