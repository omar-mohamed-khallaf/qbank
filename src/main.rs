mod api;
mod cli;
mod db;
mod error;
mod pdf;
mod processor;
mod tui;

use std::path::Path;

use anyhow::Result;
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use cli::{Cli, Command, ProcessArgs, apply_cli_settings};
use db::init_database;
use db::settings as db_settings;
use processor::run_tui;

fn setup_logging(verbose: u8, app_dir: &Path) -> Result<()> {
    let log_dir = app_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "qbank.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let log_level = match verbose {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive(log_level.into());

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
    setup_logging(cli.verbose, &app_dir)?;

    let pool = init_database(&app_dir).await?;

    match &cli.command {
        Command::ListDevices => {
            let devices = api::client::list_devices();
            for device in devices {
                println!(
                    "[{}] {} - {} ({} backend, {} total memory)",
                    device.index,
                    device.name,
                    device.description,
                    device.backend,
                    device.memory_total
                );
            }
            return Ok(());
        }
        Command::RetryFailed => {
            let settings = db_settings::get_settings(&pool).await?;
            let process_args = ProcessArgs::from(&cli.command);
            run_tui(pool, None, 0, 0, Some(process_args), settings).await?;
            return Ok(());
        }
        Command::Process {
            pdf_path,
            start_page,
            end_page,
            ..
        } => {
            let process_args = ProcessArgs::from(&cli.command);
            let settings = apply_cli_settings(&pool, &process_args).await?;

            run_tui(
                pool,
                Some(pdf_path.clone()),
                *start_page,
                *end_page,
                Some(process_args),
                settings,
            )
            .await?;

            Ok(())
        }
    }
}

