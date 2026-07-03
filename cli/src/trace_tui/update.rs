//! Pure state transitions for the trace browser (Elm-style Update layer).

use crate::trace_tui::model::{Action, App, Msg, NodeKey, View};

/// How many nodes a PageUp/PageDown moves the detail-tree cursor.
const PAGE: usize = 10;

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
            View::Detail => app.tree_cursor = app.tree_cursor.saturating_sub(1),
        },
        Msg::Down => match app.view {
            View::Sessions => app.sel_session = (app.sel_session + 1).min(top_session(app)),
            View::Calls => app.sel_call = (app.sel_call + 1).min(top_call(app)),
            View::Detail => app.tree_cursor = (app.tree_cursor + 1).min(top_node(app)),
        },

        // Enter: descend a level; toggle the node in the detail tree.
        Msg::Enter => match app.view {
            View::Sessions => descend_to_calls(app),
            View::Calls => descend_to_detail(app),
            View::Detail => toggle_node(app),
        },

        // Right (→): descend / expand.
        Msg::Right => match app.view {
            View::Sessions => descend_to_calls(app),
            View::Calls => descend_to_detail(app),
            View::Detail => set_expanded(app, true),
        },

        // Left (←): go back a level / collapse the node (or ascend if already collapsed).
        Msg::Left => match app.view {
            View::Sessions => {}
            View::Calls => app.view = View::Sessions,
            View::Detail => {
                if is_cursor_expanded(app) {
                    set_expanded(app, false);
                } else {
                    app.view = View::Calls;
                }
            }
        },

        // Esc/Backspace: always up a level (quit at the top).
        Msg::Back => match app.view {
            View::Sessions => return Some(Action::Quit),
            View::Calls => app.view = View::Sessions,
            View::Detail => app.view = View::Calls,
        },

        Msg::PageUp => match app.view {
            View::Detail => app.tree_cursor = app.tree_cursor.saturating_sub(PAGE),
            View::Sessions => app.sel_session = app.sel_session.saturating_sub(PAGE),
            View::Calls => app.sel_call = app.sel_call.saturating_sub(PAGE),
        },
        Msg::PageDown => match app.view {
            View::Detail => app.tree_cursor = (app.tree_cursor + PAGE).min(top_node(app)),
            View::Sessions => app.sel_session = (app.sel_session + PAGE).min(top_session(app)),
            View::Calls => app.sel_call = (app.sel_call + PAGE).min(top_call(app)),
        },
        Msg::Top => match app.view {
            View::Detail => app.tree_cursor = 0,
            View::Sessions => app.sel_session = 0,
            View::Calls => app.sel_call = 0,
        },
        Msg::Bottom => match app.view {
            View::Detail => app.tree_cursor = top_node(app),
            View::Sessions => app.sel_session = top_session(app),
            View::Calls => app.sel_call = top_call(app),
        },
    }
    None
}

fn descend_to_calls(app: &mut App) {
    if app.current_session().is_some_and(|s| !s.calls.is_empty()) {
        app.view = View::Calls;
        app.sel_call = 0;
    }
}

/// Enter a call's turn tree: reset the cursor and collapse everything.
fn descend_to_detail(app: &mut App) {
    if app.current_call().is_some() {
        app.view = View::Detail;
        app.tree_cursor = 0;
        app.expanded.clear();
    }
}

fn toggle_node(app: &mut App) {
    if let Some(key) = app.cursor_key() {
        if app.expanded.contains(&key) {
            app.expanded.remove(&key);
        } else {
            app.expanded.insert(key);
        }
    }
}

fn set_expanded(app: &mut App, want: bool) {
    if let Some(key) = app.cursor_key() {
        if want {
            app.expanded.insert(key);
        } else {
            app.expanded.remove(&key);
        }
    }
}

fn is_cursor_expanded(app: &App) -> bool {
    app.cursor_key()
        .map(|k: NodeKey| app.expanded.contains(&k))
        .unwrap_or(false)
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
    app.tree_cursor = app.tree_cursor.min(top_node(app));
}

fn top_session(app: &App) -> usize {
    app.sessions.len().saturating_sub(1)
}

fn top_call(app: &App) -> usize {
    app.current_session()
        .map(|s| s.calls.len().saturating_sub(1))
        .unwrap_or(0)
}

/// Last selectable node index in the current call's turn tree.
fn top_node(app: &App) -> usize {
    app.selectable_keys().len().saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace_tui::model::{Call, NodeKey, Session, Tokens, Turn};
    use serde_json::json;
    use std::path::Path;

    fn call_with_turns(n: usize) -> Call {
        Call {
            call_id: 0,
            query: "q".into(),
            turns: n,
            tokens: Tokens::default(),
            wall_ms: 0,
            answer: "ans".into(),
            truncated: false,
            ended: true,
            turn_blocks: (1..=n)
                .map(|i| Turn {
                    turn_index: i,
                    request: json!({"messages": []}),
                    response: json!({"choices":[{"message":{"role":"assistant","content":"hi"}}]}),
                    wall_ms: 10,
                })
                .collect(),
        }
    }

    /// An app parked in the detail view of a single call with `n` turns.
    fn detail_app(n: usize) -> App {
        let mut app = App::new(Path::new("/nonexistent"));
        let mut s = session("s", 0);
        s.calls.push(call_with_turns(n));
        app.sessions = vec![s];
        update(&mut app, Msg::Enter); // → Calls
        update(&mut app, Msg::Enter); // → Detail
        app
    }

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
    fn detail_starts_collapsed_with_turn_and_answer_nodes() {
        let app = detail_app(2);
        assert_eq!(app.view, View::Detail);
        assert!(app.expanded.is_empty(), "entering the tree collapses everything");
        // 2 turns + final answer = 3 selectable nodes.
        assert_eq!(app.selectable_keys(), vec![NodeKey::Turn(1), NodeKey::Turn(2), NodeKey::Answer]);
        assert_eq!(app.cursor_key(), Some(NodeKey::Turn(1)));
    }

    #[test]
    fn enter_toggles_a_turn_and_reveals_request_response() {
        let mut app = detail_app(2);
        update(&mut app, Msg::Enter); // expand Turn(1)
        assert!(app.expanded.contains(&NodeKey::Turn(1)));
        // Request/response nodes now sit under the expanded turn.
        assert_eq!(
            app.selectable_keys(),
            vec![NodeKey::Turn(1), NodeKey::Req(1), NodeKey::Resp(1), NodeKey::Turn(2), NodeKey::Answer]
        );
        update(&mut app, Msg::Enter); // collapse again
        assert!(!app.expanded.contains(&NodeKey::Turn(1)));
    }

    #[test]
    fn left_collapses_then_walks_back_out() {
        let mut app = detail_app(1);
        update(&mut app, Msg::Right); // expand Turn(1)
        assert!(app.expanded.contains(&NodeKey::Turn(1)));
        update(&mut app, Msg::Left); // collapse (still in the tree)
        assert_eq!(app.view, View::Detail);
        assert!(!app.expanded.contains(&NodeKey::Turn(1)));
        update(&mut app, Msg::Left); // nothing expanded → back to Calls
        assert_eq!(app.view, View::Calls);
    }

    #[test]
    fn detail_cursor_clamps_to_node_count() {
        let mut app = detail_app(2); // 3 nodes
        for _ in 0..10 {
            update(&mut app, Msg::Down);
        }
        assert_eq!(app.cursor_key(), Some(NodeKey::Answer), "clamps at the last node");
        update(&mut app, Msg::Top);
        assert_eq!(app.cursor_key(), Some(NodeKey::Turn(1)));
    }
}
