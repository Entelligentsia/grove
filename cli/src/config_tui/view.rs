//! Ratatui rendering for the config TUI (Elm-style View layer).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::config_tui::model::{App, Field};

/// Colours.
const FOCUSED: Color = Color::Cyan;
const NORMAL: Color = Color::White;
const SELECTED: Color = Color::Green;
const DIM: Color = Color::DarkGray;

/// Mode descriptions shown alongside each mode entry.
const MODE_DESCS: &[&str] = &[
    "Standard   — merit-based, least intrusive",
    "Balanced   — plan-first steering",
    "Aggressive — coercive steering",
];

/// Render the full TUI frame.
pub fn view(app: &App, frame: &mut Frame) {
    if app.show_logs {
        render_logs(app, frame);
        return;
    }

    let area = frame.area();

    // Outer chrome
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" grove config — Explore Setup (Tab: field · Space: toggle · F2: save · F3: trace · Esc: cancel) ");
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Vertical sections
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Provider
            Constraint::Length(3), // Endpoint URL
            Constraint::Length(3), // Model
            Constraint::Length(5), // Mode (3 items + borders)
            Constraint::Length(3), // Tap
            Constraint::Min(4),    // Allowed Tools
            Constraint::Length(1), // Status bar
        ])
        .split(inner);

    render_provider(app, frame, rows[0]);
    render_url(app, frame, rows[1]);
    render_model(app, frame, rows[2]);
    render_mode(app, frame, rows[3]);
    render_tap(app, frame, rows[4]);
    render_tools(app, frame, rows[5]);
    render_status(app, frame, rows[6]);
}

// ── Tap ─────────────────────────────────────────────────────────────────────

fn render_tap(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Field::Tap;
    let text = if app.tap {
        "☑ on — tracing LLM traffic to .grove/explore-trace.log  (F3: view live)"
    } else {
        "☐ off"
    };
    let fg = if focused {
        FOCUSED
    } else if app.tap {
        SELECTED
    } else {
        DIM
    };
    let para = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(focused))
                .title(" Tap — trace LLM traffic (Space: toggle) "),
        )
        .style(Style::default().fg(fg));
    frame.render_widget(para, area);
}

// ── Live trace-log view ───────────────────────────────────────────────────────

fn render_logs(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(FOCUSED))
        .title(" Explore trace — .grove/explore-trace.log  (F3/Esc: back · Ctrl-C: quit) ");
    let inner_h = block.inner(area).height as usize;
    let scroll = app.logs.len().saturating_sub(inner_h) as u16;
    let para = Paragraph::new(app.logs.join("\n"))
        .block(block)
        .scroll((scroll, 0));
    frame.render_widget(para, area);
}

// ── Provider ─────────────────────────────────────────────────────────────────

fn render_provider(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Field::Provider;
    let border_style = border_style(focused);

    let labels = &["Ollama", "Llama.cpp"];
    let items: Vec<ListItem> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let style = if i == app.provider {
                Style::default().fg(SELECTED).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(NORMAL)
            };
            ListItem::new(Line::from(Span::styled(*label, style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Provider (↑↓ to select) "),
        )
        .highlight_style(Style::default().fg(FOCUSED).add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    state.select(Some(app.provider));
    frame.render_stateful_widget(list, area, &mut state);
}

// ── Endpoint URL ──────────────────────────────────────────────────────────────

fn render_url(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Field::Url;
    let cursor_suffix = if focused { "█" } else { "" };
    let para = Paragraph::new(format!("{}{}", app.base_url, cursor_suffix))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(focused))
                .title(" Endpoint URL "),
        )
        .style(Style::default().fg(if focused { FOCUSED } else { NORMAL }));
    frame.render_widget(para, area);
}

// ── Model ─────────────────────────────────────────────────────────────────────

fn render_model(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Field::Model;
    let cursor_suffix = if focused { "█" } else { "" };
    let para = Paragraph::new(format!("{}{}", app.model, cursor_suffix))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(focused))
                .title(" Model "),
        )
        .style(Style::default().fg(if focused { FOCUSED } else { NORMAL }));
    frame.render_widget(para, area);
}

// ── Mode ──────────────────────────────────────────────────────────────────────

fn render_mode(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Field::Mode;
    let items: Vec<ListItem> = MODE_DESCS
        .iter()
        .enumerate()
        .map(|(i, desc)| {
            let style = if i == app.mode {
                Style::default().fg(SELECTED).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(NORMAL)
            };
            ListItem::new(Line::from(Span::styled(*desc, style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(focused))
                .title(" Mode (↑↓ to select) "),
        )
        .highlight_style(Style::default().fg(FOCUSED).add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    state.select(Some(app.mode));
    frame.render_stateful_widget(list, area, &mut state);
}

// ── Allowed Tools ─────────────────────────────────────────────────────────────

fn render_tools(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Field::Tools;

    // Split: tool list on left, add-tool input on right
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(24)])
        .split(area);

    // Tool checklist
    let items: Vec<ListItem> = app
        .tools
        .iter()
        .enumerate()
        .map(|(i, (name, selected))| {
            let check = if *selected { "☑" } else { "☐" };
            let style = if focused && i == app.tool_cursor {
                Style::default().fg(FOCUSED).add_modifier(Modifier::BOLD)
            } else if *selected {
                Style::default().fg(SELECTED)
            } else {
                Style::default().fg(DIM)
            };
            ListItem::new(Line::from(Span::styled(format!("{check} {name}"), style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(focused))
                .title(" Allowed Tools (Space: toggle) "),
        )
        .highlight_style(Style::default().fg(FOCUSED));

    let mut state = ListState::default();
    if focused && !app.tools.is_empty() {
        state.select(Some(app.tool_cursor));
    }
    frame.render_stateful_widget(list, cols[0], &mut state);

    // Add-tool buffer
    let cursor_suffix = if focused { "█" } else { "" };
    let add_para = Paragraph::new(format!("{}{}", app.add_tool_buf, cursor_suffix))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(focused))
                .title(" + Add Tool (Enter) "),
        )
        .style(Style::default().fg(if focused { FOCUSED } else { DIM }));
    frame.render_widget(add_para, cols[1]);
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn render_status(app: &App, frame: &mut Frame, area: Rect) {
    // Show the provider default-URL hint or the last error.
    let text = if let Some(ref err) = app.last_error {
        Span::styled(format!("✗ {err}"), Style::default().fg(Color::Red))
    } else {
        let hint = match app.provider {
            0 => "Provider: Ollama  ·  default http://localhost:11434/v1".to_string(),
            _ => "Provider: Llama.cpp  ·  default http://localhost:8080/v1".to_string(),
        };
        Span::styled(hint, Style::default().fg(DIM))
    };
    frame.render_widget(Paragraph::new(Line::from(text)), area);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(FOCUSED).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    }
}
