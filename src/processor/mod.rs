pub mod page;
pub mod pdf;
pub mod question;
pub mod setup;

pub use setup::run_tui;

use crate::tui::SharedState;

pub const TUI_POLL_INTERVAL_MS: u64 = 100;

// Helper for updating both DB and state atomically
// Note: DbPool is thread-safe (Arc-wrapped), no locking needed
// Only SharedState (RwLock<AppState>) needs async locking
pub async fn update_page_status(
    pool: &crate::db::DbPool,
    file_id: i64,
    page_num: i32,
    state: &SharedState,
    status: &str,
    error: Option<&str>,
) -> Result<(), crate::error::AppError> {
    crate::db::pages::update_page_status(pool, file_id, page_num, status, error).await?;
    {
        let mut s = state.write().await;
        s.update_page_status(
            page_num,
            status.to_string(),
            error.map(std::string::ToString::to_string),
        );
    }
    Ok(())
}
