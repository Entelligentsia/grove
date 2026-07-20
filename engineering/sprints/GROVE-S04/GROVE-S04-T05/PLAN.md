# GROVE-S04-T05 Plan: Move config/trace TUIs + tap to grove-explore; forwarding shims + string sweep

> **Revision note:** This is a revised plan addressing the two blocking issues raised in PLAN_REVIEW:
> (1) `cli/src/init.rs` calls `crate::config_tui::run()` which will not compile after the module moves — plan now adds `cli/src/init.rs` to Files to Modify and specifies the exec shim replacement.
> (2) Step 4 previously dropped `grove_mode` from `App` and the round-trip test asserted `mode == McpLlm` unconditionally — this contradicts AC5. Fixed: `grove_mode` is dropped from `App` but mode is re-read from disk in the save path (mirroring how `harnesses` is already preserved), and the test asserts the incoming mode is preserved.

## Objective

Move the two pure explore-product UIs — `cli/src/config_tui/` and `cli/src/trace_tui/` and `cli/src/tap.rs` (~2,950 lines) — wholesale to `grove-explore` as `grove-explore config` and `grove-explore tap`. Leave one-release forwarding shims at the old `grove` spellings. Fix `cli/src/init.rs`'s first-run TUI call to exec `grove-explore config` instead. Sweep user-facing strings that still reference the single-server story. Remove the mode-gated inert rendering that is now moot.

## Why a New Workspace Crate

`grove-explore` already hosts the MCP serve loop (T02). Adding `config` and `tap` subcommands makes it the single binary for all explore-surface interactions, so `ratatui`/`crossterm` are fully absent from the `grove` CLI dep tree (AC6).

## Approach

### Step 1 — Create new workspace member `grove-explore/`

**`grove-explore/Cargo.toml`** (new):
```toml
[package]
name = "grove-explore-bin"
version = "0.4.1"
edition = "2021"
description = "grove-explore MCP server + TUI verbs (config, tap)"
license = "MIT"
repository = "https://github.com/Entelligentsia/grove"

[[bin]]
name = "grove-explore"
path = "src/main.rs"

[dependencies]
grove-cst          = { path = "../core",    version = "=0.4.1" }
grove-explore-core = { path = "../explore", version = "=0.4.1" }
clap               = { version = "4", features = ["derive"] }
serde              = { version = "1", features = ["derive"] }
serde_json         = "1"
anyhow             = "1"
ratatui            = "0.29"
crossterm          = "0.28"
```

`ratatui` and `crossterm` live **only** here — removed from `cli/Cargo.toml`.

### Step 2 — Define subcommand structure in `grove-explore/src/main.rs`

The new `main.rs` restructures the existing `cli/src/bin/grove_explore.rs` MCP serve loop into an explicit `Serve` subcommand, adding `Config` and `Tap` as new verbs. No backward-compat positional-path mode is preserved; tests that called `grove-explore /path` are updated to `grove-explore serve /path`. (`.mcp.json` registrations call `grove-explore` with no args → defaults to `serve` from CWD.)

```
grove-explore serve [PATH]              — MCP server (default when no subcommand; PATH defaults to ".")
grove-explore config [PATH]             — full-screen config TUI
grove-explore tap [PATH] [--no-enable]  — enable tracing + trace browser
```

`main.rs` moves the full MCP loop inline, keeping the startup health gate identical to T02.

### Step 3 — Move TUI modules

Move verbatim, then apply targeted edits:

| Source path | Destination |
|-------------|-------------|
| `cli/src/config_tui/mod.rs` | `grove-explore/src/config_tui/mod.rs` |
| `cli/src/config_tui/model.rs` | `grove-explore/src/config_tui/model.rs` |
| `cli/src/config_tui/update.rs` | `grove-explore/src/config_tui/update.rs` |
| `cli/src/config_tui/view.rs` | `grove-explore/src/config_tui/view.rs` |
| `cli/src/trace_tui/mod.rs` | `grove-explore/src/trace_tui/mod.rs` |
| `cli/src/trace_tui/model.rs` | `grove-explore/src/trace_tui/model.rs` |
| `cli/src/trace_tui/update.rs` | `grove-explore/src/trace_tui/update.rs` |
| `cli/src/trace_tui/view.rs` | `grove-explore/src/trace_tui/view.rs` |
| `cli/src/tap.rs` | `grove-explore/src/tap.rs` |

