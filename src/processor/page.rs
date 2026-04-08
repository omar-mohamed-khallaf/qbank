use std::borrow::Cow;

use anyhow::Result;
use tracing::{error, info};

use crate::api::LlmClient;
use crate::db::DbPool;
use crate::error::AppError;
use crate::pdf as pdf_mod;
use crate::tui::SharedState;

use super::question::process_single_question;
use super::update_page_status;

async fn process_questions_batch(
    pool: &DbPool,
    client: &LlmClient,
    file_id: i64,
    page_num: i32,
    questions: Vec<&str>,
    state: &SharedState,
    max_parallel: i32,
) -> (i32, bool) {
    let mut total_processed: i32 = 0;
    let mut failed = false;

    if max_parallel > 0 {
        let mut processed = 0;
        for chunk in questions.chunks(max_parallel as usize) {
            let results =
                futures::future::join_all(chunk.iter().map(|q| {
                    let q = q.to_string();
                    let pool = pool.clone();
                    async move {
                        process_single_question(&pool, client, file_id, page_num, &q, state).await
                    }
                }))
                .await;

            processed += results.iter().filter(|r| r.is_ok()).count() as i32;
            if results.iter().any(std::result::Result::is_err) {
                failed = true;
                break;
            }
        }
        total_processed = processed;
    } else {
        for question_text in &questions {
            if process_single_question(pool, client, file_id, page_num, question_text, state)
                .await
                .is_ok()
            {
                total_processed += 1;
            } else {
                failed = true;
                break;
            }
        }
    }

    (total_processed, failed)
}

pub struct PageResult {
    pub questions_processed: i32,
    pub pending_incomplete: Option<String>,
}

pub async fn process_page(
    pool: &DbPool,
    client: &LlmClient,
    file_id: i64,
    pdf_path: &std::path::Path,
    page_num: i32,
    state: &SharedState,
    pending_incomplete: Option<String>,
    is_last_page: bool,
    max_parallel: i32,
) -> Result<PageResult, AppError> {
    let mut new_pending_incomplete: Option<String> = None;

    // Mark page as processing in both DB and state
    update_page_status(pool, file_id, page_num, state, "processing", None).await?;

    // Extract page text
    let page_text = match pdf_mod::extract_page_text(pdf_path, &[page_num]) {
        Ok(texts) => match texts.first() {
            Some((_, t)) if !t.trim().is_empty() => t.clone(),
            _ => {
                error!("Empty page text for page {}", page_num);
                update_page_status(
                    pool,
                    file_id,
                    page_num,
                    state,
                    "failed",
                    Some("Empty page text"),
                )
                .await?;
                return Ok(PageResult {
                    questions_processed: 0,
                    pending_incomplete: None,
                });
            }
        },
        Err(e) => {
            error!("PDF extraction error for page {}: {}", page_num, e);
            update_page_status(
                pool,
                file_id,
                page_num,
                state,
                "failed",
                Some(&e.to_string()),
            )
            .await?;
            return Err(e);
        }
    };

    // Combine with pending incomplete from previous page
    let text_with_incomplete = if let Some(ref inc) = pending_incomplete {
        format!("{inc}\n{page_text}")
    } else {
        page_text.clone()
    };

    // Split questions
    let (questions, incomplete) =
        pdf_mod::parser::split_questions_by_anchors(&text_with_incomplete);

    info!("Page {}: found {} questions", page_num, questions.len());

    // Handle no questions found
    if questions.is_empty() && incomplete.is_none() {
        update_page_status(
            pool,
            file_id,
            page_num,
            state,
            "completed",
            Some("No questions found"),
        )
        .await?;
        return Ok(PageResult {
            questions_processed: 0,
            pending_incomplete: None,
        });
    }

    // Process initial questions
    let questions_refs: Vec<&str> = questions.iter().map(std::convert::AsRef::as_ref).collect();
    let (processed, failed) = process_questions_batch(
        pool,
        client,
        file_id,
        page_num,
        questions_refs,
        state,
        max_parallel,
    )
    .await;
    let mut total_questions_processed = processed;
    let mut page_failed = failed;

    // Handle last page incomplete (leftover text without closing anchor)
    if !page_failed && is_last_page {
        if let Some(inc) = incomplete {
            let (final_questions, leftover) = pdf_mod::parser::split_questions_by_anchors(&inc);
            let final_refs: Vec<&str> = final_questions
                .iter()
                .map(std::convert::AsRef::as_ref)
                .collect();
            let (processed, failed) = process_questions_batch(
                pool,
                client,
                file_id,
                page_num,
                final_refs,
                state,
                max_parallel,
            )
            .await;
            total_questions_processed += processed;
            page_failed = failed;

            if let Some(remnant) = leftover
                && !remnant.trim().is_empty()
            {
                match process_single_question(pool, client, file_id, page_num, &remnant, state)
                    .await
                {
                    Ok(()) => total_questions_processed += 1,
                    Err(_) => page_failed = true,
                }
            }
        }
    } else {
        // Store incomplete for next page
        new_pending_incomplete = incomplete.map(|s: Cow<'_, str>| s.into_owned());
    }

    // Mark page complete or failed
    let status = if page_failed { "failed" } else { "completed" };
    let error_msg = if page_failed {
        Some("Page processing failed")
    } else {
        None
    };
    update_page_status(pool, file_id, page_num, state, status, error_msg).await?;

    Ok(PageResult {
        questions_processed: total_questions_processed,
        pending_incomplete: new_pending_incomplete,
    })
}
