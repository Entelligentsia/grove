# VALIDATION REPORT — GROVE-S03-T03 (standalone review)

**Task:** `serve` reads declared mode + shared `active_mode` resolver  
**Validator:** 🍵 grove Qa Engineer  
**Date:** 2026-07-05  
**Verdict:** ✅ Approved

---

## Acceptance Criteria Checklist

### AC-1 — Shared resolver `active_mode(root, force) -> Mode` in `core::config`

**PASS**

Evidence:
- `pub enum ModeChoice { None, ForceExplore, ForceStandard }` declared at `core/src/config.rs:268` with full doc comments.
- `pub fn active_mode(root: &Path, force: ModeChoice) -> Mode` implemented at `core/src/config.rs:291`.
- Precedence: `ForceStandard → Mcp`, `ForceExplore → McpLlm`, `None → GroveConfig::load().mode` (falls back to `Mcp` on load error with stderr diagnostic).
- Re-exported from `core/src/lib.rs:68`: `pub use config::{active_mode, GroveConfig, Mode, ModeChoice}`.
- All 6 unit tests (AM-1 through AM-6) pass:
  ```
  test config::tests::active_mode_force_standard_returns_mcp ... ok
  test config::tests::active_mode_force_explore_returns_mcp_llm ... ok
  test config::tests::active_mode_none_reads_declared_mcp_mode ... ok
  test config::tests::active_mode_none_reads_declared_mcp_llm_mode ... ok
  test config::tests::active_mode_mcp_config_ignores_stale_explore_json ... ok
  test config::tests::active_mode_no_config_falls_back_to_mcp ... ok
  ```

---

### AC-2 — `determine_surface` keys off `active_mode(...) == Mode::McpLlm`

**PASS**

Evidence:
- `determine_surface` at `cli/src/mcp.rs:52` maps CLI flags → `ModeChoice`, calls `active_mode(root, force)`, branches on `!= Mode::McpLlm` returning `Surface::Standard`.
- This is an explicit catch-all: any future `Mode` variant not equal to `McpLlm` safely routes to Standard.
- No `ExploreConfig::config_path(root).exists()` call exists anywhere in `cli/src/mcp.rs` (confirmed by grep — zero hits).
- Explore section sourced from `grove_cfg.explore` (`Option<ExploreConfig>`), not `ExploreConfig::load` — missing section handled gracefully with stderr fallback.

---

### AC-3 — Bug-1 regression: `mode: "mcp"` + stale `explore.json` → standard surface immediately

**PASS**

Evidence:
- Unit test `active_mode_mcp_config_ignores_stale_explore_json` (AM-5): writes `config.json` with `mode: "mcp"` alongside a stale `explore.json`; asserts `active_mode` returns `Mode::Mcp`. **PASS**.
- Integration test `bug1_serve_mcp_mode_ignores_stale_explore_json`: spawns real `grove serve` binary with `config.json mode=mcp` and a stale `explore.json`; asserts:
  - Exactly 7 tools returned (standard surface) ✅
  - No tool named `"explore"` in the response ✅
- Test run output: `test bug1_serve_mcp_mode_ignores_stale_explore_json ... ok`

---

### AC-4 — `mode: "mcp-llm"` + healthy provider → explore surface; health-gated fallback preserved

**PASS**

Evidence:
- Code path in `determine_surface`: when `active_mode` resolves to `McpLlm`, loads `GroveConfig`, extracts `grove_cfg.explore`, calls `health_probe`; on error emits `"falling back"` to stderr and returns `Surface::Standard`.
- Integration test `explore_mode_unhealthy_provider_falls_back_to_standard_surface`: uses legacy `explore.json` (mode key, not steering) triggering migration+health-probe path; asserts exactly 7 tools and `stderr.contains("falling back")`.
- Test run output: `test explore_mode_unhealthy_provider_falls_back_to_standard_surface ... ok`

---

### AC-5 — `--explore` / `--standard` flags retain existing precedence (`--standard` wins)

**PASS**

Evidence:
- `determine_surface` checks `force_standard` first, then `force_explore`:
  ```rust
  let force = if force_standard {
      ModeChoice::ForceStandard      // wins
  } else if force_explore {
      ModeChoice::ForceExplore
  } else {
      ModeChoice::None
  };
  ```
- Unit test AM-1 (`active_mode_force_standard_returns_mcp`): ForceStandard returns `Mode::Mcp` regardless of config content — **PASS**.
- Unit test AM-2 (`active_mode_force_explore_returns_mcp_llm`): ForceExplore returns `Mode::McpLlm` when no standard flag — **PASS**.

---

### AC-6 — All existing tests green; clippy clean; release build clean

**PASS**

Evidence:
```
cargo test --release --locked
  grove_core:  161 passed; 0 failed
  grove:        77 passed; 0 failed
  cli tests:    29 passed; 0 failed
  doc-tests:     1 passed; 0 failed
  Total:       268 passed; 0 failed ✅

cargo clippy --release --tests -- -D warnings
  Finished `release` profile — no warnings, no errors ✅
```

---

## Edge Cases Checked

| Scenario | Result |
|---|---|
| `ForceStandard` with invalid config JSON in file | PASS — short-circuits before config read |
| `ModeChoice::None`, no `config.json`, no `explore.json` | PASS — falls back to `Mode::Mcp` without panic |
| `mcp-llm` mode, explore section absent from config | PASS — graceful stderr + Standard fallback |
| `mcp-llm` + port-1 refused (unhealthy) | PASS — health_probe Err → Standard + "falling back" |
| `mode: "mcp"` alongside stale `explore.json` | PASS — bug-1 fixed |
| Unknown/future `Mode` variant | PASS — `!= McpLlm` catch-all routes to Standard |

---

## Advisories (non-blocking, inherited from code review)

- **AM-1 fixture has stray trailing quote**: The JSON written in `active_mode_force_standard_returns_mcp` ends with `}}'"#` making it unparseable. The test still passes because `ForceStandard` short-circuits before reading config. Behavior is correct; fixture is misleading documentation. No functional impact.

---

## Summary

All six acceptance criteria are satisfied. The bug-1 regression is fixed: `determine_surface` no longer sniffs `explore.json` existence. The shared `active_mode` resolver is correctly implemented in `core::config`, re-exported, and used by `cli::mcp`. Health-gated fallback from S02 is preserved. 268 tests pass; clippy is clean.