All `crate::config_tui` / `crate::trace_tui` references within the moved files are unchanged (already relative to the crate root).

### Step 4 — Mode-badge and inert-rendering removal (AC4 must-have)

**`grove-explore/src/config_tui/model.rs`** — remove:
- Field `grove_mode: Mode` from `App`
- Field `explore_active: bool` from `App`
- The `Mode` import from `grove_core::config`
- The `from_grove_config` path that reads `cfg.mode` and derives `explore_active`; always mark fields as active (the explore config is always live in `grove-explore`)
- The `from_grove_config` method becomes a thin wrapper over `from_config`: read the explore section from the GroveConfig, then call `from_config(explore_cfg)` — no mode logic

**`grove-explore/src/config_tui/view.rs`** — remove:
- The mode-badge title: `format!(" grove config   mode: {} ", ...)` → static string `" grove config "`
- `render_explore_notice` row and its `Constraint::Length(1)` slot; shift layout row indices accordingly
- All `if app.explore_active` guards; all fields are always focusable
- Footer branch `if !app.explore_active { "explore settings inactive — Esc to cancel" }`; always show the field-specific key hint
- The `Mode` import

**`grove-explore/src/config_tui/update.rs`** — remove:
- Any `explore_active` guard on `Msg::Save` (always allowed; `save_blocked_when_inert` test is deleted)

**`grove-explore/src/config_tui/update.rs` tests** — delete:
- `badge_reflects_grove_mode`
- `explore_inert_blocks_all_edits`
- `save_blocked_when_inert`
- `save_allowed_when_mcp_llm` (trivially true after removal; delete)

**`grove-explore/src/config_tui/mod.rs`** — `run(root, grove_cfg)` public signature unchanged. The entry-point still populates `App` from the existing `GroveConfig` on disk; it just no longer reads `mode` into `App` state.

### Step 5 — String sweep in `tap.rs` + behavior update

The deprecated `.grove/explore.json` path is replaced with `GroveConfig` (`.grove/config.json`) as the tap flag's home.

**Behavior change** — `grove-explore/src/tap.rs`:
- Replace `ExploreConfig::load(root)` + `ExploreConfig::save(root)` with a GroveConfig read-modify-write:
  1. `GroveConfig::load(root)` → `grove_cfg`
  2. Deserialize `grove_cfg.explore` as `ExploreConfig`, set `tap = true`, re-serialize back to `grove_cfg.explore`
  3. `grove_cfg.save(root)` → writes `.grove/config.json`
- If `GroveConfig::load` fails (no config.json) → warn message, proceed to browser

**String changes in `grove-explore/src/tap.rs`**:

| Old string | New string |
|------------|------------|
| `"enabling tap in .grove/explore.json"` | `"enabling tap in .grove/config.json"` |
| `"grove tap: tracing enabled in .grove/explore.json — restart \`grove serve\` for it to take effect"` | `"grove-explore tap: tracing enabled — restart the \`grove-explore\` server for it to take effect"` |
| `"grove tap: no .grove/explore.json yet — run \`grove init --as mcp-llm\` or \`grove config\` to set up explore mode; showing any existing traces"` | `"grove-explore tap: no explore config yet — run \`grove init --as mcp-llm\` or \`grove-explore config\` to set up; showing any existing traces"` |

**Additional string sweeps** (doc comments + user strings):

| File | Old | New |
|------|-----|-----|
| `grove-explore/src/tap.rs` (module doc) | "`grove serve --explore` records every session" | "`grove-explore` records every session" |
| `grove-explore/src/trace_tui/mod.rs` (module doc) | "a live `grove serve` session streams in" | "a live `grove-explore` session streams in" |
| `grove-explore/src/config_tui/view.rs` | `"☑ on — recording sessions to .grove/traces/  (browse: grove tap)"` | `"☑ on — recording sessions to .grove/traces/  (browse: grove-explore tap)"` |

### Step 6 — Config round-trip test for GroveConfig ownership (AC5) — REVISED

