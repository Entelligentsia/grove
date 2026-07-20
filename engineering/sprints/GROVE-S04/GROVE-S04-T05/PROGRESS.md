# GROVE-S04-T05 Progress: Move config/trace TUIs + tap to grove-explore; forwarding shims + string sweep

## Summary

Implemented the full plan: moved config_tui, trace_tui, and tap.rs from the grove CLI crate into a new `grove-explore` workspace crate (`grove-explore-bin`), wired forwarding shims in the grove CLI, fixed the init.rs first-run TUI call, preserved GroveConfig.mode on save, swept all single-server user strings, and fixed the AC7 cold-target CI blocker (the `grove_explore_bin()` test helper now uses `std::sync::Once` + on-demand `cargo build --locked -p grove-explore-bin` so `cargo test --workspace --locked` is self-contained from a clean checkout).

## Changes Made

### AC1 — TUIs moved to grove-explore
- **Created** `grove-explore/Cargo.toml` — new workspace crate with ratatui+crossterm deps; package name `grove-explore-bin`.
- **Created** `grove-explore/src/main.rs` — subcommand dispatcher: `serve`/`config`/`tap`; bare invocation defaults to `Cmd::Serve` so `.mcp.json` still works.
- **Moved** 9 files verbatim (with targeted edits):
  - `cli/src/config_tui/{mod,model,update,view}.rs` → `grove-explore/src/config_tui/`
  - `cli/src/trace_tui/{mod,model,update,view}.rs` → `grove-explore/src/trace_tui/`
  - `cli/src/tap.rs` → `grove-explore/src/tap.rs`
- **Deleted** original paths from `cli/src/`.
- **Deleted** `cli/src/bin/grove_explore.rs` (merged into grove-explore/src/main.rs).

### AC2 — Forwarding shims in grove CLI
- `cli/src/main.rs` Config/Tap match arms print `note: 'grove config'/'tap' is deprecated; use 'grove-explore config'/'tap' instead` to stderr, then exec-forward to `grove-explore`; shims work robustly even if `grove-explore` is absent (prints `grove-explore not found` and exits non-zero).
- Removed 3 `mod` declarations (`config_tui`, `trace_tui`, `tap`).

### AC3 — String sweep
- `grove-explore/src/tap.rs`: reads `.grove/config.json` (GroveConfig read-modify-write), flips `explore.tap`, preserves mode/harnesses. All user hint strings updated: no `explore.json` refs, no `grove serve` refs.
- `grove-explore/src/trace_tui/mod.rs`: module doc updated to `grove-explore tap` / `grove-explore` server.
- `grove-explore/src/config_tui/view.rs`: browse hint updated to `grove-explore tap`.

### AC4 — Mode-gated inert rendering removed
- `grove-explore/src/config_tui/model.rs`: `grove_mode: Mode` and `explore_active: bool` fields deleted; `Mode` import removed; `from_grove_config()` no longer reads mode into App state.
- `grove-explore/src/config_tui/view.rs`: mode-badge row and explore-notice row removed.
- `grove-explore/src/config_tui/update.rs`: 4 inert-guard unit tests deleted (`badge_reflects_grove_mode`, `explore_inert_blocks_all_edits`, `save_blocked_when_inert`, `save_allowed_when_mcp_llm`).

### AC5 — config_tui preserves mode + harnesses on save
- `grove-explore/src/config_tui/mod.rs`: save path calls `GroveConfig::load(root).unwrap_or_default()` to read current mode+harnesses before writing, never forces McpLlm.
- `grove-explore/src/config_tui/update.rs`: `config_round_trip_preserves_mode_and_harnesses` test seeds `config.json` with `mode=mcp`, simulates save, asserts `mode == Mode::Mcp` and harnesses preserved.

### AC6 — ratatui/crossterm removed from grove dep tree
- `cli/Cargo.toml`: ratatui + crossterm deps removed; `[[bin]] grove-explore` block removed.
- `Cargo.toml`: added `"grove-explore"` to workspace members.
- `cargo tree -p grove-cst-cli`: ratatui/crossterm ABSENT.
- `cargo tree -p grove-explore-bin`: ratatui/crossterm PRESENT.

