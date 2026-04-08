pub mod files;
pub mod pages;
pub mod questions;
pub mod settings;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use std::path::{Path, PathBuf};
use tracing::info;

use crate::error::AppError;

pub type DbPool = Pool<Sqlite>;

pub async fn init_database(app_dir: &Path) -> Result<DbPool, AppError> {
    std::fs::create_dir_all(app_dir)?;
    let db_path = app_dir.join("qbank.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    info!("Initializing database at: {}", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    info!("Database initialized successfully");
    Ok(pool)
}

pub fn get_db_path() -> Result<PathBuf, AppError> {
    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| AppError::Config("Could not find data directory".to_string()))?
        .join("qbank");
    Ok(data_dir)
}
