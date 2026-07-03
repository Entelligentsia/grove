//! Ratatui rendering for the trace browser (Elm-style View layer).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use grove_core::explore::trace::{format_request, format_response};

use crate::trace_tui::model::{hms_utc, App, Call, View};

const FOCUSED: Color = Color::Cyan;
const NORMAL: Color = Color::White;
const DIM: Color = Color::DarkGray;
const ACCENT: Color = Color::Green;

/// Render the whole frame: a body region above a highlighted shortcut footer.
pub fn view(app: &App, frame: &mut Frame) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(frame.area());

    match app.view {
        View::Sessions => render_sessions(app, frame, rows[0]),
        View::Calls => render_calls(app, frame, rows[0]),
        View::Detail => render_detail(app, frame, rows[0]),
    }
    render_footer(app, frame, rows[1]);
}

// ── Session list ──────────────────────────────────────────────────────────────

fn render_sessions(app: &App, frame: &mut Frame, area: Rect) {
    let items: Vec<ListItem> = if app.sessions.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  no traces yet — enable Tap in `grove config`, then run explore calls",
            Style::default().fg(DIM),
        )))]
    } else {
        app.sessions
            .iter()
            .map(|s| {
                let live = if s.live { " ●live" } else { "" };
                let text = format!(
                    "{}  {:<22} {:<16} {:<10} {:>3} calls  {:>7} tok{}",
                    hms_utc(s.started_at),
                    truncate(&s.client, 22),
                    truncate(&s.model, 16),
                    truncate(&s.mode, 10),
                    s.calls.len(),
                    s.total_tokens(),
                    live,
                );
                let style = if s.live {
                    Style::default().fg(ACCENT)
                } else {
                    Style::default().fg(NORMAL)
                };
                ListItem::new(Line::from(Span::styled(text, style)))
            })
            .collect()
    };

    let list = List::new(items)
        .block(titled(" grove tap — trace sessions "))
        .highlight_style(Style::default().fg(FOCUSED).add_modifier(Modifier::BOLD))
        .highlight_symbol("▸ ");
    let mut state = ListState::default();
    if !app.sessions.is_empty() {
        state.select(Some(app.sel_session));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

// ── Call list ─────────────────────────────────────────────────────────────────

fn render_calls(app: &App, frame: &mut Frame, area: Rect) {
    let Some(s) = app.current_session() else {
        return render_sessions(app, frame, area);
    };
    let items: Vec<ListItem> = s
        .calls
        .iter()
        .map(|c| {
            let flag = if !c.ended {
                " …"
            } else if c.truncated {
                " ⚠"
            } else {
                ""
            };
            let text = format!(
                "#{:<3} {:<48} {:>2} turns  {:>6} tok  {:>6}ms{}",
                c.call_id,
                truncate(&c.query, 48),
                c.turns,
                c.tokens.total,
                c.wall_ms,
                flag,
            );
            ListItem::new(Line::from(Span::styled(text, Style::default().fg(NORMAL))))
        })
        .collect();

    let title = format!(
        " {} · {} · {} · {} @ {} ",
        truncate(&s.client, 20),
        s.model,
        s.mode,
        s.provider,
        s.base_url,
    );
    let list = List::new(items)
        .block(titled(&title))
        .highlight_style(Style::default().fg(FOCUSED).add_modifier(Modifier::BOLD))
        .highlight_symbol("▸ ");
    let mut state = ListState::default();
    if !s.calls.is_empty() {
        state.select(Some(app.sel_call));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

// ── Call detail ───────────────────────────────────────────────────────────────

fn render_detail(app: &App, frame: &mut Frame, area: Rect) {
    let Some(c) = app.current_call() else {
        return render_calls(app, frame, area);
    };
    let text = detail_lines(c).join("\n");
    let title = format!(" call #{} · {} ", c.call_id, truncate(&c.query, 50));
    let para = Paragraph::new(text)
        .block(titled(&title))
        .scroll((app.detail_scroll as u16, 0));
    frame.render_widget(para, area);
}

/// Build the scrollable detail text for a call: each turn rendered through the
/// shared request/response pretty-printers, then the answer. The call's status +
/// metrics live in the footer ([`call_metrics`]), not here.
pub fn detail_lines(c: &Call) -> Vec<String> {
    let mut out = Vec::new();

    for t in &c.turn_blocks {
        out.push(format!("── turn {} ──────────────────────────", t.turn_index));
        out.push(format_request(&t.request));
        out.push(format_response(&t.response, Some(t.wall_ms)));
        out.push(String::new());
    }

    if c.ended {
        out.push("── final answer ──────────────────────".to_string());
        out.push(if c.answer.is_empty() {
            "(empty)".to_string()
        } else {
            c.answer.clone()
        });
    } else {
        out.push("(call in progress — no final answer yet)".to_string());
    }

    // Split any multi-line blocks so scroll math is line-accurate.
    out.into_iter().flat_map(|b| b.lines().map(str::to_string).collect::<Vec<_>>()).collect()
}

// ── Footer ────────────────────────────────────────────────────────────────────

/// The call's status + metrics, shown in the detail-view footer.
pub fn call_metrics(c: &Call) -> String {
    let status = if !c.ended {
        "● in progress…"
    } else if c.truncated {
        "⚠ truncated (turn cap)"
    } else {
        "✓ done"
    };
    format!(
        "{status} · {} turns · tok {}/{}/{} (p/c/total) · {}ms",
        c.turns, c.tokens.prompt, c.tokens.completion, c.tokens.total, c.wall_ms,
    )
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    // In the detail view the footer carries the call's status + metrics (with a
    // short key hint); elsewhere it's the navigation shortcuts.
    let text = match app.view {
        View::Sessions => " ↑↓ move · Enter open · q/Esc quit ".to_string(),
        View::Calls => " ↑↓ move · Enter open · Esc back · q quit ".to_string(),
        View::Detail => match app.current_call() {
            Some(c) => format!(" {}  │  ↑↓/PgUp/PgDn scroll · Esc back · q quit ", call_metrics(c)),
            None => " ↑↓/PgUp/PgDn scroll · Esc back · q quit ".to_string(),
        },
    };
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

fn titled(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .title(title.to_string())
}

/// Char-safe truncation with an ellipsis (labels may contain multibyte text).
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
    t.push('…');
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace_tui::model::{Tokens, Turn};
    use serde_json::json;

    fn call() -> Call {
        Call {
            call_id: 1,
            query: "where is main".into(),
            turns: 1,
            tokens: Tokens { prompt: 10, completion: 2, total: 12 },
            wall_ms: 50,
            answer: "src/main.rs:1".into(),
            truncated: false,
            ended: true,
            turn_blocks: vec![Turn {
                turn_index: 1,
                request: json!({"model": "qwen", "messages": [{"role": "user", "content": "where is main"}]}),
                response: json!({"choices": [{"message": {"role": "assistant", "content": "in main.rs"}}]}),
                wall_ms: 40,
            }],
        }
    }

    #[test]
    fn detail_lines_include_turn_and_answer_not_metrics() {
        let lines = detail_lines(&call());
        let joined = lines.join("\n");
        assert!(joined.contains("turn 1"), "turn header present");
        assert!(joined.contains("where is main"), "request rendered");
        assert!(joined.contains("final answer"), "answer section present");
        assert!(joined.contains("src/main.rs:1"), "answer text present");
        // Metrics moved to the footer — not in the scrollable body.
        assert!(!joined.contains("tok "), "metrics must not be in the body");
    }

    #[test]
    fn call_metrics_reports_status_and_totals() {
        let done = call_metrics(&call());
        assert!(done.contains("done"), "ended call shows done: {done}");
        assert!(done.contains("tok 10/2/12"), "token counts present: {done}");
        assert!(done.contains("50ms"), "wall time present: {done}");

        let mut inflight = call();
        inflight.ended = false;
        assert!(call_metrics(&inflight).contains("in progress"), "live status");

        let mut trunc = call();
        trunc.truncated = true;
        assert!(call_metrics(&trunc).contains("truncated"), "truncated status");
    }

    #[test]
    fn detail_lines_are_line_split_for_scroll() {
        // Every element must be a single line (no embedded newlines) so the
        // scroll offset maps 1:1 to visible rows.
        for l in detail_lines(&call()) {
            assert!(!l.contains('\n'), "line still contains a newline: {l:?}");
        }
    }

    #[test]
    fn truncate_is_char_safe() {
        assert_eq!(truncate("hello", 10), "hello");
        assert!(truncate(&"é".repeat(40), 10).ends_with('…'));
    }
}
