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

use grove_core::{config::GroveConfig, ExploreConfig};

use model::{Action, App, Field, Msg};

/// Launch the config TUI.
///
/// * `root` — project root used by [`GroveConfig::save`].
/// * `grove_cfg` — pre-populated config loaded by the caller; `None` for defaults (McpLlm active).
///
/// Returns `Ok(())` on save or cancel. Returns an error for I/O failures and
/// (importantly) for non-TTY environments.
pub fn run(root: &Path, grove_cfg: Option<GroveConfig>) -> Result<()> {
    // ── Non-TTY guard ────────────────────────────────────────────────────────
    if !io::stdout().is_terminal() {
        anyhow::bail!(
            "`grove config` requires an interactive terminal — \
             pipe output or redirect detected. \
             Use `grove config` in a real terminal session."
        );
    }

    // ── Initialise TUI state ─────────────────────────────────────────────────
    let mut app = match grove_cfg {
        Some(cfg) => App::from_grove_config(cfg),
        None => App::default(), // McpLlm-active default; used by init first-run
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
        terminal.draw(|f| view::view(app, f))?;

        let event = event::read()?;
        let Some(msg) = translate_event(app, event) else {
            continue;
        };
        match update::update(app, msg) {
            Some(Action::Save) => match app.to_config() {
                Ok(explore_cfg) => {
                    // Preserve the project's configured harness set — the config
                    // TUI only edits the explore surface, not which agents are wired.
                    let harnesses = GroveConfig::load(root)
                        .map(|c| c.harnesses)
                        .unwrap_or_else(|_| grove_core::config::default_harnesses());
                    let cfg = GroveConfig {
                        version: 1,
                        mode: app.grove_mode,
                        explore: Some(explore_cfg),
                        harnesses,
                    };
                    cfg.save(root)?;
                    return Ok(());
                }
                Err(e) => {
                    // Stay in the TUI; show the error in the status bar.
                    app.last_error = Some(e.to_string());
                }
            },
            Some(Action::Quit) => return Ok(()),
            Some(Action::FetchModels) => fetch_models(app),
            None => {
                // Clear a previous error whenever the user types.
                app.last_error = None;
            }
        }
    }
}

/// Perform the blocking model-list fetch and feed the result back as a `Msg`.
/// Only `base_url` matters to the provider's `/models` listing.
fn fetch_models(app: &mut App) {
    let cfg = ExploreConfig {
        base_url: app.base_url.clone(),
        model: app.model.clone(),
        ..ExploreConfig::default()
    };
    let msg = match grove_core::explore::list_models(&cfg) {
        Ok(list) => Msg::ModelListFetched(list),
        Err(e) => Msg::ModelFetchError(e),
    };
    update::update(app, msg);
}

/// Translate a crossterm `Event` into an optional `Msg`, taking the current
/// focus field into account so field-specific key bindings apply only when
/// that field is active.
fn translate_event(app: &App, event: Event) -> Option<Msg> {
    let Event::Key(KeyEvent { code, modifiers, .. }) = event else {
        return None;
    };

    // Ctrl-C is a universal quit.
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Msg::Quit);
    }

    // When the model dropdown is open, keys drive the picker (so Esc closes it
    // rather than quitting, and typing filters).
    if app.focus == Field::Model && app.model_dropdown {
        return match code {
            KeyCode::Tab => Some(Msg::TabNext),
            KeyCode::BackTab => Some(Msg::TabPrev),
            KeyCode::Up => Some(Msg::ModelUp),
            KeyCode::Down => Some(Msg::ModelDown),
            KeyCode::Enter => Some(Msg::ModelSelect),
            KeyCode::Esc => Some(Msg::ModelClose),
            KeyCode::Backspace => Some(Msg::ModelBackspace),
            KeyCode::Char(c) => Some(Msg::ModelChar(c)),
            _ => None,
        };
    }

    // Global bindings that apply in any field.
    match code {
        KeyCode::Tab => return Some(Msg::TabNext),
        KeyCode::BackTab => return Some(Msg::TabPrev),
        KeyCode::F(2) => return Some(Msg::Save),
        KeyCode::Esc => return Some(Msg::Quit),
        KeyCode::Char('s')
            if modifiers.is_empty()
                && app.focus != Field::Url
                && app.focus != Field::Model
                && app.focus != Field::Tools =>
        {
            return Some(Msg::Save);
        }
        KeyCode::Char('q')
            if modifiers.is_empty()
                && app.focus != Field::Url
                && app.focus != Field::Model
                && app.focus != Field::Tools =>
        {
            return Some(Msg::Quit);
        }
        _ => {}
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
            // Down opens the auto-discovery dropdown (and triggers a fetch).
            KeyCode::Down => Some(Msg::ModelDropdownOpen),
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
