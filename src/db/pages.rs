use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::db::DbPool;
use crate::error::AppError;

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct Page {
//     pub id: i64,
//     pub file_id: i64,
//     pub status: String,
//     pub error_message: Option<String>,
//     pub created_at: String,
//     pub updated_at: String,
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageStatus {
    pub page_number: i32,
    pub status: String,
    pub error_message: Option<String>,
}

pub async fn create_pages_for_file(
    pool: &DbPool,
    file_id: i64,
    total_pages: i32,
) -> Result<(), AppError> {
    for page_num in 1..=total_pages {
        sqlx::query(
            "INSERT OR IGNORE INTO pages (file_id, page_number, status) VALUES (?, ?, 'pending')",
        )
        .bind(file_id)
        .bind(page_num)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn get_page_statuses(pool: &DbPool, file_id: i64) -> Result<Vec<PageStatus>, AppError> {
    let rows = sqlx::query_as::<_, PageRow>(
        "SELECT page_number, status, error_message FROM pages WHERE file_id = ? ORDER BY page_number"
    )
    .bind(file_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(std::convert::Into::into).collect())
}

pub async fn update_page_status(
    pool: &DbPool,
    file_id: i64,
    page_number: i32,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE pages SET status = ?, error_message = ?, updated_at = datetime('now') WHERE file_id = ? AND page_number = ?"
    )
    .bind(status)
    .bind(error_message)
    .bind(file_id)
    .bind(page_number)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_page_status(pool: &DbPool, file_id: i64, page_number: i32) -> Result<Option<PageStatus>, AppError> {
    let row = sqlx::query_as::<_, PageRow>(
        "SELECT page_number, status, error_message FROM pages WHERE file_id = ? AND page_number = ?"
    )
    .bind(file_id)
    .bind(page_number)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(std::convert::Into::into))
}

pub async fn get_failed_pages(pool: &DbPool, file_id: i64) -> Result<Vec<i32>, AppError> {
    let rows = sqlx::query(
        "SELECT page_number FROM pages WHERE file_id = ? AND status = 'failed' ORDER BY page_number"
    )
    .bind(file_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.get("page_number")).collect())
}

pub async fn reset_pending_pages(pool: &DbPool, file_id: i64) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE pages SET status = 'pending', error_message = NULL WHERE file_id = ? AND status = 'failed'"
    )
    .bind(file_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reset_processing_pages(pool: &DbPool, file_id: i64) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE pages SET status = 'pending', error_message = NULL WHERE file_id = ? AND status = 'processing'"
    )
    .bind(file_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct PageRow {
    page_number: i32,
    status: String,
    error_message: Option<String>,
}

impl From<PageRow> for PageStatus {
    fn from(row: PageRow) -> Self {
        Self {
            page_number: row.page_number,
            status: row.status,
            error_message: row.error_message,
        }
    }
}