**Correction from review:** the original Step 6 asserted `mode == McpLlm` unconditionally, contradicting AC5. The fixed approach mirrors how `harnesses` are already preserved: in the save path in `grove-explore/src/config_tui/mod.rs`, re-read `mode` from `GroveConfig::load(root)` before writing (same pattern already used for `harnesses`):

```rust
Some(Action::Save) => match app.to_config() {
    Ok(explore_cfg) => {
        let existing = GroveConfig::load(root).unwrap_or_default();
        let harnesses = existing.harnesses;        // already present
        let mode = existing.mode;                  // NEW: preserve mode from disk
        let explore_val = serde_json::to_value(explore_cfg)
            .expect("ExploreConfig is always serializable");
        let cfg = GroveConfig {
            version: 1,
            mode,          // preserved from on-disk config (not from App state)
            explore: Some(explore_val),
            harnesses,
        };
        cfg.save(root)?;
        return Ok(());
    }
    ...
```

**Unit test** — add `config_round_trip_preserves_mode_and_harnesses` in `grove-explore/src/config_tui/update.rs` (or a `tests` module in `mod.rs`):
```
config_round_trip_preserves_mode_and_harnesses
  — create a temp dir; write .grove/config.json with:
      { "version": 1, "mode": "mcp", "harnesses": ["claude-code", "codex"],
        "explore": { "provider": "ollama", "base_url": "...", ... } }
  — build App from that GroveConfig (via from_grove_config)
  — simulate the save path: call to_config() → ExploreConfig,
    then GroveConfig::load(root) to get mode + harnesses,
    then GroveConfig { version:1, mode, explore:..., harnesses }.save(root)
  — read back .grove/config.json
  — assert GroveConfig.mode == Mcp   (preserved — NOT forced to McpLlm)
  — assert GroveConfig.harnesses == ["claude-code", "codex"]  (preserved)
  — assert GroveConfig.explore is Some and round-trips back to valid ExploreConfig
```

### Step 7 — Modify `cli/Cargo.toml`

- Remove `ratatui = "0.29"` and `crossterm = "0.28"` from `[dependencies]`
- Remove the `[[bin]]` block for `grove-explore` (binary moves to its own crate)

### Step 8 — Modify `cli/src/main.rs`

- Remove `mod config_tui;`, `mod trace_tui;`, `mod tap;`
- Update `Cmd::Config` and `Cmd::Tap` variant doc/help text to name `grove-explore` as canonical
- Replace `Cmd::Config { path }` handler with a forwarding shim:
  ```rust
  Cmd::Config { path } => {
      eprintln!("note: `grove config` is deprecated; use `grove-explore config` instead");
      let status = std::process::Command::new("grove-explore")
          .arg("config")
          .arg(&path)
          .status();
      match status {
          Ok(s) => std::process::exit(s.code().unwrap_or(if s.success() { 0 } else { 1 })),
          Err(_) => {
              eprintln!("`grove-explore` not found — install it or use `grove-explore config` directly");
              std::process::exit(1);
          }
      }
  }
  ```
- Replace `Cmd::Tap { path, no_enable }` handler with a forwarding shim (analogous):
  ```rust
  Cmd::Tap { path, no_enable } => {
      eprintln!("note: `grove tap` is deprecated; use `grove-explore tap` instead");
      let mut cmd = std::process::Command::new("grove-explore");
      cmd.arg("tap").arg(&path);
      if no_enable { cmd.arg("--no-enable"); }
      let status = cmd.status();
      match status {
          Ok(s) => std::process::exit(s.code().unwrap_or(if s.success() { 0 } else { 1 })),
          Err(_) => {
              eprintln!("`grove-explore` not found — install it or use `grove-explore tap` directly");
              std::process::exit(1);
          }
      }
  }
  ```
- Keep `Cmd::Config` and `Cmd::Tap` in the `Cmd` enum (shims for one release)

### Step 8a — Modify `cli/src/init.rs` — NEW (fixes blocker 1)

**Problem:** `init.rs:136` calls `crate::config_tui::run(root, None)?` for the first-run TUI. After Step 3 moves `config_tui` out of the `cli` crate, this reference does not compile.

