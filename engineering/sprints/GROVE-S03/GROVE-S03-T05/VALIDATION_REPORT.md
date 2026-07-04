# Validation Report — GROVE-S03-T05 (standalone review)

**Task:** config TUI mode badge + inert explore-section rendering  
**Validator:** grove QA Engineer  
**Date:** 2026-07-05  

---

**Verdict:** Approved

---

## Acceptance Criteria Results

### AC1 — TUI reads the active `mode` from `GroveConfig` and shows it as a header/badge

**PASS**

`view.rs` renders the outer block title as `" grove config   mode: {badge} "` with a `match app.grove_mode` covering all five `Mode` variants:
- `Mode::Mcp` → `"mcp"`
- `Mode::Skill` → `"skill"`
- `Mode::Both` → `"both"`
- `Mode::McpLlm` → `"mcp-llm ✓"`
- `Mode::Grammars` → `"grammars"`

`App::from_grove_config` sets `grove_mode` directly from `cfg.mode`. `main.rs:301` loads `GroveConfig::load(&path).ok()` and passes it into `config_tui::run`. The `badge_reflects_grove_mode` test confirms `Mode::Mcp` → `explore_active=false`, correct mode stored.

---

### AC2 — When `mode == mcp-llm`: explore section is live and editable exactly as today

**PASS**

`App::default()` routes through `App::from_grove_config(GroveConfig { mode: Mode::McpLlm, … })` which sets `explore_active = true`. All 17 pre-existing update tests (tab navigation, provider switching, model dropdown, save/quit round-trip, to_config failures, etc.) use `fresh()` which calls `App::default()` and remain green with **zero modifications**. The `save_allowed_when_mcp_llm` test confirms `Msg::Save` returns `Some(Action::Save)` under McpLlm. Total suite: **283 tests, 0 failures**.

---

### AC3 — When `mode != mcp-llm`: explore fields render inert/greyed with dormant note; not editable, not saveable

**PASS**

Multiple layers of enforcement validated:

**Notice row:** `render_explore_notice` shows `"  ⚠  Explore settings inactive — run: grove init --as mcp-llm to activate"` in `Color::Yellow` when `!app.explore_active`. Blank placeholder when active (layout stable).

**DIM rendering:** `border_style(focused)` returns `DIM` border when `focused=false`; every `render_*` function gates `focused = app.explore_active && app.focus == Field::X`, so all field borders go dim when inactive.

**Footer hint:** `render_footer` shows `"explore settings inactive — Esc to cancel"` instead of field-specific hints when `!app.explore_active`.

**Edit no-ops:** Every explore-edit `Msg` variant guards `if !app.explore_active { return None; }`: `ProviderUp/Down`, `UrlChar`, `UrlBackspace`, `ModelChar`, `ModelBackspace`, `ModelDropdownOpen`, `ModeUp/Down`, `ToolsToggle`, `ToolsAddChar`, `ToolsAddConfirm`, `TapToggle`.

**Save blocked:** `Msg::Save` sets `app.last_error` and returns `None` when `!app.explore_active`. The `save_blocked_when_inert` test (using `Mode::Skill`) confirms `None` returned and `last_error` is set. The `explore_inert_blocks_all_edits` test (using `Mode::Mcp`) confirms all 8 edit variants return `None` and leave `provider`, `base_url`, `model`, `mode`, `tap`, `tools`, `add_tool_buf` unchanged.

**Note:** `ToolsUp`, `ToolsDown`, `ToolsAddBackspace` are not gated (cursor navigation / empty-buffer backspace) — these do not mutate actual explore data and `ToolsAddConfirm` is gated so no tool can be committed when inactive. This is consistent with the AC which specifies "not editable and cannot be saved", both of which hold.

---

### AC4 — The TUI **never mutates `mode`** — no mode-selection control

**PASS**

`app.grove_mode` is set once in `App::from_grove_config` and never reassigned anywhere in `update.rs` or `mod.rs`. No `Msg` variant targets `grove_mode`. The `mod.rs` save path builds `GroveConfig { mode: app.grove_mode, … }` — round-trips the read-only value unchanged. `tab_and_quit_always_work` test uses `Mode::Both` inert app: focus advances on `TabNext`, `Quit` returns `Some(Action::Quit)`, no mode mutation occurs.

---

### AC5 — Existing `config` TUI tests pass; new coverage asserts badge and inert path

**PASS**

5 new unit tests confirmed present and green:
1. `badge_reflects_grove_mode` — `Mode::Mcp` → `explore_active=false` ✓
2. `explore_inert_blocks_all_edits` — 8 edit Msgs return `None`, state unchanged ✓
3. `save_blocked_when_inert` — `Mode::Skill`, `Msg::Save` → `None` + `last_error` set ✓
4. `save_allowed_when_mcp_llm` — `fresh()`, `Msg::Save` → `Some(Action::Save)` ✓
5. `tab_and_quit_always_work` — `Mode::Both` inert, Tab advances focus, Quit passes ✓

All 17 pre-existing update tests pass unmodified. Full config_tui test run: **22 passed, 0 failed**.

---

### AC6 — `cargo build` warning-clean; `cargo clippy -- -D warnings` clean; `cargo test` green; files end with newline

**PASS**

- `cargo test --release --locked`: **283 passed, 0 failed** (161 + 92 + 29 + 1 across all crates/integration suites)
- `cargo clippy -- -D warnings`: **clean** — "Finished dev profile" with no warnings emitted
- All 6 modified files verified to end with `\n` (LF): `model.rs`, `view.rs`, `update.rs`, `mod.rs`, `main.rs`, `init.rs` ✓

---

## Summary

All 6 acceptance criteria are satisfied. The implementation is complete, correct, and regression-free. The badge covers all 5 Mode variants; the inert path blocks edits and saves via multiple defensive layers; no mode mutation path exists; the test suite is both backward-compatible and forward-covered.
