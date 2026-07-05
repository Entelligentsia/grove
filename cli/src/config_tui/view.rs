//! Ratatui rendering for the config TUI (Elm-style View layer).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use grove_core::config::Mode;

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
    let area = frame.area();

    // Outer chrome — title includes the integration mode badge.
    let title = format!(
        " grove config   mode: {} ",
        match app.grove_mode {
            Mode::Mcp      => "mcp",
            Mode::Skill    => "skill",
            Mode::Both     => "both",
            Mode::McpLlm   => "mcp-llm ✓",
            Mode::Grammars => "grammars",
        }
    );
    let outer = Block::default().borders(Borders::ALL).title(title);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Vertical sections (leading row for the explore notice)
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0] explore notice (blank when active)
            Constraint::Length(3), // [1] Provider
            Constraint::Length(3), // [2] Endpoint URL
            Constraint::Length(3), // [3] Model
            Constraint::Length(5), // [4] Mode (3 items + borders)
            Constraint::Length(3), // [5] Tap
            Constraint::Min(4),    // [6] Allowed Tools
            Constraint::Length(1), // [7] Status bar
            Constraint::Length(1), // [8] Footer shortcuts
        ])
        .split(inner);

    render_explore_notice(app, frame, rows[0]);
    render_provider(app, frame, rows[1]);
    render_url(app, frame, rows[2]);
    render_model(app, frame, rows[3]);
    render_mode(app, frame, rows[4]);
    render_tap(app, frame, rows[5]);
    render_tools(app, frame, rows[6]);
    render_status(app, frame, rows[7]);
    render_footer(app, frame, rows[8]);

    // The model dropdown floats over the lower rows when open.
    if app.focus == Field::Model && app.model_dropdown {
        render_model_dropdown(app, frame, rows[3]);
    }
}

// ── Explore notice ────────────────────────────────────────────────────────────

fn render_explore_notice(app: &App, frame: &mut Frame, area: Rect) {
    let para = if app.explore_active {
        // Invisible placeholder — blank line keeps layout stable.
        Paragraph::new("")
    } else {
        Paragraph::new(
            "  ⚠  Explore settings inactive — run: grove init --as mcp-llm to activate",
        )
        .style(Style::default().fg(Color::Yellow))
    };
    frame.render_widget(para, area);
}

// ── Tap ─────────────────────────────────────────────────────────────────────

fn render_tap(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.explore_active && app.focus == Field::Tap;
    let text = if app.tap {
        "☑ on — recording sessions to .grove/traces/  (browse: grove tap)"
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
                .title(" Tap — trace LLM sessions (Space: toggle) "),
        )
        .style(Style::default().fg(fg));
    frame.render_widget(para, area);
}

// ── Provider ─────────────────────────────────────────────────────────────────

fn render_provider(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.explore_active && app.focus == Field::Provider;
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
    let focused = app.explore_active && app.focus == Field::Url;
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
    let focused = app.explore_active && app.focus == Field::Model;
    let cursor_suffix = if focused { "█" } else { "" };
    let title = if focused {
        " Model (↓ to pick from provider) "
    } else {
        " Model "
    };
    let para = Paragraph::new(format!("{}{}", app.model, cursor_suffix))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(focused))
                .title(title),
        )
        .style(Style::default().fg(if focused { FOCUSED } else { NORMAL }));
    frame.render_widget(para, area);
}

/// The floating auto-discovery dropdown under the Model field.
fn render_model_dropdown(app: &App, frame: &mut Frame, model_area: Rect) {
    let filtered = app.model_filtered();
    let rows: Vec<ListItem> = if filtered.is_empty() {
        let msg = app
            .model_status
            .clone()
            .unwrap_or_else(|| "no matches — Esc to close".to_string());
        vec![ListItem::new(Line::from(Span::styled(msg, Style::default().fg(DIM))))]
    } else {
        filtered
            .iter()
            .map(|m| ListItem::new(Line::from(Span::styled(m.clone(), Style::default().fg(NORMAL)))))
            .collect()
    };

    // Height: entries (capped) + borders; width matches the model field.
    let shown = filtered.len().clamp(1, 8) as u16;
    let height = (shown + 2).clamp(3, model_area.height.saturating_add(10));
    let full = frame.area();
    let y = (model_area.y + model_area.height).min(full.height.saturating_sub(height));
    let popup = Rect {
        x: model_area.x + 1,
        y,
        width: model_area.width.saturating_sub(2),
        height,
    };

    let title = match &app.model_status {
        Some(s) => format!(" Models · {s} "),
        None => " Models (↑↓ select · type to filter · Enter · Esc) ".to_string(),
    };
    let list = List::new(rows)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(FOCUSED))
                .title(title),
        )
        .highlight_style(Style::default().fg(FOCUSED).add_modifier(Modifier::BOLD))
        .highlight_symbol("▸ ");

    let mut state = ListState::default();
    if !filtered.is_empty() {
        state.select(Some(app.model_cursor.min(filtered.len() - 1)));
    }
    frame.render_widget(Clear, popup);
    frame.render_stateful_widget(list, popup, &mut state);
}

// ── Mode ──────────────────────────────────────────────────────────────────────

fn render_mode(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.explore_active && app.focus == Field::Mode;
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
    let focused = app.explore_active && app.focus == Field::Tools;

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

// ── Footer shortcut bar ─────────────────────────────────────────────────────

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    // Context-sensitive to the focused field; global keys always shown.
    let field_keys = if !app.explore_active {
        "explore settings inactive — Esc to cancel"
    } else {
        match app.focus {
            Field::Provider => "↑↓ select provider",
            Field::Url => "type to edit URL",
            Field::Model if app.model_dropdown => "↑↓ pick · type filter · Enter select · Esc close",
            Field::Model => "type model · ↓ browse provider models",
            Field::Mode => "↑↓ select mode",
            Field::Tap => "Space toggle tracing",
            Field::Tools => "↑↓ move · Space toggle · type+Enter add",
        }
    };
    let text = format!(" {field_keys}  │  Tab next · F2 save · Esc cancel ");
    let bar = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default()
            .fg(Color::Black)
            .bg(FOCUSED)
            .add_modifier(Modifier::BOLD),
    )))
    .style(Style::default().bg(FOCUSED));
    frame.render_widget(bar, area);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(FOCUSED).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    }
}