**Fix:** Replace the `crate::config_tui::run()` call with an exec of `grove-explore config <root>` using the existing `find_explore_binary()` helper (defined at `init.rs:597`). Also fix the stale comment on line ~134 which says the TUI "writes `.grove/explore.json`" when it actually writes `config.json`.

```rust
// BEFORE (init.rs:~133-136):
// First-run TUI: launch the config TUI to create .grove/explore.json when
// it doesn't exist yet. Skipped on re-runs (file already there).
if target == Target::McpLlm && !root.join(".grove").join("explore.json").exists() {
    crate::config_tui::run(root, None)?;
}

// AFTER:
// First-run TUI: exec grove-explore config to set up .grove/config.json when
// the explore section is not yet configured. Skipped on re-runs.
if target == Target::McpLlm && !root.join(".grove").join("explore.json").exists() {
    let bin = find_explore_binary()
        .context("locating grove-explore for first-run config setup")?;
    let status = std::process::Command::new(&bin)
        .arg("config")
        .arg(root)
        .status()
        .with_context(|| format!("launching {} config", bin.display()))?;
    if !status.success() {
        anyhow::bail!("grove-explore config exited with status {:?}", status.code());
    }
}
```

**Test:** Update the existing `mcp_llm_steering_block_idempotency` test (which pre-seeds `.grove/explore.json` to bypass the TUI guard) so it continues to bypass the guard correctly. Add a new integration test `init_first_run_grove_explore_absent` in `cli/tests/cli.rs`:
- Run `grove init --as mcp-llm` in a temp dir where `grove-explore` cannot be found (e.g. PATH is empty, or the binary is absent from the sibling-to-grove location)
- Assert exit is non-zero and stderr contains "grove-explore" (the binary-not-found error)
- This tests the error path without needing a real interactive TTY

### Step 9 — Update root `Cargo.toml`

Add `"grove-explore"` to the workspace `members` array.

### Step 10 — Update tests in `cli/tests/cli.rs`

**Two test sites use `CARGO_BIN_EXE_grove-explore`** (lines 1390 and 1510). Since `grove-explore` is no longer a `[[bin]]` in the `cli` package, this compile-time macro fails. Replace with:
```rust
fn grove_explore_bin() -> std::path::PathBuf {
    let grove = std::path::PathBuf::from(env!("CARGO_BIN_EXE_grove"));
    grove.parent().unwrap().join("grove-explore")
}
```

Update the two invocation sites to use `grove_explore_bin()`. Update `.arg(dir.to_str().unwrap())` to `.arg("serve").arg(dir.to_str().unwrap())` for the new explicit `serve` subcommand.

**`tap_enables_tracing_in_config`** and **`tap_no_enable_leaves_config_untouched`** — update to seed `.grove/config.json` (with `version`/`mode`/`explore` section) instead of `.grove/explore.json`, and read back assertions from `config.json` after the tap command runs. Check `config.json`'s `explore.tap` field.

**`config_in_non_tty_fails_fast`** — update to invoke `grove-explore config` (via `grove_explore_bin()`) directly; the shim is tested separately. Keep this test focused on the `grove-explore config` non-TTY guard.

**New tests:**

| Test | What it verifies |
|------|-----------------|
| `grove_config_shim_prints_new_spelling` | `grove config` (non-TTY) stderr contains `"grove-explore config"` — the deprecation line |
| `grove_tap_shim_prints_new_spelling` | `grove tap` (non-TTY) stderr contains `"grove-explore tap"` — the deprecation line |
| `grove_explore_config_non_tty_fails_fast` | `grove-explore config` exits non-zero and stderr contains `"interactive terminal"` |
| `grove_explore_tap_enables_tracing_in_config_json` | `grove-explore tap` writes `explore.tap = true` to `.grove/config.json` |
| `init_first_run_grove_explore_absent` | `grove init --as mcp-llm` fails gracefully when `grove-explore` binary is absent |

**Note on advisory 3:** The shim tests (`grove_config_shim_prints_new_spelling` and `grove_tap_shim_prints_new_spelling`) must assert on the substring `"grove-explore config"` / `"grove-explore tap"` (present in the unconditional `eprintln!("note: ... deprecated; use grove-explore config instead")` line), so both the exec-success and grove-explore-absent paths satisfy AC2 "either way."

