//! Pure state transitions for the trace browser (Elm-style Update layer).

use crate::trace_tui::model::{Action, App, Msg, View};

/// How many lines a PageUp/PageDown moves the detail view.
const PAGE: usize = 15;

/// Apply `msg` to `app`; return `Some(Action)` when the loop should terminate.
pub fn update(app: &mut App, msg: Msg) -> Option<Action> {
    match msg {
        Msg::Quit => return Some(Action::Quit),

        Msg::Reload(sessions) => {
            reload(app, sessions);
        }

        Msg::Up => match app.view {
            View::Sessions => app.sel_session = app.sel_session.saturating_sub(1),
            View::Calls => app.sel_call = app.sel_call.saturating_sub(1),
            View::Detail => {
                app.detail_follow = false;
                app.detail_scroll = app.detail_scroll.saturating_sub(1);
            }
        },
        Msg::Down => match app.view {
            View::Sessions => {
                app.sel_session = (app.sel_session + 1).min(top_session(app));
            }
            View::Calls => {
                app.sel_call = (app.sel_call + 1).min(top_call(app));
            }
            View::Detail => {
                // The event loop clamps to the real bottom and re-attaches follow.
                app.detail_scroll += 1;
            }
        },

        Msg::Enter => match app.view {
            View::Sessions => {
                if app.current_session().is_some_and(|s| !s.calls.is_empty()) {
                    app.view = View::Calls;
                    app.sel_call = 0;
                }
            }
            View::Calls => {
                if app.current_call().is_some() {
                    app.view = View::Detail;
                    app.detail_scroll = 0;
                    app.detail_follow = true;
                }
            }
            View::Detail => {}
        },

        Msg::Back => match app.view {
            View::Sessions => return Some(Action::Quit),
            View::Calls => app.view = View::Sessions,
            View::Detail => app.view = View::Calls,
        },

        Msg::PageUp => {
            if app.view == View::Detail {
                app.detail_follow = false;
                app.detail_scroll = app.detail_scroll.saturating_sub(PAGE);
            }
        }
        Msg::PageDown => {
            if app.view == View::Detail {
                app.detail_scroll += PAGE;
            }
        }
        Msg::Top => match app.view {
            View::Detail => {
                app.detail_follow = false;
                app.detail_scroll = 0;
            }
            View::Sessions => app.sel_session = 0,
            View::Calls => app.sel_call = 0,
        },
        Msg::Bottom => match app.view {
            View::Detail => app.detail_follow = true,
            View::Sessions => app.sel_session = top_session(app),
            View::Calls => app.sel_call = top_call(app),
        },
    }
    None
}

/// Replace the session list on a refresh, keeping the cursor on the same
/// session id where possible and clamping every index back into range.
fn reload(app: &mut App, sessions: Vec<crate::trace_tui::model::Session>) {
    // Preserve the selected session across the reload by id.
    let selected_id = app.current_session().map(|s| s.id.clone());
    app.sessions = sessions;
    if let Some(id) = selected_id {
        if let Some(pos) = app.sessions.iter().position(|s| s.id == id) {
            app.sel_session = pos;
        }
    }
    app.sel_session = app.sel_session.min(top_session(app));
    app.sel_call = app.sel_call.min(top_call(app));
}

fn top_session(app: &App) -> usize {
    app.sessions.len().saturating_sub(1)
}

fn top_call(app: &App) -> usize {
    app.current_session()
        .map(|s| s.calls.len().saturating_sub(1))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace_tui::model::{Call, Session, Tokens};
    use std::path::Path;

    fn session(id: &str, n_calls: usize) -> Session {
        Session {
            id: id.to_string(),
            started_at: 100,
            client: "x".into(),
            model: "m".into(),
            mode: "standard".into(),
            provider: "ollama".into(),
            base_url: "u".into(),
            calls: (0..n_calls)
                .map(|i| Call {
                    call_id: i as u32,
                    query: format!("q{i}"),
                    turns: 1,
                    tokens: Tokens::default(),
                    wall_ms: 0,
                    answer: String::new(),
                    truncated: false,
                    ended: true,
                    turn_blocks: Vec::new(),
                })
                .collect(),
            live: false,
        }
    }

    fn app_with(sessions: Vec<Session>) -> App {
        let mut app = App::new(Path::new("/nonexistent"));
        app.sessions = sessions;
        app
    }

    #[test]
    fn enter_and_back_walk_the_levels() {
        let mut app = app_with(vec![session("s1", 2)]);
        assert_eq!(app.view, View::Sessions);
        update(&mut app, Msg::Enter);
        assert_eq!(app.view, View::Calls);
        update(&mut app, Msg::Enter);
        assert_eq!(app.view, View::Detail);
        update(&mut app, Msg::Back);
        assert_eq!(app.view, View::Calls);
        update(&mut app, Msg::Back);
        assert_eq!(app.view, View::Sessions);
        // Back at the top level, Back quits.
        assert_eq!(update(&mut app, Msg::Back), Some(Action::Quit));
    }

    #[test]
    fn enter_on_empty_session_does_not_descend() {
        let mut app = app_with(vec![session("empty", 0)]);
        update(&mut app, Msg::Enter);
        assert_eq!(app.view, View::Sessions, "no calls → stay put");
    }

    #[test]
    fn navigation_clamps_to_bounds() {
        let mut app = app_with(vec![session("a", 1), session("b", 1), session("c", 1)]);
        update(&mut app, Msg::Up); // already at top
        assert_eq!(app.sel_session, 0);
        for _ in 0..10 {
            update(&mut app, Msg::Down);
        }
        assert_eq!(app.sel_session, 2, "clamps at the last session");
    }

    #[test]
    fn reload_preserves_selection_by_id() {
        let mut app = app_with(vec![session("a", 1), session("b", 1), session("c", 1)]);
        app.sel_session = 2; // "c"
        // A new session "z" arrives at the front; "c" shifts down.
        update(
            &mut app,
            Msg::Reload(vec![
                session("z", 1),
                session("a", 1),
                session("b", 1),
                session("c", 1),
            ]),
        );
        assert_eq!(app.current_session().unwrap().id, "c", "cursor tracks the id");
    }

    #[test]
    fn detail_scroll_releases_and_reattaches_follow() {
        let mut app = app_with(vec![session("s", 1)]);
        update(&mut app, Msg::Enter);
        update(&mut app, Msg::Enter);
        assert!(app.detail_follow);
        update(&mut app, Msg::Up);
        assert!(!app.detail_follow, "scrolling up releases follow");
        update(&mut app, Msg::Bottom);
        assert!(app.detail_follow, "End re-attaches follow");
    }
}
