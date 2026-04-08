use std::io::stdout;

use crossterm::ExecutableCommand;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, poll};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::processor::TUI_POLL_INTERVAL_MS;
use crate::tui::{SharedState, render_app};

pub async fn run_tui_loop(state: SharedState) {
    stdout().execute(EnterAlternateScreen).unwrap();
    crossterm::terminal::enable_raw_mode().ok();
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout())).unwrap();
    let mut current_scroll_target: Option<&str> = None;

    loop {
        let mut state_guard = state.write().await;
        let current_state = state_guard.clone();

        if let Some(ref batch) = current_state.current_batch {
            let (_, end_page) = *batch;
            if current_scroll_target != Some("logs")
                && current_state
                    .page_statuses
                    .iter()
                    .find(|p| p.page_number == end_page && p.status == "processing")
                    .is_some()
            {
                let idx = current_state
                    .page_statuses
                    .iter()
                    .position(|p| p.page_number == end_page)
                    .unwrap_or(0);
                state_guard.pages_scroll = idx.saturating_sub(5);
            }
        }
        drop(state_guard);

        terminal.draw(|f| render_app(f, &current_state)).unwrap();

        if poll(std::time::Duration::from_millis(TUI_POLL_INTERVAL_MS)).unwrap()
            && let Ok(event) = crossterm::event::read()
        {
            let mut state_guard = state.write().await;
            match event {
                Event::Key(
                    KeyEvent {
                        code: KeyCode::Char('q'),
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    },
                ) => break,
                Event::Key(KeyEvent {
                    code: KeyCode::Up, ..
                }) => {
                    current_scroll_target = Some("logs");
                    state_guard.logs_scroll = state_guard.logs_scroll.saturating_sub(1);
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    ..
                }) => {
                    current_scroll_target = Some("logs");
                    state_guard.logs_scroll =
                        (state_guard.logs_scroll + 1).min(state_guard.logs.len().saturating_sub(1));
                }
                Event::Key(KeyEvent {
                    code: KeyCode::PageUp,
                    ..
                }) => {
                    current_scroll_target = Some("pages");
                    state_guard.pages_scroll = state_guard.pages_scroll.saturating_sub(10);
                }
                Event::Key(KeyEvent {
                    code: KeyCode::PageDown,
                    ..
                }) => {
                    current_scroll_target = Some("pages");
                    state_guard.pages_scroll = (state_guard.pages_scroll + 10)
                        .min(state_guard.page_statuses.len().saturating_sub(1));
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('g'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => {
                    current_scroll_target = Some("logs");
                    state_guard.logs_scroll = state_guard.logs.len().saturating_sub(1);
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('G'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => {
                    current_scroll_target = Some("pages");
                    state_guard.pages_scroll = state_guard.page_statuses.len().saturating_sub(1);
                }
                _ => {}
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(TUI_POLL_INTERVAL_MS)).await;
    }

    stdout().execute(LeaveAlternateScreen).unwrap();
    disable_raw_mode().ok();
}
