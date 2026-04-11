use std::path::Path;
use std::path::PathBuf;
use std::thread;

use anyhow::Result;
use tracing::info;

use crate::api::LlmClient;
#[allow(unused_imports)]
use crate::cli;
use crate::db::DbPool;
use crate::db::files as db_files;
use crate::db::pages as db_pages;
use crate::db::questions as db_questions;
use crate::error::AppError;
use crate::pdf as pdf_mod;
use crate::tui::{
    SharedState,
    state::{FileInfo, PageStatusInfo, ProcessingStatus},
    tui_loop,
};

use super::pdf::process_pdf;

pub async fn run_tui(
    pool: DbPool,
    pdf_path: Option<PathBuf>,
    start_page: i32,
    end_page: i32,
    process_args: Option<crate::cli::ProcessArgs>,
    settings: crate::db::settings::Settings,
) -> Result<(), AppError> {
    let _process_args = process_args.unwrap_or_default();
    let state = crate::tui::create_shared_state(settings.clone());

    let state_for_tui = state.clone();
    let tui_handle = tokio::spawn(async move {
        tui_loop::run_tui_loop(state_for_tui).await;
    });

    let state_for_thread = state.clone();
    let pool_for_thread = pool.clone();
    let pdf_path_clone = pdf_path.clone();
    let settings_clone = settings.clone();

    let _processing_thread = thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async move {
            if let Some(pdf_path) = pdf_path_clone {
                let filename = pdf_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                if let Err(e) = load_and_process_file(
                    &pool_for_thread,
                    Some(pdf_path),
                    filename,
                    start_page,
                    end_page,
                    &state_for_thread,
                    &settings_clone,
                )
                .await
                {
                    let mut s = state_for_thread.write().await;
                    s.processing_status = ProcessingStatus::Error;
                    s.add_error(format!("File loading failed: {}", e));
                }
            } else {
                let existing_files = db_files::get_all_files(&pool_for_thread).await.unwrap_or_default();
                if existing_files.is_empty() {
                    let mut s = state_for_thread.write().await;
                    s.processing_status = ProcessingStatus::Error;
                    s.add_error("No files found to retry".to_string());
                } else {
                    let file = existing_files.into_iter().max_by_key(|f| f.id).unwrap();

                    if let Err(e) = load_and_process_file(
                        &pool_for_thread,
                        None,
                        file.filename,
                        0,
                        0,
                        &state_for_thread,
                        &settings_clone,
                    )
                    .await
                    {
                        let mut s = state_for_thread.write().await;
                        s.processing_status = ProcessingStatus::Error;
                        s.add_error(format!("File loading failed: {}", e));
                    }
                }
            }

            let mut s = state_for_thread.write().await;
            s.shutdown = true;
        });
    });

    let _ = tui_handle.await;

    Ok(())
}

async fn load_and_process_file(
    pool: &DbPool,
    pdf_path: Option<PathBuf>,
    filename: String,
    start_page: i32,
    end_page: i32,
    state: &SharedState,
    settings: &crate::db::settings::Settings,
) -> Result<(), AppError> {
    let pdf_path = pdf_path.unwrap_or_else(|| PathBuf::from(&filename));

    let file_id = load_or_create_file(pool, &pdf_path, &filename).await?;
    db_pages::reset_processing_pages(pool, file_id).await?;

    init_file_state(pool, file_id, state.clone()).await?;

    run_processing_task(
        pool.clone(),
        pdf_path,
        state.clone(),
        file_id,
        start_page,
        end_page,
        settings.clone(),
    )
    .await
}

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

async fn run_processing_task(
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

    let model_path = settings
        .model_path
        .as_ref()
        .map(Path::new)
        .ok_or_else(|| AppError::Config("model_path not configured".to_string()))?;

    let context_size = settings.context_size;

    let devices = settings.devices.as_ref().map(|d| {
        d.split(',')
            .filter_map(|s| s.trim().parse::<usize>().ok())
            .collect::<Vec<usize>>()
    });

    let mut client = LlmClient::new(model_path, context_size, devices, settings.think)?;

    if let Err(e) = process_pdf(
        &pool_clone,
        &mut client,
        file_id,
        pdf_path_clone,
        state_clone,
        start_page,
        end_page,
        settings.max_parallel_questions,
    )
    .await
    {
        let mut s = state.write().await;
        s.processing_status = ProcessingStatus::Error;
        s.add_error(format!("Processing failed: {}", e));
    }

    Ok(())
}
