use std::path::PathBuf;

use anyhow::Result;
use tracing::{error, info};

use crate::api::LlmClient;
use crate::db::DbPool;
use crate::db::files as db_files;
use crate::db::pages as db_pages;
use crate::db::questions as db_questions;
use crate::db::settings as db_settings;
use crate::error::AppError;
use crate::pdf as pdf_mod;
use crate::tui::{
    SharedState,
    state::{FileInfo, PageStatusInfo},
    tui_loop,
};

use super::pdf::process_pdf;

pub async fn run_tui(
    pool: DbPool,
    pdf_path: PathBuf,
    start_page: i32,
    end_page: i32,
    command: Option<crate::cli::Command>,
    state: SharedState,
) -> Result<(), AppError> {
    let settings = db_settings::get_settings(&pool).await?;

    let filename = pdf_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Load or create file
    let file_id = load_or_create_file(&pool, &pdf_path, &filename).await?;

    // Reset processing state
    db_pages::reset_processing_pages(&pool, file_id).await?;

    // Handle retry command
    if let Some(crate::cli::Command::RetryFailed) = command {
        let failed_pages = db_pages::get_failed_pages(&pool, file_id).await?;
        if !failed_pages.is_empty() {
            let mut s = state.write().await;
            s.add_info(format!("Retrying {} failed pages", failed_pages.len()));
            db_pages::reset_pending_pages(&pool, file_id).await?;
        }
    }

    // Load file info into state
    init_file_state(&pool, file_id, state.clone()).await?;

    // Spawn processing task
    spawn_processing_task(
        pool, pdf_path, state, file_id, start_page, end_page, settings,
    )
    .await
}

/// Load existing file or create new one
async fn load_or_create_file(
    pool: &DbPool,
    pdf_path: &std::path::Path,
    filename: &str,
) -> Result<i64, AppError> {
    let existing_file = db_files::get_file_by_name(pool, filename).await?;

    if let Some(f) = existing_file {
        let status = db_files::get_file_status(pool, f.id).await?;
        if status == "completed" {
            info!("File already processed: {}", filename);
            return Err(AppError::Config("File already processed".to_string()));
        }
        Ok(f.id)
    } else {
        let page_count = pdf_mod::get_pdf_page_count(pdf_path)?;
        let file = db_files::create_file(pool, filename, page_count as i32).await?;
        db_pages::create_pages_for_file(pool, file.id, page_count as i32).await?;
        Ok(file.id)
    }
}

/// Initialize state with file and page information
async fn init_file_state(pool: &DbPool, file_id: i64, state: SharedState) -> Result<(), AppError> {
    let file = db_files::get_file_by_id(pool, file_id)
        .await?
        .ok_or_else(|| AppError::Config("File not found".to_string()))?;

    let statuses = db_pages::get_page_statuses(pool, file_id).await?;
    let page_statuses: Vec<PageStatusInfo> = statuses
        .into_iter()
        .map(|ps| PageStatusInfo {
            page_number: ps.page_number,
            status: ps.status,
            error_message: ps.error_message,
        })
        .collect();

    let question_count = db_questions::get_questions_count(pool, file_id).await?;
    let file_status = db_files::get_file_status(pool, file_id).await?;

    let mut s = state.write().await;
    s.set_file(Some(FileInfo {
        id: file.id,
        filename: file.filename,
        total_pages: file.total_pages,
        status: file_status,
    }));
    s.set_page_statuses(page_statuses);
    s.total_questions = question_count;

    Ok(())
}

/// Spawn the PDF processing task and run TUI
async fn spawn_processing_task(
    pool: DbPool,
    pdf_path: PathBuf,
    state: SharedState,
    file_id: i64,
    start_page: i32,
    end_page: i32,
    settings: crate::db::settings::Settings,
) -> Result<(), AppError> {
    let pool_clone = pool.clone();
    let pdf_path_clone = pdf_path.clone();
    let state_clone = state.clone();
    let start_page_clone = start_page;
    let end_page_clone = end_page;

    let client = LlmClient::new(
        settings.api_model.clone(),
        settings.max_retries,
        settings.retry_delay_ms,
        settings.retry_multiplier,
        settings.think,
    );

    tokio::spawn(async move {
        if let Err(e) = process_pdf(
            &pool_clone,
            &client,
            file_id,
            pdf_path_clone,
            state_clone,
            start_page_clone,
            end_page_clone,
            settings.max_parallel_questions,
        )
        .await
        {
            error!("Processing error: {}", e);
        }
    });

    tui_loop::run_tui_loop(state).await;

    Ok(())
}
