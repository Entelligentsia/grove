# PLAN — GROVE-S02-T05: Full-screen setup TUI + `grove config` verb

## Objective

Add a full-screen ratatui TUI that collects and persists mcp-llm configuration
(provider, endpoint URL, model, mode, allowed bash tools) via the T01
`ExploreConfig` model, wired to a new `grove config` CLI subcommand that
re-opens the TUI pre-populated from the existing `.grove/explore.json`.

---

## Approach

### Architecture: Elm-style Model-Update-View

The TUI is implemented as three clean layers:

1. **`model.rs`** — `App` struct: all form state (field values, cursor/focus,
   intent). Pure data, no terminal dependency. `App::from_config(cfg)` for
   pre-populated entry; `App::default()` for first-run defaults. A `Field` enum
   drives the tab-stop sequence.

2. **`update.rs`** — `update(app: &mut App, key: KeyCode, modifiers) ->
   Option<Action>` — a pure function (no I/O, no terminal). Returns
   `Some(Action::Save)` or `Some(Action::Quit)` to signal the runner to
   terminate. All logic here is headlessly unit-testable.

3. **`view.rs`** — `view(app: &App, frame: &mut Frame)` — renders to the
   ratatui `Frame`. Not tested headlessly (renderer depends on terminal
   backend), but kept thin (purely display logic, no state mutation).

4. **`mod.rs`** — `run(root: &Path) -> Result<()>`: the event loop. Detects
   non-TTY (via `std::io::IsTerminal::is_terminal(&std::io::stdout())`), enters
   raw mode, ticks the `update`/`view` loop, and on `Action::Save` calls
   `ExploreConfig::save(root)`.

The `config_tui` module lives entirely in `cli/src/`; `core` remains TUI-free.

### Non-TTY Detection

Before entering raw mode, `run()` checks `std::io::IsTerminal` on both stdin
and stdout. If either is non-TTY, it returns `Err(...)` with a descriptive
message: _"grove config requires an interactive terminal — stdin/stdout is not a
TTY."_

### Fields and Widgets

| Field            | Widget                   | Key Interactions                         |
|------------------|--------------------------|------------------------------------------|
| Provider         | `List` (2 items)         | ↑/↓ or j/k to select; Tab to advance     |
| Endpoint URL     | Inline buffer (manual)   | Char insert; Backspace; Tab to advance   |
| Model            | Inline buffer (manual)   | Char insert; Backspace; Tab to advance   |
| Mode             | `List` (3 items + desc)  | ↑/↓ or j/k; Tab to advance              |
| Allowed Tools    | Checkbox list            | ↑/↓ navigate; Space to toggle; 'a' to add new entry; Tab to advance |

**Global keys:** `s` or `F2` → Save; `Esc` or `q` → Cancel (leaves config
untouched). A bottom status bar shows current keybindings.

### Provider → URL Default

Switching the `Provider` selection **auto-fills** `base_url` with its default
*only when the URL field is still holding the other provider's default* (i.e.,
the user has not manually edited it). This avoids clobbering a custom URL.

### Save / Cancel Semantics

- **Save** (`s` / F2): calls `ExploreConfig::save(root)` with the TUI-assembled
  config. Atomic write per T01 contract.
- **Cancel** (`Esc` / `q`): exits without calling `save`; existing
  `.grove/explore.json` is not touched. If no config existed, none is created.

### Nice-to-Have: Model Auto-Discovery (AC #5)

Out of scope for this plan. The `update.rs` `Msg` type will include a
`ModelListFetched(Vec<String>)` variant (dead code in this task) so the
extension point exists without requiring a refactor; it is gated behind
`#[allow(dead_code)]` and will not ship in the binary.

---

## Files to Modify

| File | Action | Notes |
|------|--------|-------|
| `cli/Cargo.toml` | Edit | Add `ratatui = "0.29"` and `crossterm = "0.28"` under `[dependencies]` |
| `cli/src/main.rs` | Edit | Add `mod config_tui;`; add `Cmd::Config` variant; handle in `main()` |
| `cli/src/config_tui/mod.rs` | New | Event loop, `run()` entry point, non-TTY guard |
| `cli/src/config_tui/model.rs` | New | `App`, `Field`, `Action`, `Msg` — pure data |
| `cli/src/config_tui/update.rs` | New | `update()` — pure state transition function |
| `cli/src/config_tui/view.rs` | New | `view()` — ratatui rendering |

