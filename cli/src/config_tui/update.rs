//! Pure state transitions for the config TUI (Elm-style Update layer).
//!
//! `update()` takes an `App` by mutable reference and a `Msg`, applies the
//! transition in place, and returns an optional `Action` when the event loop
//! should perform a side-effect (save, quit, or fetch the model list).

use crate::config_tui::model::{Action, App, Msg, LLAMACPP_DEFAULT_URL, OLLAMA_DEFAULT_URL};

/// Apply `msg` to `app`; return `Some(Action)` if the loop should act.
pub fn update(app: &mut App, msg: Msg) -> Option<Action> {
    match msg {
        Msg::TabNext => {
            app.model_dropdown = false;
            app.focus = app.focus.next();
            None
        }
        Msg::TabPrev => {
            app.model_dropdown = false;
            app.focus = app.focus.prev();
            None
        }

        // ── Provider ────────────────────────────────────────────────────────
        Msg::ProviderUp => {
            app.provider = app.provider.saturating_sub(1);
            apply_provider_default(app);
            None
        }
        Msg::ProviderDown => {
            app.provider = (app.provider + 1).min(1);
            apply_provider_default(app);
            None
        }

        // ── URL text buffer ──────────────────────────────────────────────────
        Msg::UrlChar(c) => {
            app.base_url.push(c);
            app.dirty_url = true;
            None
        }
        Msg::UrlBackspace => {
            app.base_url.pop();
            app.dirty_url = true;
            None
        }

        // ── Model text buffer + dropdown ─────────────────────────────────────
        Msg::ModelChar(c) => {
            app.model.push(c);
            app.model_cursor = 0; // re-filter from the top
            None
        }
        Msg::ModelBackspace => {
            app.model.pop();
            app.model_cursor = 0;
            None
        }
        Msg::ModelDropdownOpen => {
            app.model_dropdown = true;
            app.model_cursor = 0;
            app.model_status = Some("fetching models…".to_string());
            Some(Action::FetchModels)
        }
        Msg::ModelUp => {
            app.model_cursor = app.model_cursor.saturating_sub(1);
            None
        }
        Msg::ModelDown => {
            let last = app.model_filtered().len().saturating_sub(1);
            app.model_cursor = (app.model_cursor + 1).min(last);
            None
        }
        Msg::ModelSelect => {
            if let Some(pick) = app.model_filtered().get(app.model_cursor) {
                app.model = pick.clone();
            }
            app.model_dropdown = false;
            None
        }
        Msg::ModelClose => {
            app.model_dropdown = false;
            None
        }
        Msg::ModelListFetched(list) => {
            app.model_status = if list.is_empty() {
                Some("no models reported — type one".to_string())
            } else {
                None
            };
            app.model_list = list;
            app.model_cursor = 0;
            None
        }
        Msg::ModelFetchError(e) => {
            app.model_list.clear();
            app.model_status = Some(format!("fetch failed ({e}) — type a model"));
            None
        }

        // ── Mode ─────────────────────────────────────────────────────────────
        Msg::ModeUp => {
            app.mode = app.mode.saturating_sub(1);
            None
        }
        Msg::ModeDown => {
            app.mode = (app.mode + 1).min(2);
            None
        }

        // ── Tools ─────────────────────────────────────────────────────────────
        Msg::ToolsUp => {
            app.tool_cursor = app.tool_cursor.saturating_sub(1);
            None
        }
        Msg::ToolsDown => {
            if !app.tools.is_empty() {
                app.tool_cursor = (app.tool_cursor + 1).min(app.tools.len() - 1);
            }
            None
        }
        Msg::ToolsToggle => {
            if let Some(entry) = app.tools.get_mut(app.tool_cursor) {
                entry.1 = !entry.1;
            }
            None
        }
        Msg::ToolsAddChar(c) => {
            app.add_tool_buf.push(c);
            None
        }
        Msg::ToolsAddBackspace => {
            app.add_tool_buf.pop();
            None
        }
        Msg::ToolsAddConfirm => {
            let name = app.add_tool_buf.trim().to_string();
            if !name.is_empty() {
                app.tools.push((name, true));
                app.tool_cursor = app.tools.len() - 1;
                app.add_tool_buf.clear();
            }
            None
        }

        // ── Tap ───────────────────────────────────────────────────────────────
        Msg::TapToggle => {
            app.tap = !app.tap;
            None
        }

        // ── Terminal actions ──────────────────────────────────────────────────
        Msg::Save => Some(Action::Save),
        Msg::Quit => Some(Action::Quit),
    }
}

