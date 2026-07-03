//! Full-screen ratatui config TUI — entry point + event loop.
//!
//! `run(root, maybe_cfg)` is the only public surface. It:
//!   1. Fast-fails with a clear message when stdout is not a TTY.
//!   2. Enters the alternate screen in raw mode.
//!   3. Runs the Elm-style event loop (decode event → Msg → update → draw).
//!   4. Restores the terminal on **every** exit path (save, cancel, error).

pub mod model;
pub mod update;
pub mod view;

use std::io::{self, IsTerminal};
use std::path::Path;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use grove_core::ExploreConfig;

use model::{Action, App, Field, Msg};

/// Launch the config TUI.
///
/// * `root` — project root used by [`ExploreConfig::save`].
/// * `existing` — pre-populated config loaded by the caller; `None` for defaults.
///
/// Returns `Ok(())` on save or cancel. Returns an error for I/O failures and
/// (importantly) for non-TTY environments.
pub fn run(root: &Path, existing: Option<ExploreConfig>) -> Result<()> {
    // ── Non-TTY guard ────────────────────────────────────────────────────────
    if !io::stdout().is_terminal() {
        anyhow::bail!(
            "`grove config` requires an interactive terminal — \
             pipe output or redirect detected. \
             Use `grove config` in a real terminal session."
        );
    }

    // ── Initialise TUI state ─────────────────────────────────────────────────
    let mut app = match existing {
        Some(cfg) => App::from_config(cfg),
        None => App::default(),
    };

    // ── Set up terminal ──────────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ── Event loop ───────────────────────────────────────────────────────────
    let result = event_loop(&mut terminal, &mut app, root);

    // ── Restore terminal — on ALL exit paths ─────────────────────────────────
    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    root: &Path,
) -> Result<()> {
    loop {
        if app.show_logs {
            refresh_logs(app, root);
            // Keep the scroll offset valid; pin to the bottom while following.
            let h = terminal.size()?.height.saturating_sub(2) as usize; // minus borders
            let max_scroll = app.logs.len().saturating_sub(h);
            if app.log_follow {
                app.log_scroll = max_scroll;
            } else {
                app.log_scroll = app.log_scroll.min(max_scroll);
                if app.log_scroll >= max_scroll {
                    app.log_follow = true; // scrolled to the bottom → re-attach
                }
            }
        }
        terminal.draw(|f| view::view(app, f))?;

        // Poll rather than block, so the live trace view refreshes on a tick
        // even without keystrokes.
        if !event::poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        let event = event::read()?;
        if let Some(msg) = translate_event(app, event) {
            match update::update(app, msg) {
                Some(Action::Save) => {
                    match app.to_config() {
                        Ok(cfg) => {
                            cfg.save(root)?;
                            return Ok(());
                        }
                        Err(e) => {
                            // Stay in the TUI; show the error in the status bar.
                            app.last_error = Some(e.to_string());
                        }
                    }
                }
                Some(Action::Quit) => return Ok(()),
                None => {
                    // Clear a previous error whenever the user types.
                    app.last_error = None;
                }
            }
        }
    }
}

/// Refresh the in-memory tail of the trace log for the live view (last ~1000
/// lines; a hint when the file doesn't exist yet).
fn refresh_logs(app: &mut App, root: &Path) {
    let path = grove_core::explore::trace::trace_path(root);
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let lines: Vec<String> = s.lines().map(str::to_string).collect();
            let start = lines.len().saturating_sub(1000);
            app.logs = lines[start..].to_vec();
        }
        Err(_) => {
            app.logs = vec![
                format!("(no trace yet at {})", path.display()),
                "Enable Tap above, save, then run an explore call to see traffic here."
                    .to_string(),
            ];
        }
    }
}

/// Translate a crossterm `Event` into an optional `Msg`, taking the current
/// focus field into account so field-specific key bindings apply only when
/// that field is active.
fn translate_event(app: &App, event: Event) -> Option<Msg> {
    let Event::Key(KeyEvent { code, modifiers, .. }) = event else {
        return None;
    };

    // In the live trace-log view, keys scroll the log or navigate back out.
    if app.show_logs {
        return match code {
            KeyCode::F(3) | KeyCode::Esc | KeyCode::Char('l') => Some(Msg::ToggleLogs),
            KeyCode::Up | KeyCode::Char('k') => Some(Msg::LogUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Msg::LogDown),
            KeyCode::PageUp => Some(Msg::LogPageUp),
            KeyCode::PageDown => Some(Msg::LogPageDown),
            KeyCode::Home | KeyCode::Char('g') => Some(Msg::LogTop),
            KeyCode::End | KeyCode::Char('G') => Some(Msg::LogBottom),
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
            _ => None,
        };
    }

    // Global bindings that apply in any field.
    match code {
        KeyCode::Tab => return Some(Msg::TabNext),
        KeyCode::BackTab => return Some(Msg::TabPrev),
        KeyCode::F(2) => return Some(Msg::Save),
        KeyCode::F(3) => return Some(Msg::ToggleLogs),
        KeyCode::Esc => return Some(Msg::Quit),
        KeyCode::Char('s') if modifiers.is_empty() && app.focus != Field::Url && app.focus != Field::Model && app.focus != Field::Tools => {
            return Some(Msg::Save);
        }
        KeyCode::Char('q') if modifiers.is_empty() && app.focus != Field::Url && app.focus != Field::Model && app.focus != Field::Tools => {
            return Some(Msg::Quit);
        }
        _ => {}
    }

    // Ctrl-C is a universal quit.
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Msg::Quit);
    }

    // Field-specific bindings.
    match app.focus {
        Field::Provider => match code {
            KeyCode::Up | KeyCode::Char('k') => Some(Msg::ProviderUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Msg::ProviderDown),
            _ => None,
        },
        Field::Url => match code {
            KeyCode::Char(c) => Some(Msg::UrlChar(c)),
            KeyCode::Backspace => Some(Msg::UrlBackspace),
            _ => None,
        },
        Field::Model => match code {
            KeyCode::Char(c) => Some(Msg::ModelChar(c)),
            KeyCode::Backspace => Some(Msg::ModelBackspace),
            _ => None,
        },
        Field::Mode => match code {
            KeyCode::Up | KeyCode::Char('k') => Some(Msg::ModeUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Msg::ModeDown),
            _ => None,
        },
        Field::Tap => match code {
            KeyCode::Char(' ') | KeyCode::Enter => Some(Msg::TapToggle),
            _ => None,
        },
        Field::Tools => match code {
            KeyCode::Up | KeyCode::Char('k') => Some(Msg::ToolsUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Msg::ToolsDown),
            KeyCode::Char(' ') => Some(Msg::ToolsToggle),
            KeyCode::Enter => Some(Msg::ToolsAddConfirm),
            KeyCode::Backspace => Some(Msg::ToolsAddBackspace),
            KeyCode::Char(c) => Some(Msg::ToolsAddChar(c)),
            _ => None,
        },
    }
}
