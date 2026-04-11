use std::path::PathBuf;

use anyhow::Result;
use tracing::error;

use crate::api::LlmClient;
use crate::db::DbPool;
use crate::db::files as db_files;
use crate::error::AppError;
use crate::tui::SharedState;
use crate::tui::state::ProcessingStatus;

use super::page::process_page;

pub async fn process_pdf(
    pool: &DbPool,
    client: &mut LlmClient,
    file_id: i64,
    pdf_path: PathBuf,
    state: SharedState,
    start_page: i32,
    end_page: i32,
    max_parallel: i32,
) -> Result<(), AppError> {
    let file = db_files::get_file_by_id(pool, file_id)
        .await?
        .ok_or_else(|| AppError::Config("File not found".to_string()))?;

    let total_pages = file.total_pages;
    let effective_start = if start_page > 0 { start_page } else { 1 };
    let effective_end = if end_page > 0 { end_page } else { total_pages };
    let mut current_page = effective_start;
    let mut pending_incomplete: Option<String> = None;
    let mut total_questions_processed: i32 = 0;

    {
        let mut s = state.write().await;
        s.processing_status = ProcessingStatus::Processing;
        s.add_info(format!(
            "Starting processing of {} (pages {}-{} of {})",
            file.filename, effective_start, effective_end, total_pages
        ));
    }

    if effective_start > effective_end {
        {
            let mut s = state.write().await;
            s.processing_status = ProcessingStatus::Completed;
            s.add_info("No pages to process");
        }
        return Ok(());
    }

    while current_page <= effective_end {
        let page_num = current_page;
        let is_last_page = page_num >= effective_end;

        match process_page(
            pool,
            client,
            file_id,
            &pdf_path,
            page_num,
            &state,
            pending_incomplete.take(),
            is_last_page,
            max_parallel,
        )
        .await
        {
            Ok(result) => {
                total_questions_processed = result.questions_processed;
                pending_incomplete = result.pending_incomplete;
            }
            Err(e) => {
                error!("Page {} failed: {}", page_num, e);
            }
        }

        {
            let mut s = state.write().await;
            s.total_questions = total_questions_processed;
            let current_total = s.total_questions;
            s.add_info(format!(
                "Extracted {current_total} questions so far (page {page_num}/{effective_end})"
            ));
        }

        current_page += 1;
    }

    {
        let mut s = state.write().await;
        s.processing_status = ProcessingStatus::Completed;
        s.current_batch = None;
        s.add_info("Processing completed");
    }

    Ok(())
}
