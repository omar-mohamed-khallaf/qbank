use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table},
};

use crate::tui::state::AppState;

pub fn render_app(frame: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Min(8),
        ])
        .split(frame.area());

    render_header(frame, state, chunks[0]);
    render_progress(frame, state, chunks[1]);
    render_pages_table(frame, state, chunks[2]);
    render_logs(frame, state, chunks[3]);
}

fn render_header(frame: &mut Frame, state: &AppState, area: Rect) {
    let title = match &state.current_file {
        Some(f) => format!("Processing: {}", f.filename),
        None => "No file loaded".to_string(),
    };

    let status_text = match state.processing_status {
        crate::tui::state::ProcessingStatus::Pending => "Pending",
        crate::tui::state::ProcessingStatus::Processing => "Processing...",
        crate::tui::state::ProcessingStatus::Paused => "Paused",
        crate::tui::state::ProcessingStatus::Completed => "Completed",
        crate::tui::state::ProcessingStatus::Error => "Error",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let paragraph = Paragraph::new(status_text)
        .block(block)
        .style(Style::default().fg(Color::Cyan));

    frame.render_widget(paragraph, area);
}

fn render_progress(frame: &mut Frame, state: &AppState, area: Rect) {
    let (completed, total) = state.get_progress();
    let percentage = if total > 0 {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        {
            (f64::from(completed) / f64::from(total) * 100.0) as u16
        }
    } else {
        0
    };

    let label = if let Some((start, end)) = state.current_batch {
        format!("Processing pages {start}-{end} ({completed}/{total})")
    } else if total > 0 {
        format!("Progress ({completed}/{total})")
    } else {
        "No pages".to_string()
    };

    let gauge = Gauge::default()
        .label(label)
        .percent(percentage)
        .gauge_style(Style::default().fg(Color::Green))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );

    frame.render_widget(gauge, area);
}

fn render_pages_table(frame: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .title("Pages")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    if state.page_statuses.is_empty() {
        let paragraph = Paragraph::new("No pages loaded")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(paragraph, area);
        return;
    }

    let header_lines = 1;
    let available_rows = area.height.saturating_sub(2).saturating_sub(header_lines) as usize;
    let total_pages = state.page_statuses.len();

    let start_idx = state.pages_scroll.min(total_pages.saturating_sub(1));
    let end_idx = (start_idx + available_rows).min(total_pages);
    let visible_pages = &state.page_statuses[start_idx..end_idx];

    let rows: Vec<Row> = visible_pages
        .iter()
        .map(|p| {
            let status_symbol = match p.status.as_str() {
                "completed" => "✓",
                "processing" => "⟳",
                "failed" => "✗",
                _ => "○",
            };

            let status_color = match p.status.as_str() {
                "completed" => Color::Green,
                "processing" => Color::Yellow,
                "failed" => Color::Red,
                _ => Color::Gray,
            };

            let error_msg = p.error_message.as_deref().unwrap_or("-");

            Row::new(vec![
                Cell::from(format!("{}", p.page_number)),
                Cell::from(format!("{} {}", status_symbol, p.status))
                    .style(Style::default().fg(status_color)),
                Cell::from(error_msg),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(6),
            Constraint::Length(14),
            Constraint::Min(30),
        ],
    )
    .header(
        Row::new(vec!["#", "Status", "Error"]).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(block)
    .widths([
        Constraint::Length(6),
        Constraint::Length(14),
        Constraint::Min(30),
    ]);

    frame.render_widget(table, area);
}

fn render_logs(frame: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .title("Logs / Errors / Warnings")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    if state.logs.is_empty() {
        let paragraph = Paragraph::new("No logs yet")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(paragraph, area);
        return;
    }

    let available_rows = area.height.saturating_sub(2) as usize;
    let total_logs = state.logs.len();

    let start_idx = state.logs_scroll.min(total_logs.saturating_sub(1));
    let end_idx = (start_idx + available_rows).min(total_logs);

    let visible_logs: Vec<_> = state.logs[start_idx..end_idx].iter().collect();

    let items: Vec<ListItem> = visible_logs
        .iter()
        .map(|log| {
            let level_color = match log.level.as_str() {
                "ERROR" => Color::Red,
                "WARN" => Color::Yellow,
                _ => Color::Green,
            };

            let line = Line::from(vec![
                Span::raw(format!("[{}] ", log.timestamp)),
                Span::raw(&log.level).style(Style::default().fg(level_color)),
                Span::raw(format!(" {}", log.message)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(block);

    frame.render_widget(list, area);
}
