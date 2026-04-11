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
    #[arg(short, default_value_t = 0)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

fn parse_device_indices(s: &str) -> Result<Vec<usize>, String> {
    s.split(',')
        .map(|id| id.trim().parse::<usize>().map_err(|e| e.to_string()))
        .collect()
}

#[derive(Subcommand)]
pub enum Command {
    Process {
        #[arg(value_name = "PDF_FILE")]
        pdf_path: PathBuf,

        #[arg(long, default_value_t = 0)]
        start_page: i32,

        #[arg(long, default_value_t = 0)]
        end_page: i32,

        #[arg(long, default_value_t = 0)]
        batch_size: i32,

        #[arg(long, default_value_t = 0)]
        max_retries: i32,

        #[arg(long, default_value_t = 0)]
        retry_delay_ms: u64,

        #[arg(long, default_value_t = 2.0)]
        retry_multiplier: f64,

        #[arg(long)]
        model_path: Option<PathBuf>,

        #[arg(long, default_value_t = 0)]
        context_size: u32,

        #[arg(long, value_delimiter = ',', num_args = 0..)]
        devices: Option<Vec<usize>>,

        #[arg(long, default_value_t = true)]
        think: bool,

        #[arg(long)]
        parallel: Option<i32>,
    },

    RetryFailed,

    ListDevices,
}

pub async fn apply_cli_settings(
    pool: &DbPool,
    process: &ProcessArgs,
) -> Result<Settings, AppError> {
    if process.batch_size > 0 {
        db_settings::update_setting(pool, "pages_per_batch", &process.batch_size.to_string())
            .await?;
    }
    if process.max_retries > 0 {
        db_settings::update_setting(pool, "max_retries", &process.max_retries.to_string()).await?;
    }
    if process.retry_delay_ms > 0 {
        db_settings::update_setting(pool, "retry_delay_ms", &process.retry_delay_ms.to_string())
            .await?;
    }
    if (process.retry_multiplier - 2.0).abs() > 0.1 {
        db_settings::update_setting(
            pool,
            "retry_multiplier",
            &process.retry_multiplier.to_string(),
        )
        .await?;
    }
    if let Some(path) = &process.model_path {
        db_settings::update_setting(pool, "model_path", &path.to_string_lossy()).await?;
    }
    if process.context_size != 0 {
        db_settings::update_setting(pool, "context_size", &process.context_size.to_string())
            .await?;
    }
    if let Some(devices) = &process.devices {
        let devices_str = devices
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        db_settings::update_setting(pool, "devices", &devices_str).await?;
    }
    if process.think {
        db_settings::update_setting(pool, "think", "true").await?;
    }
    if let Some(n) = process.parallel {
        if n > 0 {
            db_settings::update_setting(pool, "max_parallel_questions", &n.to_string()).await?;
        } else {
            db_settings::update_setting(pool, "max_parallel_questions", "0").await?;
        }
    }
    db_settings::get_settings(pool).await
}

pub struct ProcessArgs {
    pub batch_size: i32,
    pub max_retries: i32,
    pub retry_delay_ms: u64,
    pub retry_multiplier: f64,
    pub model_path: Option<PathBuf>,
    pub context_size: u32,
    pub devices: Option<Vec<usize>>,
    pub think: bool,
    pub parallel: Option<i32>,
}

impl Default for ProcessArgs {
    fn default() -> Self {
        ProcessArgs {
            batch_size: 0,
            max_retries: 0,
            retry_delay_ms: 0,
            retry_multiplier: 2.0,
            model_path: None,
            context_size: 0,
            devices: None,
            think: true,
            parallel: None,
        }
    }
}

impl From<&Command> for ProcessArgs {
    fn from(cmd: &Command) -> Self {
        match cmd {
            Command::Process {
                batch_size,
                max_retries,
                retry_delay_ms,
                retry_multiplier,
                model_path,
                context_size,
                devices,
                think,
                parallel,
                ..
            } => ProcessArgs {
                batch_size: *batch_size,
                max_retries: *max_retries,
                retry_delay_ms: *retry_delay_ms,
                retry_multiplier: *retry_multiplier,
                model_path: model_path.clone(),
                context_size: *context_size,
                devices: devices.clone(),
                think: *think,
                parallel: *parallel,
            },
            Command::RetryFailed | Command::ListDevices => ProcessArgs {
                batch_size: 0,
                max_retries: 0,
                retry_delay_ms: 0,
                retry_multiplier: 2.0,
                model_path: None,
                context_size: 0,
                devices: None,
                think: true,
                parallel: None,
            },
        }
    }
}