**Note on advisory 5 (scope):** `mcp_llm_steering_block_idempotency` and the `bug1_serve_mcp_mode_ignores_stale_explore_json` / init dry-run tests intentionally seed `.grove/explore.json` to guard the first-run path. Leave those seeds in place — they test that stale `explore.json` presence bypasses the TUI prompt, which remains correct. The string sweep in Step 5 must not reach into these init tests.

---

## Files to Modify

| File | Change |
|------|--------|
| `Cargo.toml` | Add `"grove-explore"` to workspace `members` |
| `cli/Cargo.toml` | Remove ratatui + crossterm deps; remove `[[bin]] grove-explore` block |
| `cli/src/main.rs` | Remove 3 `mod` decls; replace Config/Tap match arms with exec shims; update help text |
| `cli/src/init.rs` | Replace `crate::config_tui::run(root, None)?` with `find_explore_binary()` exec of `grove-explore config <root>`; fix stale comment (explore.json → config.json) |
| `cli/tests/cli.rs` | Replace `CARGO_BIN_EXE_grove-explore`; update tap tests to seed/assert config.json; update `config_in_non_tty_fails_fast`; add 5 new tests |

## Files to Create

| File | Description |
|------|-------------|
| `grove-explore/Cargo.toml` | New workspace crate manifest (Step 1) |
| `grove-explore/src/main.rs` | Subcommand dispatcher: `serve` / `config` / `tap` (Step 2) |
| `grove-explore/src/config_tui/mod.rs` | Moved + patched: save path re-reads mode from disk (Step 3 + 6) |
| `grove-explore/src/config_tui/model.rs` | Moved + AC4 edits: drop `grove_mode`/`explore_active` fields (Step 3 + 4) |
| `grove-explore/src/config_tui/update.rs` | Moved + AC4 edits: delete inert-guard tests (Step 3 + 4) |
| `grove-explore/src/config_tui/view.rs` | Moved + AC4 edits: remove mode-badge and inert-render rows (Step 3 + 4) |
| `grove-explore/src/trace_tui/mod.rs` | Moved + string sweep (Step 3 + 5) |
| `grove-explore/src/trace_tui/model.rs` | Moved verbatim (Step 3) |
| `grove-explore/src/trace_tui/update.rs` | Moved verbatim (Step 3) |
| `grove-explore/src/trace_tui/view.rs` | Moved verbatim (Step 3) |
| `grove-explore/src/tap.rs` | Moved + config.json read-modify-write + string sweep (Step 3 + 5) |

## Files to Delete

| File | Reason |
|------|--------|
| `cli/src/config_tui/mod.rs` | Moved to grove-explore |
| `cli/src/config_tui/model.rs` | Moved to grove-explore |
| `cli/src/config_tui/update.rs` | Moved to grove-explore |
| `cli/src/config_tui/view.rs` | Moved to grove-explore |
| `cli/src/trace_tui/mod.rs` | Moved to grove-explore |
| `cli/src/trace_tui/model.rs` | Moved to grove-explore |
| `cli/src/trace_tui/update.rs` | Moved to grove-explore |
| `cli/src/trace_tui/view.rs` | Moved to grove-explore |
| `cli/src/tap.rs` | Moved to grove-explore |
| `cli/src/bin/grove_explore.rs` | Content merges into grove-explore/src/main.rs |

## Data Model Changes

- **`App` struct** (`config_tui/model.rs`): Drop both `grove_mode: Mode` and `explore_active: bool`; drop `Mode` import. `from_grove_config()` no longer reads `.mode` from `GroveConfig` into `App` state.
- **Save path** (`config_tui/mod.rs`): After `app.to_config()`, re-read `GroveConfig::load(root)` to obtain both `mode` and `harnesses` before constructing the new `GroveConfig` to save. This preserves mode and harnesses in a single disk read (one `GroveConfig::load` call covers both, so the existing two-field pattern consolidates naturally).
- **`tap.rs` write path**: `ExploreConfig::save` (→ `explore.json`) replaced by GroveConfig read-modify-write (→ `config.json`). `ExploreConfig` import retained for deserialization within the modify step.

## Testing Strategy

### Unit tests (in moved modules)

