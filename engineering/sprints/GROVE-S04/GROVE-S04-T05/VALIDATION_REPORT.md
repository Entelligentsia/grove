# GROVE-S04-T05 Validation Report (standalone review)

**Task:** Move config/trace TUIs + `tap` to `grove-explore`; forwarding shims + string sweep
**Verdict:** ✅ Approved

---

## Summary

All 7 acceptance criteria from the task prompt were verified against the on-disk implementation and the live test suite. The workspace is green: 340 tests pass, 0 failures, clippy clean. The cold-target AC7 blocker (on-demand binary build) was independently reproduced and confirmed resolved.

---

## Acceptance Criteria — Verdict Per Criterion

### AC1 — `grove-explore config` and `grove-explore tap` provide today's TUIs unchanged ✅ PASS

**Evidence:**
- `grove-explore/src/` contains `config_tui/`, `trace_tui/`, and `tap.rs` (all 9 files moved)
- `grove-explore/src/main.rs` dispatches `serve`/`config`/`tap`; bare invocation defaults to `Cmd::Serve` (`.mcp.json` compatibility preserved)
- `config_tui/model.rs`: `Field::{Engine, Url, Model, Tap, Tools}` present — engine discovery picker, model dropdown, Tap toggle all intact
- `trace_tui/mod.rs` doc: "Drills through recorded explore sessions: session list → call list → per-call turn detail" — session→call→turn browser confirmed
- 42 grove-explore-bin unit tests pass (including `config_tui::update::tests::*` and `trace_tui::*`)

---

### AC2 — `grove config`/`grove tap` shims print new spelling ✅ PASS

**Evidence:**
- `cli/src/main.rs` Config arm (line 370): `eprintln!("note: \`grove config\` is deprecated; use \`grove-explore config\` instead")`
- `cli/src/main.rs` Tap arm (line 430): `eprintln!("note: \`grove tap\` is deprecated; use \`grove-explore tap\` instead")`
- Both arms exec-forward via `find_explore_binary()` — new spelling is printed unconditionally, before the exec attempt
- When binary absent: actionable error referencing the new spelling also printed
- Integration tests `grove_config_shim_prints_new_spelling` and `grove_tap_shim_prints_new_spelling` both pass (confirmed in 5-test run above)

---

### AC3 — String sweep: tap hints name `grove-explore`, no `explore.json` user refs ✅ PASS

**Evidence:**
- `grove-explore/src/tap.rs` line 47-49: `"grove-explore tap: tracing enabled — restart the \`grove-explore\` server for it to take effect"` — no "restart grove serve"
- Error path (lines 54-58): references `grove init --as mcp-llm` or `grove-explore config` — no explore.json mention
- Only `explore.json` references in tap.rs are doc-comments (not user-facing strings)
- `trace_tui/mod.rs`: zero explore.json / grove-serve / grove-tap user strings
- `grove-explore/src/main.rs` serve error messages reference `grove-explore config` throughout

---

### AC4 — Mode-gated inert rendering removed ✅ PASS

**Evidence:**
- `grove-explore/src/config_tui/model.rs`: no `grove_mode` or `explore_active` fields; comment at line 134 confirms removal ("AC4: `grove_mode` and `explore_active` have been removed")
- `grove-explore/src/config_tui/view.rs` line 3: "AC4: mode-badge title, explore-notice row, and explore_active guards removed" — confirmed via grep (zero occurrences of `grove_mode`/`explore_active` in update.rs)
- `grove-explore/src/config_tui/view.rs` line 26: "Outer chrome — static title (mode badge removed in AC4)"
- The 4 previously inert unit tests deleted from update.rs — 42 remaining tests all meaningful

---

### AC5 — `config_tui` preserves `mode` + `harnesses` on save ✅ PASS

**Evidence:**
- `grove-explore/src/config_tui/mod.rs` lines 95-112: save path calls `GroveConfig::load(root).unwrap_or_default()`, extracts `harnesses` and `mode`, then reconstructs `GroveConfig` with those values preserved — mode never forced to `McpLlm`
- `grove-explore/src/config_tui/update.rs::config_round_trip_preserves_mode_and_harnesses` (line 399): builds a config with `mode=mcp`, simulates the full save path, reads back and asserts `readback.mode == Mode::Mcp` and `harnesses == [ClaudeCode, Codex]`
- Test passes in the 42-test grove-explore-bin suite
- `tap.rs` also does GroveConfig read-modify-write (lines 28-45) preserving all fields outside the `explore` section

---

### AC6 — ratatui/crossterm absent from `grove` dep tree ✅ PASS

**Evidence:**
- `cargo tree -p grove-cst-cli | grep -E 'ratatui|crossterm'` → no output
- `cargo tree -p grove-explore-bin | grep -E 'ratatui|crossterm'` → `├── crossterm v0.28.1` and `├── ratatui v0.29.0`
- `cli/Cargo.toml` has no ratatui/crossterm entries
- `cli/src/main.rs` has no `mod config_tui`, `mod trace_tui`, or `mod tap` declarations

---

### AC7 — Workspace green (tests, clippy, warning-clean) ✅ PASS

**Evidence:**

**Full workspace suite:**
```
cargo test --release --locked --workspace
running 136 tests → test result: ok. 136 passed; 0 failed
running 65 tests  → test result: ok. 65 passed; 0 failed
running 42 tests  → test result: ok. 42 passed; 0 failed
running 42 tests  → test result: ok. 42 passed; 0 failed
running 54 tests  → test result: ok. 54 passed; 0 failed
running 1 test    → test result: ok. 1 passed; 0 failed
running 0 tests   → test result: ok. 0 passed; 0 failed
Total: 340 passed; 0 failed
```

**Clippy:**
```
cargo clippy --all-targets --workspace -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
```
Clean — exit 0.

**Cold-target AC7 (independently reproduced):**
```
$ rm target/release/grove-explore
$ cargo test --release --locked -p grove-cst-cli --test cli -- grove_explore_config_non_tty_fails_fast
   Compiling grove-explore-core v0.4.1 (…)
   Compiling grove-explore-bin v0.4.1 (…)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out
```
`grove_explore_bin()` fired `cargo build --locked -p grove-explore-bin` mid-run, binary rebuilt, test passed — AC7 cold-target blocker confirmed resolved.

**init.rs first-run fix (blocker 1):**
- `cli/src/init.rs` line 132-145: calls `find_explore_binary()` then `std::process::Command::new(&bin).arg("config").arg(root).status()` — no longer calls `crate::config_tui::run`

---

## Regression Assessment

No regressions detected. The 42 previously-existing integration tests in `cli/tests/cli.rs` continue to pass. The `tap_enables_tracing_in_config` and `tap_no_enable_leaves_config_untouched` tests were updated to seed/assert `config.json` (not `explore.json`) — correct and consistent with AC3/AC5.

---

## Advisories (non-blocking, inherited from code review)

1. The `config_round_trip_preserves_mode_and_harnesses` test re-implements the save sequence inline rather than exercising the real `event_loop` Save arm — adequate coverage for AC5, but a shared helper would reduce drift risk.
2. `init.rs` first-run guard still keys on `.grove/explore.json` as the bypass sentinel (pre-existing behaviour, acceptable during the one-release deprecation window).

Neither advisory blocks approval.
