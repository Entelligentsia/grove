# Validation Report — GROVE-S04-T03 (standalone review)

**Task:** `grove serve` always structural — delete `Surface`/`determine_surface`/serve mode flags/health fallback
**Validator:** 🍵 grove Qa Engineer — I validate against what was promised. The code compiling is not enough.
**Verdict:** Approved

---

## Acceptance Criteria Results

### AC1 — `Surface`, `determine_surface`, health-probe fallback deleted; zero `explore` imports in `cli/src/mcp.rs` ✅ PASS

**Evidence:**
- `grep -n "explore\|Surface\|determine_surface" cli/src/mcp.rs` → zero hits.
- `grep -n "grove_explore_core\|active_mode\|GroveConfig\|Mode\|ModeChoice" cli/src/mcp.rs` → zero hits.
- `mcp__grove__outline cli/src/mcp.rs` confirms: `Surface` enum absent, `determine_surface` absent, `open_session_trace`/`enum_str`/`explore_instructions`/`explore_tool_spec`/`resolve_explore_question`/`StdoutProgress`/`call_explore_tool` — all absent.
- Retained: `serve(_root: &Path) -> Result<()>`, `handle(method, params) -> Outcome`, `instructions()`, `tool_specs()`, `call_tool()`, `Outcome` enum — exactly the structural surface, nothing else.

---

### AC2 — `serve --explore` / `serve --standard` exit non-zero with error naming `grove-explore serve` ✅ PASS

**Evidence:**
- `cli/src/main.rs:189–194`: both flags are declared `#[arg(hide = true)]` so they parse without a clap error.
- `cli/src/main.rs:408–414`: dispatch arm fires `anyhow::bail!` naming `grove-explore serve` as the LLM-delegating replacement and `grove serve` as the structural replacement.
- Integration test `serve_removed_explore_flag_errors_with_hint` (line 522): asserts `grove serve --explore` and `grove serve --standard` exit non-zero and stderr contains `"grove-explore serve"` → **PASS** in the full test run.

---

### AC3 — `grove serve` returns 7 structural tools regardless of mode/config/provider health ✅ PASS

**Evidence:**
- `serve_always_serves_structural_surface` (line 429): spawns grove with `mode=mcp-llm`, unreachable provider URL `http://127.0.0.1:1/v1`; sends `initialize` + `tools/list`; asserts `tools.len() == 7` and stderr does NOT contain `"falling back"` → **PASS**.
- `serve_mcp_llm_config_returns_7_structural_tools` (line 556): spawns grove with `mode=mcp-llm` and no `explore` section; asserts 7 tools and no `"explore"` tool name → **PASS**.
- `bug1_serve_mcp_mode_ignores_stale_explore_json` (line 629): stale `explore.json` alongside `config.json`; asserts 7 structural tools, `"explore"` tool absent → **PASS**.
- All 37 integration tests ran: `test result: ok. 37 passed; 0 failed`.

---

### AC4 — ratatui/crossterm no longer dependencies of the `grove` binary ⚠️ DEFERRED TO T05 (sanctioned)

**Evidence:**
- `cargo tree --manifest-path cli/Cargo.toml | grep -i "ratatui\|crossterm"` → finds `ratatui v0.29.0` and `crossterm v0.28.1` as direct deps.
- Source of pull: `cli/src/trace_tui/` and `cli/src/config_tui/` use both crates. These are the `grove tap` and `grove config` TUI modules.
- `cli/src/mcp.rs` and `cli/src/main.rs` (serve dispatch) have **zero** ratatui/crossterm imports — the serve path is fully decoupled.
- **AC4 as written** says "the TUI code itself moves in T05 — this task covers the serve-path decoupling and must be consistent with T05's move." The serve-path decoupling is complete. The full cargo dep removal requires T05 to move the TUI crates out of `cli/`.
- Approved PLAN marks AC4 "N/A (serve-path decoupling only; Cargo.toml prune lands with T05)". PLAN_REVIEW and CODE_REVIEW both confirm this as a forced, sanctioned deferral. The joint T03+T05 cargo-tree gate must run when T05 lands.
- **This gap is known, sanctioned, and tracked — not a blocking failure for T03.**

---

### AC5 — Existing structural-surface integration tests pass; deleted-flag tests replaced ✅ PASS

**Evidence:**
- `explore_mode_unhealthy_provider_falls_back_to_standard_surface` → deleted (verified: name does not appear in `cli/tests/cli.rs`).
- Replaced by `serve_always_serves_structural_surface` — asserts constant-surface invariant (no "falling back"), not a fallback.
- `bug1_serve_mcp_mode_ignores_stale_explore_json` updated: assertions retargeted to constant-surface language (`!contains("falling back")`), inverted sign preserved correctly.
- Full 37-test suite: `test result: ok. 37 passed; 0 failed`.
- Unit tests: `109 passed; 0 failed` (all mcp:: unit tests updated to drop `&Surface::Standard, None` args from `handle()` call sites).

---

### AC6 — Workspace green: `cargo test`, `cargo clippy --all-targets --workspace --locked -- -D warnings`, warning-clean, newlines ✅ PASS

**Evidence:**
- `cargo test --release --locked` full run: **336 tests pass** (135 core + 109 cli unit + 37 integration + 54 explore + 1 doc), 0 failures.
- `cargo clippy --all-targets --workspace --locked -- -D warnings`: **zero warnings, zero errors** (`Finished dev profile` — clean exit).
- Newline-at-EOF: `tail -c 1 | xxd` on all three changed files confirms trailing `0a` byte:
  - `cli/src/mcp.rs` ✅
  - `cli/src/main.rs` ✅
  - `cli/tests/cli.rs` ✅

---

## Edge Cases Checked

| Boundary | Outcome |
|---|---|
| `mode=mcp-llm` + unreachable explore provider | 7 structural tools, no health probe, no fallback stderr |
| `mode=mcp-llm` + no explore section in config | 7 structural tools, no explore tool in list |
| Stale `explore.json` alongside `config.json` | 7 structural tools, explore tool absent |
| `grove serve --explore` (hidden flag) | Non-zero exit, error names `grove-explore serve` |
| `grove serve --standard` (hidden flag) | Non-zero exit, error names `grove-explore serve` |
| ratatui/crossterm in mcp::serve path | Zero imports — serve path fully decoupled |

---

## Regression Check

All 37 pre-existing integration tests pass. The binary unit test suite (109 tests in `cli`) passes, including the 7 updated `handle()` call sites in `mod tests` (dropped `&Surface::Standard, None` args). The 3 deleted `resolve_explore_question` unit tests are confirmed absent.

---

## Summary

**Verdict: Approved**

ACs 1, 2, 3, 5, and 6 are fully met with direct test evidence. AC4's serve-path decoupling (T03 scope) is complete — mcp.rs has zero ratatui/crossterm imports. The full cargo dep prune is a sanctioned T05 deferral acknowledged in the approved PLAN, PLAN_REVIEW, and CODE_REVIEW; the joint T03+T05 `cargo tree` gate must run when T05 lands.
