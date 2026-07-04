# PROGRESS — GROVE-S03-T05: config TUI mode badge + inert explore-section rendering

## Summary

Implemented all 7 plan items: the config TUI now reads `grove_mode` from `GroveConfig`,
shows a mode badge in the outer title, and renders the explore-section as inert/greyed
when `mode != mcp-llm`. No mode-mutation control was added. All 5 new unit tests pass.

## Changes Made

### `cli/src/config_tui/model.rs`
- Added `use grove_core::{config::GroveConfig, ExploreConfig, Provider, Steering}` and
  `use grove_core::config::Mode` imports.
- Added `pub grove_mode: Mode` and `pub explore_active: bool` fields to `App`.
- Added `App::from_grove_config(cfg: GroveConfig) -> Self` constructor that sets
  `explore_active = (cfg.mode == Mode::McpLlm)` and forwards `cfg.explore.unwrap_or_default()`
  to `from_config`.
- Updated `Default::default()` to call `from_grove_config(GroveConfig { mode: McpLlm, … })`
  so all existing headless tests remain green unmodified.
- `from_config` initialises new fields to `grove_mode: Mode::McpLlm, explore_active: true`
  as sensible defaults when called directly.

### `cli/src/config_tui/view.rs`
- Added `use grove_core::config::Mode` (review-plan Advisory 3).
- Outer block title now shows the mode badge via `match app.grove_mode { … }`.
- Layout gained a leading `Constraint::Length(1)` row [0] for the explore notice; all
  subsequent row indices shifted by +1.
- Added `render_explore_notice`: blank paragraph when `explore_active`, yellow warning
  text when inactive.
- All six explore-field render functions (`render_provider`, `render_url`, `render_model`,
  `render_mode`, `render_tap`, `render_tools`) now gate `focused` on
  `app.explore_active && app.focus == Field::X` — DIM styling applies automatically
  when inactive.
- Footer suppresses field-specific hints when `!explore_active`, showing
  `"explore settings inactive — Esc to cancel"` instead.

### `cli/src/config_tui/update.rs`
- All explore-edit `Msg` variants (`ProviderUp/Down`, `UrlChar`, `UrlBackspace`,
  `ModelChar`, `ModelBackspace`, `ModelDropdownOpen`, `ModeUp/Down`, `TapToggle`,
  `ToolsToggle`, `ToolsAddChar`, `ToolsAddConfirm`) early-return `None` when
  `!app.explore_active`.
- `Msg::Save` returns `None` and sets `app.last_error` when `!explore_active`.
- `Tab*` and `Quit` always pass through (never gated).
- Added 5 new unit tests: `badge_reflects_grove_mode`, `explore_inert_blocks_all_edits`,
  `save_blocked_when_inert`, `save_allowed_when_mcp_llm`, `tab_and_quit_always_work`.

### `cli/src/config_tui/mod.rs`
- Import changed to `use grove_core::{config::GroveConfig, ExploreConfig}` (review-plan
  Advisory 2: `Mode` not imported here as it's not used in mod.rs body).
- `run()` signature changed from `Option<ExploreConfig>` to `Option<GroveConfig>`.
- App initialisation uses `App::from_grove_config(cfg)` / `App::default()`.
- `Action::Save` handler builds `GroveConfig { mode: app.grove_mode, explore: Some(explore_cfg) }`
  and calls `cfg.save(root)` instead of `explore_cfg.save(root)`.

### `cli/src/main.rs`
- `Cmd::Config` now loads `grove_core::config::GroveConfig::load(&path).ok()` and
  passes the result to `config_tui::run`.

### `cli/src/init.rs`
- After `config_tui::run(root, None)?`, the explore-load chain now prefers
  `GroveConfig::load(root).ok().and_then(|c| c.explore)` (TUI writes here), falls back
  to legacy `ExploreConfig::load(root).ok()`, then to `old_cfg.explore`.

## Test Evidence

```
test result: ok. 92 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
Doc-tests: ok. 1 passed; 0 failed

New tests specifically:
test config_tui::update::tests::badge_reflects_grove_mode          ... ok
test config_tui::update::tests::explore_inert_blocks_all_edits     ... ok
test config_tui::update::tests::save_blocked_when_inert            ... ok
test config_tui::update::tests::save_allowed_when_mcp_llm          ... ok
test config_tui::update::tests::tab_and_quit_always_work           ... ok
```

`cargo clippy -- -D warnings` → clean (0 warnings).

## Files Changed

- `cli/src/config_tui/model.rs`
- `cli/src/config_tui/view.rs`
- `cli/src/config_tui/update.rs`
- `cli/src/config_tui/mod.rs`
- `cli/src/main.rs`
- `cli/src/init.rs`