---

## Data Model Changes

No changes to `ExploreConfig` or any store schema. The TUI consumes the
existing T01 types directly:

```
ExploreConfig { provider: Provider, base_url: String, model: String,
                mode: Mode, allowed_tools: Vec<String> }
```

The `App` struct mirrors these fields with editing affordances:

```
App {
  provider: usize,          // index into Provider::LEGAL
  base_url: String,
  model: String,
  mode: usize,              // index into Mode::LEGAL
  tools: Vec<(String, bool)>, // (name, selected)
  tool_cursor: usize,
  add_tool_buf: String,     // pending new-tool text
  focus: Field,
  dirty_url: bool,          // user has manually edited base_url
}
```

`App::to_config() -> Result<ExploreConfig>` converts the TUI state back to a
validated `ExploreConfig`, delegating to `ExploreConfig::validate()`.

---

## Testing Strategy

### Headless Unit Tests (in `config_tui/update.rs`)

All via `update()`:

1. **Tab navigation** — `Field::Provider` → Tab → `Field::Url` → ... →
   `Field::Tools` → Tab → wraps to `Field::Provider`.
2. **Provider selection drives URL default** — selecting LlamaCpp sets
   `base_url` to `http://localhost:8080/v1` when URL is at Ollama default;
   switching back restores Ollama default; custom-edited URL is not clobbered.
3. **Mode cycling** — arrow keys cycle `Mode` enum values.
4. **Tool toggle** — Space on a seeded tool flips its checked state.
5. **Save action** — pressing `s` returns `Some(Action::Save)`.
6. **Cancel action** — pressing `Esc` returns `Some(Action::Quit)` without
   mutating state.
7. **`App::to_config()` round-trip** — default `App` maps to the same config as
   `ExploreConfig::default()`; a mutated `App` maps to the expected config.
8. **Empty required field** — `to_config()` propagates `ExploreConfig::validate()`
   failure for blank model or URL.
9. **`App::from_config()` pre-populates** — all fields reflect the loaded config.

### Regression: Existing Tests

`cargo test --release --locked` must remain green. The existing `cli/tests/cli.rs`
smoke tests do not exercise `config_tui` (they run in non-TTY; the command
would fast-fail). A dedicated test verifies the non-TTY error path:

```
grove config  →  exit non-zero, stderr contains "interactive terminal"
```

(Run via `Command::new(GROVE_BIN).arg("config")` with no TTY — i.e., the
default subprocess context in the test harness already satisfies this.)

---

## Acceptance Criteria Mapping

| AC | Coverage |
|----|----------|
| 1. Full-screen TUI collecting all 5 fields | `config_tui/view.rs` renders; `config_tui/update.rs` handles |
| 2. `grove config` verb pre-populated from `explore.json` | `Cmd::Config` in `main.rs`; `App::from_config()` |
| 3. Save/cancel semantics | `Action::Save` / `Action::Quit`; `mod.rs` event loop |
| 4. Non-TTY fast fail | `std::io::IsTerminal` guard in `config_tui::run()` |
| 5. Model auto-discovery | Out of scope; extension point stubbed with dead-code variant |
| 6. Binary size delta measured | Post-build measurement step in implementation |
| 7. Warning-clean, clippy-clean, tests green | CI gates + `#[allow(dead_code)]` for stub |

---

## Operational Impact

- **New verb:** `grove config` — no existing surface is altered.
- **TUI dependency scope:** `ratatui` + `crossterm` are in `cli/Cargo.toml` only;
  `core` remains TUI-free (verifiable via `cargo tree -p grove-cst`).
- **Binary size:** ratatui + crossterm typically adds 1–2 MB to a stripped
  release binary. The exact delta (baseline vs. post-change `ls -lh`) must be
  measured and recorded in the PR description per AC #6.
- **Non-breaking:** `grove serve`, `grove init`, and all structural commands are
  unaffected. Existing `.grove/explore.json` files written by T01 are fully
  compatible.
- **Version bump:** not required for this task (sprint-final bump deferred).
