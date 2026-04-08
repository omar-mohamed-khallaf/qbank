use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::db::DbPool;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub pages_per_batch: i32,
    pub max_retries: i32,
    pub retry_delay_ms: u64,
    pub retry_multiplier: f64,
    pub api_model: String,
    pub batch_questions_max: i32,
    pub think: bool,
    pub max_parallel_questions: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pages_per_batch: 2,
            max_retries: 3,
            retry_delay_ms: 1000,
            retry_multiplier: 2.0,
            api_model: "deepseek-r1".to_string(),
            batch_questions_max: 20,
            think: false,
            max_parallel_questions: 0,
        }
    }
}

pub async fn get_settings(pool: &DbPool) -> Result<Settings, AppError> {
    let rows = sqlx::query("SELECT key, value FROM settings")
        .fetch_all(pool)
        .await?;

    let mut settings = Settings::default();

    for row in rows {
        let key: String = row.get("key");
        let value: String = row.get("value");
        match key.as_str() {
            "pages_per_batch" => settings.pages_per_batch = value.parse().unwrap_or(1),
            "max_retries" => settings.max_retries = value.parse().unwrap_or(3),
            "retry_delay_ms" => settings.retry_delay_ms = value.parse().unwrap_or(1000),
            "retry_multiplier" => settings.retry_multiplier = value.parse().unwrap_or(2.0),
            "api_model" => settings.api_model = value,
            "batch_questions_max" => settings.batch_questions_max = value.parse().unwrap_or(20),
            "think" => settings.think = value == "true",
            "max_parallel_questions" => {
                settings.max_parallel_questions = value.parse().unwrap_or(0);
            }
            _ => {}
        }
    }

    Ok(settings)
}

pub async fn update_setting(pool: &DbPool, key: &str, value: &str) -> Result<(), AppError> {
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(())
}