- **Delete**: `badge_reflects_grove_mode`, `explore_inert_blocks_all_edits`, `save_blocked_when_inert`, `save_allowed_when_mcp_llm`
- **Existing passing tests**: all other `config_tui` and `trace_tui` inline unit tests — navigation, update logic, model construction — are updated where they directly construct `App { grove_mode, explore_active, ... }` to use the new field-agnostic constructors
- **Add**: `config_round_trip_preserves_mode_and_harnesses` — builds from a temp dir's `config.json` with `mode=mcp`; simulates save path; asserts `mode == Mcp` and harnesses are preserved (not cleared, not forced to McpLlm)

### Integration tests (`cli/tests/cli.rs`)

| Test name | Action |
|-----------|--------|
| `config_in_non_tty_fails_fast` | Update to call `grove-explore config` via `grove_explore_bin()` directly |
| `tap_enables_tracing_in_config` | Update: seed `config.json` (not `explore.json`); assert on `config.json` after run |
| `tap_no_enable_leaves_config_untouched` | Update: seed `config.json`; assert `explore.tap == false` in `config.json` |
| `grove_explore_startup_fails_on_unhealthy_provider` | Update invocation to `grove-explore serve /path` (was positional path) |
| `grove_explore_mcp_server_responds` | Update invocation similarly to `serve` subcommand |
| `grove_config_shim_prints_new_spelling` | New: `grove config` non-TTY stderr contains `"grove-explore config"` |
| `grove_tap_shim_prints_new_spelling` | New: `grove tap` non-TTY stderr contains `"grove-explore tap"` |
| `grove_explore_config_non_tty_fails_fast` | New: `grove-explore config` exits non-zero; stderr contains `"interactive terminal"` |
| `grove_explore_tap_enables_tracing_in_config_json` | New: `grove-explore tap` sets `explore.tap=true` in `config.json` |
| `init_first_run_grove_explore_absent` | New: `grove init --as mcp-llm` fails gracefully when `grove-explore` binary absent |

### Build / lint

```sh
cargo test --release --locked
cargo clippy --all-targets --workspace --locked -- -D warnings
```

Both must be green and warning-clean.

### Dependency check (AC6)

```sh
# grove binary must have NO ratatui/crossterm in its dep tree
cargo tree -p grove-cst-cli --edges no-dev | grep -E "ratatui|crossterm"
# Must produce no output

# grove-explore-bin must carry both
cargo tree -p grove-explore-bin --edges no-dev | grep -E "ratatui|crossterm"
# Must show both crates
```

## Acceptance Criteria Checklist

| AC | Plan coverage |
|----|--------------|
| AC1: `grove-explore config`/`tap` provide today's TUIs unchanged | Step 3 — modules moved verbatim; AC4 removals are targeted |
| AC2: `grove config`/`tap` shims print new spelling | Step 8 — unconditional `eprintln!` before exec; tests `grove_config_shim_prints_new_spelling` + `grove_tap_shim_prints_new_spelling` |
| AC3: String sweep (tap hints, no explore.json refs) | Step 5 — all user strings updated |
| AC4: Mode-gated inert rendering removed | Step 4 — `grove_mode`/`explore_active` fields and all conditional rendering removed |
| AC5: `config_tui` preserves `mode` + `harnesses` on save | Step 6 revised — save path re-reads mode from disk; `config_round_trip_preserves_mode_and_harnesses` asserts `mode == Mcp` (input preserved, not forced to McpLlm) |
| AC6: ratatui/crossterm absent from `grove` dep tree | Steps 1 + 7 — moved to `grove-explore-bin` only; `cargo tree` check |
| AC7: Workspace green (tests, clippy, warnings) | Steps 7–10 — all compile breaks addressed including `init.rs` (Step 8a) |

## Operational Impact

- **Version bump:** rides the next release train; shim deprecation noted in changelog.
- **Regeneration:** none — old `grove config` / `grove tap` spellings keep working for one release via exec shims.
- **Security scan:** not required.
- **`init.rs` first-run behavior:** first-run TUI on `grove init --as mcp-llm` now execs `grove-explore config` instead of the in-process TUI. If `grove-explore` is absent from the sibling directory, `grove init` fails with a clear "grove-explore not found" error (the same failure mode as when `grove-explore serve` is absent in T02).
