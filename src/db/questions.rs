use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: i64,
    pub file_id: i64,
    pub text: String,
    pub choices: String,
    pub explanation: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionInput {
    pub text: String,
    pub choices: String,
    pub explanation: String,
}

pub async fn insert_questions(
    pool: &DbPool,
    file_id: i64,
    questions: Vec<QuestionInput>,
) -> Result<i32, AppError> {
    let mut count = 0;
    for q in questions {
        sqlx::query(
            "INSERT INTO questions (file_id, text, choices, explanation) VALUES (?, ?, ?, ?)",
        )
        .bind(file_id)
        .bind(&q.text)
        .bind(&q.choices)
        .bind(&q.explanation)
        .execute(pool)
        .await?;
        count += 1;
    }
    Ok(count)
}

pub async fn get_questions_by_file(pool: &DbPool, file_id: i64) -> Result<Vec<Question>, AppError> {
    let rows =
        sqlx::query_as::<_, QuestionRow>("SELECT * FROM questions WHERE file_id = ? ORDER BY id")
            .bind(file_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(std::convert::Into::into).collect())
}

pub async fn get_questions_count(pool: &DbPool, file_id: i64) -> Result<i32, AppError> {
    let count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM questions WHERE file_id = ?")
        .bind(file_id)
        .fetch_one(pool)
        .await?;
    Ok(count)
}

#[derive(sqlx::FromRow)]
struct QuestionRow {
    id: i64,
    file_id: i64,
    text: String,
    choices: String,
    explanation: Option<String>,
    created_at: String,
}

impl From<QuestionRow> for Question {
    fn from(row: QuestionRow) -> Self {
        Self {
            id: row.id,
            file_id: row.file_id,
            text: row.text,
            choices: row.choices,
            explanation: row.explanation,
            created_at: row.created_at,
        }
    }
}
