use anyhow::Result;
use tracing::error;

use crate::api::LlmClient;
use crate::db::DbPool;
use crate::db::pages as db_pages;
use crate::db::questions as db_questions;
use crate::error::AppError;
use crate::tui::SharedState;

pub async fn process_single_question(
    pool: &DbPool,
    client: &LlmClient,
    file_id: i64,
    page_num: i32,
    question_text: &str,
    state: &SharedState,
) -> Result<(), AppError> {
    match client.process_medical_question(question_text).await {
        Ok(responses) => {
            for qr in responses {
                let choices_json = serde_json::to_string(&qr.c).unwrap_or_default();
                let explanation_json = serde_json::to_string(&qr.e).unwrap_or_default();
                let question_input = db_questions::QuestionInput {
                    text: qr.q,
                    choices: choices_json,
                    explanation: explanation_json,
                };
                db_questions::insert_questions(pool, file_id, vec![question_input]).await?;
            }
            Ok(())
        }
        Err(e) => {
            error!("API error processing page {}: {}", page_num, e);
            db_pages::update_page_status(pool, file_id, page_num, "failed", Some(&e.to_string()))
                .await?;
            let mut s = state.write().await;
            s.update_page_status(page_num, "failed".to_string(), Some(e.to_string()));
            s.add_error(format!("API error on page {page_num}: {e}"));
            Err(e)
        }
    }
}
