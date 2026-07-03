//! Full-screen ratatui trace browser (`grove tap`) — entry point + event loop.
//!
//! Drills through recorded explore sessions: session list → call list → per-call
//! turn detail. Reloads from `.grove/traces/` on a tick so a live `grove serve`
//! session streams in. Structured like `config_tui` (model/update/view/mod).

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

use model::{Action, App, Msg};

/// Launch the trace browser rooted at `root`. Returns an error only for I/O
/// failures and non-TTY environments.
pub fn run(root: &Path) -> Result<()> {
    if !io::stdout().is_terminal() {
        anyhow::bail!(
            "`grove tap` requires an interactive terminal — pipe/redirect detected. \
             Run it in a real terminal session."
        );
    }

    let mut app = App::new(root);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app);

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| view::view(app, f))?;

        // Poll so the session list refreshes on a tick even without keystrokes.
        if !event::poll(std::time::Duration::from_millis(500))? {
            let sessions = model::load_sessions(&app.root);
            if let Some(Action::Quit) = update::update(app, Msg::Reload(sessions)) {
                return Ok(());
            }
            continue;
        }
        let ev = event::read()?;
        if let Some(msg) = translate(app, ev) {
            if let Some(Action::Quit) = update::update(app, msg) {
                return Ok(());
            }
        }
    }
}

/// Translate a crossterm event into an optional [`Msg`].
fn translate(_app: &App, ev: Event) -> Option<Msg> {
    let Event::Key(KeyEvent { code, modifiers, .. }) = ev else {
        return None;
    };
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Msg::Quit);
    }
    match code {
        KeyCode::Up | KeyCode::Char('k') => Some(Msg::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(Msg::Down),
        KeyCode::Enter => Some(Msg::Enter),
        KeyCode::Right | KeyCode::Char('l') => Some(Msg::Right),
        KeyCode::Left | KeyCode::Char('h') => Some(Msg::Left),
        KeyCode::Esc | KeyCode::Backspace => Some(Msg::Back),
        KeyCode::PageUp => Some(Msg::PageUp),
        KeyCode::PageDown => Some(Msg::PageDown),
        KeyCode::Home | KeyCode::Char('g') => Some(Msg::Top),
        KeyCode::End | KeyCode::Char('G') => Some(Msg::Bottom),
        KeyCode::Char('q') => Some(Msg::Quit),
        _ => None,
    }
}