### AC7 — Workspace green from cold target (blocker fix)
- `cli/tests/cli.rs` `grove_explore_bin()` helper rewritten: `std::sync::Once` + on-demand `cargo build --locked -p grove-explore-bin` (detects release vs debug, respects `$CARGO` env var). Warm-target runs short-circuit on `bin.exists()`.
- `let _ = grove_explore_bin();` guard added to `tap_enables_tracing_in_config` and `tap_no_enable_leaves_config_untouched` tests.
- 5 new integration tests added: `grove_config_shim_prints_new_spelling`, `grove_tap_shim_prints_new_spelling`, `grove_explore_config_non_tty_fails_fast`, `grove_explore_tap_enables_tracing_in_config_json`, `init_first_run_grove_explore_absent`.
- Existing tests updated: `config_in_non_tty_fails_fast`, `tap_enables_tracing_in_config`, `tap_no_enable_leaves_config_untouched`, `grove_explore_startup_fails_on_unhealthy_provider`, `grove_explore_mcp_server_responds`.

### init.rs fix (blocker 1)
- `cli/src/init.rs`: first-run TUI call replaced with `find_explore_binary()` exec of `grove-explore config <root>`.

## Test Evidence

### Cold-target verification
```
$ rm target/release/grove-explore
$ cargo test --release --locked -p grove-cst-cli --test cli
   Compiling grove-explore-core v0.4.1 (…/explore)
   Compiling grove-explore-bin v0.4.1 (…/grove-explore)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.66s
```
"Compiling grove-explore-bin" observed mid-run — on-demand build fired, all tests pass.

### Full workspace test suite
```
$ cargo test --release --locked --workspace
   Compiling grove-explore-core v0.4.1 (…)
   Compiling grove-explore-bin v0.4.1 (…)
running 136 tests  → test result: ok. 136 passed; 0 failed
running 65 tests   → test result: ok. 65 passed; 0 failed
running 42 tests   → test result: ok. 42 passed; 0 failed
running 42 tests   → test result: ok. 42 passed; 0 failed
running 54 tests   → test result: ok. 54 passed; 0 failed
running 1 test     → test result: ok. 1 passed; 0 failed
running 0 tests    → test result: ok. 0 passed; 0 failed
Total: 340 passed; 0 failed
```

### Clippy
```
$ cargo clippy --all-targets --workspace -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```
Clean — exit 0.

### AC6 dependency check
```
$ cargo tree -p grove-cst-cli | grep -E 'ratatui|crossterm'
(no output — absent from grove binary dep tree) ✅

$ cargo tree -p grove-explore-bin | grep -E 'ratatui|crossterm'
├── crossterm v0.28.1
├── ratatui v0.29.0 ✅
```

## Files Changed

### Created
- `grove-explore/Cargo.toml`
- `grove-explore/src/main.rs`
- `grove-explore/src/config_tui/mod.rs`
- `grove-explore/src/config_tui/model.rs`
- `grove-explore/src/config_tui/update.rs`
- `grove-explore/src/config_tui/view.rs`
- `grove-explore/src/tap.rs`
- `grove-explore/src/trace_tui/mod.rs`
- `grove-explore/src/trace_tui/model.rs`
- `grove-explore/src/trace_tui/update.rs`
- `grove-explore/src/trace_tui/view.rs`

### Modified
- `Cargo.toml` — workspace members
- `Cargo.lock` — updated
- `cli/Cargo.toml` — removed ratatui+crossterm deps and [[bin]] grove-explore block
- `cli/src/main.rs` — removed mod decls, replaced Config/Tap arms with shims
- `cli/src/init.rs` — first-run TUI → grove-explore exec
- `cli/tests/cli.rs` — grove_explore_bin() helper, 5 new tests, updated existing tests

### Deleted
- `cli/src/bin/grove_explore.rs`
- `cli/src/config_tui/mod.rs`
- `cli/src/config_tui/model.rs`
- `cli/src/config_tui/update.rs`
- `cli/src/config_tui/view.rs`
- `cli/src/tap.rs`
- `cli/src/trace_tui/mod.rs`
- `cli/src/trace_tui/model.rs`
- `cli/src/trace_tui/update.rs`
- `cli/src/trace_tui/view.rs`
