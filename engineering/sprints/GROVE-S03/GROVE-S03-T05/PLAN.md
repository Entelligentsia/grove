# PLAN — GROVE-S03-T05: `config` TUI mode badge + inert explore-section rendering

## Objective

Extend the `grove config` TUI so it reads `mode` from `GroveConfig` (introduced
by T01), displays the active mode as a visible badge, and renders explore-section
fields as inert/greyed when `mode != mcp-llm`. No mode-mutation control is added;
`grove init --as` remains the single harness writer.

---

## Background

After T01, `.grove/config.json` is the canonical config file with schema:
```json
{ "version": 1, "mode": "<mode>", "explore": { ... } }
```
The config TUI today loads/saves only `ExploreConfig` (`.grove/explore.json`).
This task upgrades it to load a full `GroveConfig` and to gate explore-section
editing on `mode == mcp-llm`.

---

## Approach

Elm-style layered change: Model → Update → View → wiring (mod.rs, main.rs, init.rs).

1. **Model** — add `grove_mode: Mode` and `explore_active: bool` to `App`; add
   `App::from_grove_config(cfg: GroveConfig)` constructor.
2. **Update** — gate all explore-field edits and `Save` on `app.explore_active`.
3. **View** — add mode badge to the outer block title; insert a 1-line notice row
   at the top of the inner layout; suppress focus/cursor in all explore fields
   when `!explore_active`.
4. **mod.rs** — change `run` signature to `Option<GroveConfig>`; build a
   `GroveConfig` on save instead of `ExploreConfig`.
5. **main.rs** — load `GroveConfig::load` instead of `ExploreConfig::load`.
6. **init.rs** — read the explore section from `GroveConfig` (not `explore.json`)
   after the TUI returns.

The change is material (visible TUI change).

---

## Files to Modify

| File | Change |
|------|--------|
| `cli/src/config_tui/model.rs` | Add `grove_mode`, `explore_active` fields; add `from_grove_config`; update `Default` |
| `cli/src/config_tui/view.rs` | Mode badge in outer title; notice row in layout; DIM rendering when inert |
| `cli/src/config_tui/update.rs` | Gate edits and Save on `explore_active`; new unit tests |
| `cli/src/config_tui/mod.rs` | Change `run` signature; save via `GroveConfig`; import `GroveConfig`, `Mode` |
| `cli/src/main.rs` | Load `GroveConfig` instead of `ExploreConfig` for `Cmd::Config` |
| `cli/src/init.rs` | After TUI returns, prefer `GroveConfig::load` over `ExploreConfig::load` |

---

## Data Model Changes

### `App` struct (model.rs)

```rust
pub struct App {
    /// Integration mode read from GroveConfig — determines explore_active.
    pub grove_mode: Mode,
    /// True only when grove_mode == Mode::McpLlm. Explore fields are live.
    pub explore_active: bool,
    // ... existing fields unchanged ...
}
```

### New constructor

```rust
/// Populate TUI state from a GroveConfig.
pub fn from_grove_config(cfg: GroveConfig) -> Self {
    let explore = cfg.explore.unwrap_or_default();
    let mut app = App::from_config(explore);      // reuse existing mapping
    app.grove_mode = cfg.mode;
    app.explore_active = matches!(cfg.mode, Mode::McpLlm);
    app
}
```

### `Default` alignment

`App::default()` sets `grove_mode = Mode::McpLlm`, `explore_active = true` — so
all existing headless tests continue to exercise the explore-active path without
any changes to those tests.

### `to_config` unchanged

`App::to_config() -> anyhow::Result<ExploreConfig>` stays as-is; the caller
(`mod.rs`) wraps the result into a `GroveConfig` before saving.

---

## Detailed Changes

### model.rs

- Import `GroveConfig, Mode` from `grove_core`.
- Add two fields: `pub grove_mode: Mode` and `pub explore_active: bool`.
- Add `App::from_grove_config(cfg: GroveConfig) -> Self` as described above.
- Update `Default::default()` to call `from_grove_config` with a
  `GroveConfig { version: 1, mode: Mode::McpLlm, explore: None }` then
  `reset_dirty()`.

### view.rs

**Outer block title** changes to include the mode badge:

```rust
let title = format!(
    " grove config   mode: {} ",
    match app.grove_mode {
        Mode::Mcp      => "mcp",
        Mode::Skill    => "skill",
        Mode::Both     => "both",
        Mode::McpLlm   => "mcp-llm ✓",
        Mode::Grammars => "grammars",
    }
);
let outer = Block::default().borders(Borders::ALL).title(title);
```

**Layout** gains one leading row (1 line) for the explore notice:

```rust
let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(1), // [0] explore notice (blank when active)
        Constraint::Length(3), // [1] Provider
        Constraint::Length(3), // [2] Endpoint URL
        Constraint::Length(3), // [3] Model
        Constraint::Length(5), // [4] Steering mode
        Constraint::Length(3), // [5] Tap
        Constraint::Min(4),    // [6] Allowed Tools
        Constraint::Length(1), // [7] Status bar
        Constraint::Length(1), // [8] Footer shortcuts
    ])
    .split(inner);
```

`render_explore_notice(app, frame, rows[0])`:
- When `explore_active`: renders an empty paragraph (invisible placeholder).
- When `!explore_active`: renders a one-line message in `Color::Yellow`:
  `"  ⚠  Explore settings inactive — run: grove init --as mcp-llm to activate"`

