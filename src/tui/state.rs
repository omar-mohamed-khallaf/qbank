use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::db::settings::Settings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

impl LogEntry {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: "INFO".to_string(),
            message: message.into(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        }
    }

    pub fn warn(message: impl Into<String>) -> Self {
        Self {
            level: "WARN".to_string(),
            message: message.into(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: "ERROR".to_string(),
            message: message.into(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingStatus {
    Pending,
    Processing,
    Paused,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: i64,
    pub filename: String,
    pub total_pages: i32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageStatusInfo {
    pub page_number: i32,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub current_file: Option<FileInfo>,
    pub page_statuses: Vec<PageStatusInfo>,
    pub total_questions: i32,
    pub logs: Vec<LogEntry>,
    pub processing_status: ProcessingStatus,
    pub current_batch: Option<(i32, i32)>,
    pub settings: Settings,
    pub pages_scroll: usize,
    pub logs_scroll: usize,
    pub shutdown: bool,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        Self {
            current_file: None,
            page_statuses: Vec::new(),
            total_questions: 0,
            logs: Vec::new(),
            processing_status: ProcessingStatus::Pending,
            current_batch: None,
            settings,
            pages_scroll: 0,
            logs_scroll: 0,
            shutdown: false,
        }
    }

    pub fn add_info(&mut self, message: impl Into<String>) {
        let was_at_bottom = self.logs_scroll >= self.logs.len().saturating_sub(1);
        self.logs.push(LogEntry::info(message));
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
        if was_at_bottom {
            self.logs_scroll = self.logs.len().saturating_sub(1);
        }
    }

    pub fn add_warn(&mut self, message: impl Into<String>) {
        let was_at_bottom = self.logs_scroll >= self.logs.len().saturating_sub(1);
        self.logs.push(LogEntry::warn(message));
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
        if was_at_bottom {
            self.logs_scroll = self.logs.len().saturating_sub(1);
        }
    }

    pub fn add_error(&mut self, message: impl Into<String>) {
        let was_at_bottom = self.logs_scroll >= self.logs.len().saturating_sub(1);
        self.logs.push(LogEntry::error(message));
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
        if was_at_bottom {
            self.logs_scroll = self.logs.len().saturating_sub(1);
        }
    }

    pub fn set_file(&mut self, file: Option<FileInfo>) {
        self.current_file = file;
        self.page_statuses.clear();
        self.total_questions = 0;
    }

    pub fn set_page_statuses(&mut self, statuses: Vec<PageStatusInfo>) {
        self.page_statuses = statuses;
    }

    pub fn update_page_status(&mut self, page_number: i32, status: String, error: Option<String>) {
        if let Some(ps) = self
            .page_statuses
            .iter_mut()
            .find(|p| p.page_number == page_number)
        {
            ps.status = status;
            ps.error_message = error;
        }
    }

    pub fn get_progress(&self) -> (u32, u32) {
        let completed = self
            .page_statuses
            .iter()
            .filter(|p| p.status == "completed")
            .count();
        let total = self.page_statuses.len();
        (completed as u32, total as u32)
    }

    pub fn get_failed_pages(&self) -> Vec<i32> {
        self.page_statuses
            .iter()
            .filter(|p| p.status == "failed")
            .map(|p| p.page_number)
            .collect()
    }

    pub fn add_log(&mut self, entry: LogEntry) {
        let was_at_bottom = self.logs_scroll >= self.logs.len().saturating_sub(1);
        self.logs.push(entry);
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
        if was_at_bottom {
            self.logs_scroll = self.logs.len().saturating_sub(1);
        }
    }

    pub fn set_pages_scroll(&mut self, offset: usize) {
        self.pages_scroll = offset.min(self.page_statuses.len().saturating_sub(1));
    }

    pub fn set_logs_scroll(&mut self, offset: usize) {
        self.logs_scroll = offset.min(self.logs.len().saturating_sub(1));
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

pub fn create_shared_state(settings: Settings) -> SharedState {
    Arc::new(RwLock::new(AppState::new(settings)))
}