/// When the provider changes and the user has NOT manually edited the URL,
/// auto-fill the default endpoint for the new provider.
fn apply_provider_default(app: &mut App) {
    if app.dirty_url {
        return;
    }
    app.base_url = match app.provider {
        0 => OLLAMA_DEFAULT_URL.to_string(),
        _ => LLAMACPP_DEFAULT_URL.to_string(),
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Headless unit tests (no terminal required)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_tui::model::Field;
    use grove_core::ExploreConfig;

    fn fresh() -> App {
        App::default()
    }

    // 1. Tab navigation — forward and backward, wrapping.
    #[test]
    fn tab_navigation_forward_wraps() {
        let mut app = fresh();
        assert_eq!(app.focus, Field::Provider);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Url);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Model);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Mode);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Tap);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Tools);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Provider); // wrapped
    }

    #[test]
    fn tab_navigation_backward_wraps() {
        let mut app = fresh();
        assert_eq!(app.focus, Field::Provider);
        update(&mut app, Msg::TabPrev);
        assert_eq!(app.focus, Field::Tools); // wrapped
    }

    #[test]
    fn tap_toggles() {
        let mut app = fresh();
        assert!(!app.tap);
        update(&mut app, Msg::TapToggle);
        assert!(app.tap, "tap flips on");
        update(&mut app, Msg::TapToggle);
        assert!(!app.tap, "tap flips off");
    }

    // 2. Provider selection drives URL default (unless URL is dirty).
    #[test]
    fn provider_switch_sets_url_default() {
        let mut app = fresh();
        assert_eq!(app.provider, 0); // Ollama
        assert_eq!(app.base_url, OLLAMA_DEFAULT_URL);

        update(&mut app, Msg::ProviderDown); // → LlamaCpp
        assert_eq!(app.provider, 1);
        assert_eq!(app.base_url, LLAMACPP_DEFAULT_URL);

        update(&mut app, Msg::ProviderUp); // → Ollama
        assert_eq!(app.provider, 0);
        assert_eq!(app.base_url, OLLAMA_DEFAULT_URL);
    }

    #[test]
    fn dirty_url_not_clobbered_on_provider_switch() {
        let mut app = fresh();
        // Manually edit URL → dirty_url = true
        update(&mut app, Msg::UrlChar('x'));
        let before = app.base_url.clone();
        // Switch provider
        update(&mut app, Msg::ProviderDown);
        assert_eq!(app.base_url, before, "dirty URL must not be overwritten");
    }

    // 3. Mode cycling.
    #[test]
    fn mode_cycling() {
        let mut app = fresh();
        assert_eq!(app.mode, 0); // Standard
        update(&mut app, Msg::ModeDown);
        assert_eq!(app.mode, 1); // Balanced
        update(&mut app, Msg::ModeDown);
        assert_eq!(app.mode, 2); // Aggressive
        update(&mut app, Msg::ModeDown); // already max
        assert_eq!(app.mode, 2);
        update(&mut app, Msg::ModeUp);
        assert_eq!(app.mode, 1);
    }

    // 4. Tool toggle.
    #[test]
    fn tool_toggle_flips_selected() {
        let mut app = fresh();
        // Default tools are all selected (true)
        assert!(app.tools[0].1, "first tool should start selected");
        update(&mut app, Msg::ToolsToggle);
        assert!(!app.tools[0].1, "should be deselected after toggle");
        update(&mut app, Msg::ToolsToggle);
        assert!(app.tools[0].1, "should be reselected after second toggle");
    }

    // 5. Model auto-discovery dropdown.
    #[test]
    fn model_dropdown_open_requests_fetch() {
        let mut app = fresh();
        let action = update(&mut app, Msg::ModelDropdownOpen);
        assert_eq!(action, Some(Action::FetchModels));
        assert!(app.model_dropdown, "dropdown is now open");
        assert!(app.model_status.is_some(), "shows a fetching status");
    }

    #[test]
    fn model_list_fetch_populates_and_select_sets_model() {
        let mut app = fresh();
        update(&mut app, Msg::ModelDropdownOpen);
        update(
            &mut app,
            Msg::ModelListFetched(vec!["qwen2.5-coder:7b".into(), "llama3:8b".into()]),
        );
        assert!(app.model_status.is_none(), "status cleared on non-empty list");
        // Filtering: typing narrows the list.
        app.model.clear();
        for c in "llama".chars() {
            update(&mut app, Msg::ModelChar(c));
        }
        assert_eq!(app.model_filtered(), vec!["llama3:8b".to_string()]);
        update(&mut app, Msg::ModelSelect);
        assert_eq!(app.model, "llama3:8b");
        assert!(!app.model_dropdown, "selecting closes the dropdown");
    }

    #[test]
    fn model_fetch_error_falls_back_to_free_text() {
        let mut app = fresh();
        update(&mut app, Msg::ModelDropdownOpen);
        update(&mut app, Msg::ModelFetchError("connection refused".into()));
        assert!(app.model_list.is_empty());
        assert!(app.model_status.as_deref().unwrap().contains("fetch failed"));
        // Free-text entry still works.
        update(&mut app, Msg::ModelChar('x'));
        assert!(app.model.ends_with('x'));
    }

    #[test]
    fn tab_closes_open_dropdown() {
        let mut app = fresh();
        app.model_dropdown = true;
        update(&mut app, Msg::TabNext);
        assert!(!app.model_dropdown, "moving focus closes the dropdown");
    }

    // 6. Save / quit actions.
    #[test]
    fn save_returns_action() {
        let mut app = fresh();
        let action = update(&mut app, Msg::Save);
        assert_eq!(action, Some(Action::Save));
    }

    #[test]
    fn quit_returns_action_without_mutation() {
        let mut app = fresh();
        let orig_url = app.base_url.clone();
        let action = update(&mut app, Msg::Quit);
        assert_eq!(action, Some(Action::Quit));
        assert_eq!(app.base_url, orig_url, "quit must not mutate state");
    }

    // 7. App::to_config() round-trip.
    #[test]
    fn to_config_round_trip() {
        let app = fresh();
        let cfg = app.to_config().expect("default App should produce a valid config");
        assert_eq!(cfg, ExploreConfig::default());
    }

    // 8. Empty required field fails validation.
    #[test]
    fn to_config_fails_on_blank_model() {
        let mut app = fresh();
        app.model.clear();
        let err = app.to_config().unwrap_err();
        assert!(err.to_string().contains("model"), "error should name the field: {err}");
    }

    #[test]
    fn to_config_fails_on_blank_url() {
        let mut app = fresh();
        app.base_url.clear();
        let err = app.to_config().unwrap_err();
        assert!(err.to_string().contains("base_url"), "error should name the field: {err}");
    }

    // 9. App::from_config() pre-populates all fields.
    #[test]
    fn from_config_pre_populates() {
        let cfg = ExploreConfig {
            provider: grove_core::Provider::LlamaCpp,
            base_url: "http://localhost:8080/v1".to_string(),
            model: "llama3".to_string(),
            steering: grove_core::Steering::Aggressive,
            allowed_tools: vec!["grove".to_string()],
            tap: true,
            trace_retain: 25,
        };
        let app = App::from_config(cfg.clone());
        assert_eq!(app.provider, 1, "LlamaCpp should map to index 1");
        assert_eq!(app.base_url, "http://localhost:8080/v1");
        assert_eq!(app.model, "llama3");
        assert_eq!(app.mode, 2, "Aggressive should map to index 2");
        assert_eq!(app.tools, vec![("grove".to_string(), true)]);
        assert_eq!(app.trace_retain, 25, "retention carried through unchanged");
        assert!(app.dirty_url, "loaded config must set dirty_url=true");

        // Round-trip back
        let back = app.to_config().unwrap();
        assert_eq!(back, cfg);
    }
}
