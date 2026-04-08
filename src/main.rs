mod api;
mod cli;
mod db;
mod error;
mod pdf;
mod processor;
mod tui;

use std::path::Path;

use anyhow::Result;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use cli::{Cli, apply_cli_settings};
use db::init_database;
use processor::run_tui;
use tui::create_shared_state;

fn setup_logging(app_dir: &Path) -> Result<()> {
    let log_dir = app_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "qbank.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .init();

    std::mem::forget(guard);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = <Cli as clap::Parser>::parse();

    let app_dir = db::get_db_path()?;
    setup_logging(&app_dir)?;

    let pool = init_database(&app_dir).await?;

    let _settings = db::settings::get_settings(&pool).await?;

    if let Some(pdf_path) = cli.pdf_path.clone() {
        let settings = apply_cli_settings(&pool, &cli).await?;
        let state = create_shared_state(settings);
        run_tui(
            pool,
            pdf_path,
            cli.start_page,
            cli.end_page,
            cli.command,
            state,
        )
        .await?;
    } else {
        println!("Please provide a PDF file path");
    }

    Ok(())
}
