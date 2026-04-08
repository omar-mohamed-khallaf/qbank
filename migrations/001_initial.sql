-- Migration: Create initial tables

CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    filename TEXT NOT NULL UNIQUE,
    total_pages INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    page_number INTEGER NOT NULL,
    status TEXT DEFAULT 'pending' NOT NULL CHECK(status IN ('pending', 'processing', 'paused', 'completed', 'failed')),
    error_message TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
    UNIQUE(file_id, page_number)
);

CREATE TABLE IF NOT EXISTS questions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    text TEXT NOT NULL,
    choices TEXT NOT NULL,
    explanation TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Insert default settings
INSERT OR IGNORE INTO settings (key, value) VALUES ('max_retries', '3');
INSERT OR IGNORE INTO settings (key, value) VALUES ('retry_delay_ms', '1000');
INSERT OR IGNORE INTO settings (key, value) VALUES ('retry_multiplier', '2.0');
INSERT OR IGNORE INTO settings (key, value) VALUES ('api_model', 'deepseek-r1:8b');
INSERT OR IGNORE INTO settings (key, value) VALUES ('think', 'false');

CREATE INDEX idx_pages_file_id ON pages(file_id);
CREATE INDEX idx_questions_file_id ON questions(file_id);
