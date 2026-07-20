//! Pure state transitions for the config TUI (Elm-style Update layer).
//!
//! `update()` takes an `App` by mutable reference and a `Msg`, applies the
//! transition in place, and returns an optional `Action` when the event loop
//! should perform a side-effect (save, quit, or fetch the model list).

use crate::config_tui::model::{Action, App, Msg};

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

        // ── Engine picker ────────────────────────────────────────────────────
        Msg::EngineUp => {
            app.engine_cursor = app.engine_cursor.saturating_sub(1);
            apply_selected_engine(app);
            None
        }
        Msg::EngineDown => {
            let last = app.engines.len().saturating_sub(1);
            app.engine_cursor = (app.engine_cursor + 1).min(last);
            apply_selected_engine(app);
            None
        }

        // ── URL text buffer ──────────────────────────────────────────────────
        Msg::UrlChar(c) => {
            app.base_url.push(c);
            None
        }
        Msg::UrlBackspace => {
            app.base_url.pop();
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

/// Selecting an engine fills the endpoint URL from that engine and preloads its
/// model list (so the Model dropdown is instant for a detected engine). This is
/// an explicit choice, so it always overwrites the URL buffer — a custom
/// endpoint is entered by editing the URL field, not by moving this cursor.
fn apply_selected_engine(app: &mut App) {
    let Some((base_url, models)) = app
        .current_engine()
        .map(|e| (e.base_url.clone(), e.models.clone()))
    else {
        return;
    };
    app.base_url = base_url;
    app.model_list = models;
    app.model_cursor = 0;
}

// ─────────────────────────────────────────────────────────────────────────────
// Headless unit tests (no terminal required)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_tui::model::Field;
    use grove_core::config::{GroveConfig, Mode};
    use grove_explore_core::ExploreConfig;

    fn fresh() -> App {
        App::default()
    }

    // 1. Tab navigation — forward and backward, wrapping.
    #[test]
    fn tab_navigation_forward_wraps() {
        let mut app = fresh();
        assert_eq!(app.focus, Field::Engine);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Url);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Model);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Tap);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Tools);
        update(&mut app, Msg::TabNext);
        assert_eq!(app.focus, Field::Engine); // wrapped
    }

    #[test]
    fn tab_navigation_backward_wraps() {
        let mut app = fresh();
        assert_eq!(app.focus, Field::Engine);
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

    // 2. Engine selection fills the endpoint URL and preloads models.
    #[test]
    fn engine_switch_fills_url_from_candidate() {
        let mut app = fresh();
        // Row 0 is ollama (11434), row 1 llama.cpp (8080) — see ENGINE_CANDIDATES.
        update(&mut app, Msg::EngineUp); // clamp to row 0 (ollama)
        assert_eq!(app.engine_cursor, 0);
        assert!(app.base_url.contains("11434"), "ollama URL: {}", app.base_url);

        update(&mut app, Msg::EngineDown); // → llama.cpp
        assert_eq!(app.engine_cursor, 1);
        assert!(app.base_url.contains("8080"), "llama.cpp URL: {}", app.base_url);
    }

    #[test]
    fn engine_select_preloads_models() {
        let mut app = fresh();
        // Simulate a live probe result on row 0.
        app.engines[0].alive = true;
        app.engines[0].models = vec!["qwen3.5:4b".into(), "llama3.2".into()];
        update(&mut app, Msg::EngineUp); // select row 0
        assert_eq!(app.engine_cursor, 0);
        assert_eq!(app.model_list, vec!["qwen3.5:4b".to_string(), "llama3.2".to_string()]);
        assert_eq!(app.base_url, app.engines[0].base_url);
    }

    #[test]
    fn custom_url_edit_survives_until_engine_reselected() {
        let mut app = fresh();
        // Hand-type a custom endpoint — editing the URL does not touch the engine.
        for c in "http://box:9000/v1".chars() {
            update(&mut app, Msg::UrlChar(c));
        }
        assert!(app.base_url.ends_with("9000/v1"));
        // Re-selecting an engine is an explicit choice and overwrites it.
        update(&mut app, Msg::EngineDown);
        assert_eq!(app.base_url, app.engines[app.engine_cursor].base_url);
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

    #[test]
    fn tab_and_quit_always_work() {
        let mut app = fresh();
        // TabNext always passes through.
        let focus_before = app.focus;
        let result = update(&mut app, Msg::TabNext);
        assert_eq!(result, None);
        assert_ne!(app.focus, focus_before, "TabNext must advance focus");
        // TabPrev always passes through.
        let result = update(&mut app, Msg::TabPrev);
        assert_eq!(result, None);
        // Quit always passes through.
        let result = update(&mut app, Msg::Quit);
        assert_eq!(result, Some(Action::Quit));
    }

    // 9. App::from_config() pre-populates all fields.
    #[test]
    fn from_config_pre_populates() {
        let cfg = ExploreConfig {
            provider: grove_explore_core::Provider::LlamaCpp,
            base_url: "http://localhost:8080/v1".to_string(),
            model: "llama3".to_string(),
            steering: grove_explore_core::Steering::Standard,
            allowed_tools: vec!["grove".to_string()],
            tap: true,
            trace_retain: 25,
        };
        let app = App::from_config(cfg.clone());
        // 8080 is the llama.cpp candidate → engine cursor aligns to row 1.
        assert_eq!(app.engine_cursor, 1, "llama.cpp candidate is row 1");
        assert_eq!(app.base_url, "http://localhost:8080/v1");
        assert_eq!(app.model, "llama3");
        assert_eq!(app.tools, vec![("grove".to_string(), true)]);
        assert_eq!(app.trace_retain, 25, "retention carried through unchanged");

        // Round-trip back — provider is derived from the URL (8080 → LlamaCpp),
        // steering is always persisted as the default now.
        let back = app.to_config().unwrap();
        assert_eq!(back, cfg);
    }

    // 10. AC5: config save round-trip preserves mode and harnesses from disk.
    #[test]
    fn config_round_trip_preserves_mode_and_harnesses() {
        use std::io::Write as _;

        // Create a temp dir with .grove/config.json containing mode=mcp.
        let dir = std::env::temp_dir().join(format!(
            "grove_config_roundtrip_{}", std::process::id()
        ));
        let grove_dir = dir.join(".grove");
        std::fs::create_dir_all(&grove_dir).unwrap();

        let config_json = serde_json::json!({
            "version": 1,
            "mode": "mcp",
            "harnesses": ["claude-code", "codex"],
            "explore": {
                "provider": "ollama",
                "base_url": "http://localhost:11434/v1",
                "model": "llama3",
                "steering": "standard",
                "allowed_tools": ["grove"],
                "tap": false
            }
        });
        let mut f = std::fs::File::create(grove_dir.join("config.json")).unwrap();
        write!(f, "{}", serde_json::to_string_pretty(&config_json).unwrap()).unwrap();
        drop(f);

        // Build App from that GroveConfig.
        let loaded = GroveConfig::load(&dir).expect("config.json loads");
        let app = App::from_grove_config(loaded.clone());

        // Simulate the save path: to_config() + GroveConfig::load + re-build + save.
        let explore_cfg = app.to_config().expect("valid config");
        let existing = GroveConfig::load(&dir).unwrap_or_default();
        let harnesses = existing.harnesses.clone();
        let mode = existing.mode;
        let explore_val = serde_json::to_value(explore_cfg).unwrap();
        let new_cfg = GroveConfig {
            version: 1,
            mode,
            explore: Some(explore_val),
            harnesses,
        };
        new_cfg.save(&dir).expect("save succeeds");

        // Read back and assert.
        let readback = GroveConfig::load(&dir).expect("readback");
        assert_eq!(
            readback.mode, Mode::Mcp,
            "mode must be preserved as Mcp (not forced to McpLlm)"
        );
        assert_eq!(
            readback.harnesses,
            vec![
                grove_core::harness::HarnessId::ClaudeCode,
                grove_core::harness::HarnessId::Codex,
            ],
            "harnesses must be preserved"
        );
        let explore_back: ExploreConfig = serde_json::from_value(
            readback.explore.expect("explore section present"),
        ).expect("explore section valid");
        assert_eq!(explore_back.model, "llama3", "explore config round-trips");
        assert_eq!(explore_back.allowed_tools, vec!["grove".to_string()]);

        std::fs::remove_dir_all(&dir).ok();
    }
}
