# GROVE-S03-T03 — Implementation Progress

## Summary

Implemented the `active_mode(root, ModeChoice) -> Mode` shared resolver in `core/src/config.rs`
and rewrote `determine_surface` in `cli/src/mcp.rs` to use it, fixing bug-1 where a stale
`.grove/explore.json` would activate explore mode even when `.grove/config.json` declared
`mode: "mcp"`.

## Changes Made

### `core/src/config.rs`

- Added `pub enum ModeChoice { None, ForceExplore, ForceStandard }` — encodes CLI-flag intent
  without threading boolean pairs through call sites.
- Added `pub fn active_mode(root: &Path, force: ModeChoice) -> Mode` — pure config resolver
  with precedence: `ForceStandard` → Mcp; `ForceExplore` → McpLlm; `None` → load
  `GroveConfig` and return `cfg.mode` (falls back to Mcp on load failure with stderr diagnostic).
- Added 6 unit tests:
  - `active_mode_force_standard_returns_mcp`
  - `active_mode_force_explore_returns_mcp_llm`
  - `active_mode_none_reads_declared_mcp_mode`
  - `active_mode_none_reads_declared_mcp_llm_mode`
  - `active_mode_mcp_config_ignores_stale_explore_json` (bug-1 regression)
  - `active_mode_no_config_falls_back_to_mcp`

### `core/src/lib.rs`

- Extended `pub use config::{GroveConfig, Mode}` to include `active_mode` and `ModeChoice`,
  making them available as `grove_core::active_mode` / `grove_core::ModeChoice`.

### `cli/src/mcp.rs`

- Added import: `use grove_core::config::{active_mode, GroveConfig, Mode, ModeChoice};`
- Rewrote `determine_surface`:
  - Maps `(force_standard, force_explore)` to `ModeChoice`
  - Calls `active_mode(root, force)` — no more `ExploreConfig::config_path(root).exists()` sniff
  - If result != `Mode::McpLlm`: return `Surface::Standard` immediately
  - If `McpLlm`: load `GroveConfig` to get `explore` section, then call `health_probe`
  - Health-gated fallback (unhealthy provider → standard) preserved unchanged
- Updated doc comment to explain the bug-1 fix and reference GROVE-S03-T03.

### `cli/tests/cli.rs`

- Updated comment on `explore_mode_unhealthy_provider_falls_back_to_standard_surface` to
  reference T03 and explain the new code path (legacy explore.json migrated → McpLlm →
  health probe fires).
- Fixed test fixture to use legacy explore.json format (`"mode"` key) so `GroveConfig::load`
  can migrate it correctly, allowing `active_mode` to return `McpLlm` and the health probe
  to fire.
- Added new integration test `bug1_serve_mcp_mode_ignores_stale_explore_json`:
  - Writes `config.json` with `mode: "mcp"` + stale `explore.json`
  - Spawns `grove serve`, sends `initialize` + `tools/list`
  - Asserts exactly 7 tools (standard surface) — not 1 explore tool
  - Asserts the `"explore"` delegating tool is absent

## Test Evidence

```
running 161 tests
test config::tests::active_mode_force_standard_returns_mcp ... ok
test config::tests::active_mode_force_explore_returns_mcp_llm ... ok
test config::tests::active_mode_no_config_falls_back_to_mcp ... ok
test config::tests::active_mode_mcp_config_ignores_stale_explore_json ... ok
test config::tests::active_mode_none_reads_declared_mcp_llm_mode ... ok
test config::tests::active_mode_none_reads_declared_mcp_mode ... ok
...
test result: ok. 161 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s

running 77 tests
...
test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

running 29 tests
test bug1_serve_mcp_mode_ignores_stale_explore_json ... ok
test explore_mode_unhealthy_provider_falls_back_to_standard_surface ... ok
...
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

Doc-tests grove_core
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

Total: 268 tests, 0 failed.
```

clippy: `Finished dev profile — no warnings`

## Files Changed

| File | Change |
|------|--------|
| `core/src/config.rs` | Added `ModeChoice` enum, `active_mode` fn, 6 unit tests |
| `core/src/lib.rs` | Re-exported `active_mode` and `ModeChoice` |
| `cli/src/mcp.rs` | Rewrote `determine_surface`, added config import |
| `cli/tests/cli.rs` | Updated existing test comment + fixture; added bug1 regression test |
