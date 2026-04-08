use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::db::DbPool;
use crate::db::settings as db_settings;
use crate::db::settings::Settings;
use crate::error::AppError;

#[derive(Parser)]
#[command(name = "qbank")]
#[command(about = "PDF Question Bank Processor", long_about = None)]
pub struct Cli {
    #[arg(required = true, value_name = "PDF_FILE")]
    pub pdf_path: Option<PathBuf>,

    #[arg(long, default_value_t = 0)]
    pub start_page: i32,

    #[arg(long, default_value_t = 0)]
    pub end_page: i32,

    #[arg(long, default_value_t = 0)]
    pub batch_size: i32,

    #[arg(long, default_value_t = 0)]
    pub max_retries: i32,

    #[arg(long, default_value_t = 0)]
    pub retry_delay_ms: u64,

    #[arg(long, default_value_t = 2.0)]
    pub retry_multiplier: f64,

    #[arg(long)]
    pub api_model: Option<String>,

    #[arg(long, default_value_t = true)]
    pub think: bool,

    #[arg(long)]
    pub parallel: Option<i32>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    RetryFailed,
}

pub async fn apply_cli_settings(pool: &DbPool, cli: &Cli) -> Result<Settings, AppError> {
    if cli.batch_size > 0 {
        db_settings::update_setting(pool, "pages_per_batch", &cli.batch_size.to_string()).await?;
    }
    if cli.max_retries > 0 {
        db_settings::update_setting(pool, "max_retries", &cli.max_retries.to_string()).await?;
    }
    if cli.retry_delay_ms > 0 {
        db_settings::update_setting(pool, "retry_delay_ms", &cli.retry_delay_ms.to_string())
            .await?;
    }
    if (cli.retry_multiplier - 2.0).abs() > 0.1 {
        db_settings::update_setting(pool, "retry_multiplier", &cli.retry_multiplier.to_string())
            .await?;
    }
    if let Some(model) = &cli.api_model {
        db_settings::update_setting(pool, "api_model", model).await?;
    }
    if cli.think {
        db_settings::update_setting(pool, "think", "true").await?;
    }
    if let Some(n) = cli.parallel {
        if n > 0 {
            db_settings::update_setting(pool, "max_parallel_questions", &n.to_string()).await?;
        } else {
            db_settings::update_setting(pool, "max_parallel_questions", "0").await?;
        }
    }
    db_settings::get_settings(pool).await
}