**All explore-field render functions** (`render_provider`, `render_url`,
`render_model`, `render_mode`, `render_tap`, `render_tools`) check
`app.explore_active && app.focus == Field::X` for `focused` rather than just
`app.focus == Field::X`. When inactive, `focused = false`, so the existing DIM
border style and DIM text colour apply automatically — no additional code needed.

**Footer** changes to suppress field-specific hints when `!explore_active`:

```rust
let field_keys = if !app.explore_active {
    "explore settings inactive — Esc to cancel"
} else {
    match app.focus { /* existing arms */ }
};
```

### update.rs

The `update` function gates all explore-edit messages on `app.explore_active`.
Any edit message (`ProviderUp/Down`, `UrlChar`, `ModelChar`, `ModeUp/Down`,
`TapToggle`, `ToolsToggle`, `ToolsAddChar`, etc.) is a no-op when `!explore_active`.
`Msg::Save` also returns `None` when `!explore_active` (and sets `app.last_error`
to a "settings inactive" string as feedback in the status bar).

Tab-navigation (`TabNext`, `TabPrev`) and `Quit` are never gated — users can
always navigate and exit.

Pattern:
```rust
Msg::ProviderUp | Msg::ProviderDown | Msg::UrlChar(_) | ... => {
    if !app.explore_active { return None; }
    // existing handler
}
Msg::Save => {
    if !app.explore_active {
        app.last_error = Some(
            "explore settings inactive — run: grove init --as mcp-llm".to_string()
        );
        return None;
    }
    Some(Action::Save)
}
```

### mod.rs

Change signature:
```rust
use grove_core::{ExploreConfig, GroveConfig, Mode};

pub fn run(root: &Path, grove_cfg: Option<GroveConfig>) -> Result<()> {
    // ...
    let mut app = match grove_cfg {
        Some(cfg) => App::from_grove_config(cfg),
        None => App::default(), // McpLlm-active default; used by init first-run
    };
```

Save action in event loop:
```rust
Some(Action::Save) => match app.to_config() {
    Ok(explore_cfg) => {
        let cfg = GroveConfig {
            version: 1,
            mode: app.grove_mode,
            explore: Some(explore_cfg),
        };
        cfg.save(root)?;
        return Ok(());
    }
    Err(e) => { app.last_error = Some(e.to_string()); }
},
```

Remove the now-unused `ExploreConfig` import (it was the previous `run` parameter
type); `ExploreConfig` is still used indirectly through `App`/`model.rs`.

### main.rs

```rust
Cmd::Config { path } => {
    let grove_cfg = grove_core::GroveConfig::load(&path).ok();
    config_tui::run(&path, grove_cfg)?;
}
```

### init.rs

After `crate::config_tui::run(root, None)?;`, change the explore-load to prefer
`GroveConfig` (which the TUI now writes) with a fallback chain for the legacy path:

```rust
let explore = if target == Target::McpLlm {
    // Prefer config.json (TUI writes here); fall back to legacy explore.json,
    // then to the explore section of the pre-run config.
    GroveConfig::load(root).ok().and_then(|c| c.explore)
        .or_else(|| ExploreConfig::load(root).ok())
        .or_else(|| old_cfg.as_ref().and_then(|c| c.explore.clone()))
} else {
    old_cfg.as_ref().and_then(|c| c.explore.clone())
};
```

---

## Testing Strategy

### Existing tests — no changes required

All tests in `update.rs` use `fresh()` → `App::default()` which has
`grove_mode = McpLlm`, `explore_active = true`. Every existing test exercises the
fully-active path and must continue to pass unmodified.

### New unit tests in `update.rs`

1. **`badge_reflects_grove_mode`** — `App::from_grove_config(GroveConfig { mode: Mode::Mcp, … })`
   yields `app.explore_active == false` and `app.grove_mode == Mode::Mcp`.

2. **`explore_inert_blocks_all_edits`** — verify that `ProviderUp`, `UrlChar('x')`,
   `ModelChar('x')`, `ModeDown`, `TapToggle`, `ToolsToggle`, `ToolsAddChar('x')`
   all return `None` and leave state unchanged when `explore_active = false`.

3. **`save_blocked_when_inert`** — `Msg::Save` returns `None` and sets
   `app.last_error` to a non-empty string when `!explore_active`.

4. **`save_allowed_when_mcp_llm`** — `Msg::Save` returns `Some(Action::Save)` when
   `explore_active = true` (default App).

5. **`tab_and_quit_always_work`** — `TabNext`, `TabPrev`, `Quit` return their
   expected values regardless of `explore_active`.

---

## Acceptance Criteria (mapping)

| AC | Implementation |
|----|---------------|
| TUI reads mode, shows badge | outer block title; `App::grove_mode` from `GroveConfig::load` |
| mcp-llm: explore live | `explore_active = true`; all existing behaviour unchanged |
| non-mcp-llm: fields inert/greyed + note | `explore_active = false`; notice row; DIM rendering; edit no-ops |
| TUI never mutates mode | no `Mode` selector added; `grove_mode` is read-only in `App` |
| Existing tests pass | `App::default()` stays `McpLlm`-active; no test changes needed |
| New coverage asserts badge + inert path | 5 new unit tests in `update.rs` |
| Build + clippy + test clean | verified at implementation time |

---

## Operational Impact

- **Version bump:** required at release (visible TUI change — badge, inert path).
- **Regeneration:** none.
- **Security scan:** not required.
- **Dependency change:** none; `grove_core::GroveConfig` and `grove_core::Mode`
  already re-exported from T01.
