use serde::{Deserialize, Serialize};

use crate::db::{AppError, DbPool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub id: i64,
    pub filename: String,
    pub total_pages: i32,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn create_file(
    pool: &DbPool,
    filename: &str,
    total_pages: i32,
) -> Result<File, AppError> {
    let result = sqlx::query_as::<_, FileRow>(
        "INSERT INTO files (filename, total_pages) VALUES (?, ?) RETURNING *",
    )
    .bind(filename)
    .bind(total_pages)
    .fetch_one(pool)
    .await?;
    Ok(result.into())
}

pub async fn get_file_by_name(pool: &DbPool, filename: &str) -> Result<Option<File>, AppError> {
    let result = sqlx::query_as::<_, FileRow>("SELECT * FROM files WHERE filename = ?")
        .bind(filename)
        .fetch_optional(pool)
        .await?;
    Ok(result.map(std::convert::Into::into))
}

pub async fn get_file_by_id(pool: &DbPool, id: i64) -> Result<Option<File>, AppError> {
    let result = sqlx::query_as::<_, FileRow>("SELECT * FROM files WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(result.map(std::convert::Into::into))
}

pub async fn get_file_status(pool: &DbPool, file_id: i64) -> Result<String, AppError> {
    let result = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM pages WHERE file_id = ? AND status = 'completed'",
    )
    .bind(file_id)
    .fetch_one(pool)
    .await?;

    let completed = result.0;
    let file = get_file_by_id(pool, file_id)
        .await?
        .ok_or_else(|| AppError::Config("File not found".to_string()))?;

    if completed >= i64::from(file.total_pages) && file.total_pages > 0 {
        Ok("completed".to_string())
    } else {
        Ok("not_completed".to_string())
    }
}

#[derive(sqlx::FromRow)]
struct FileRow {
    id: i64,
    filename: String,
    total_pages: i32,
    created_at: String,
    updated_at: String,
}

impl From<FileRow> for File {
    fn from(row: FileRow) -> Self {
        Self {
            id: row.id,
            filename: row.filename,
            total_pages: row.total_pages,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
