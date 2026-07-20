# CODE_REVIEW.md — GROVE-S04-T04 (iteration 1 of 3)

**Verdict:** Approved

## Summary

The implementation correctly re-keys `Mode::McpLlm` to the both-servers layout: `grove init --as mcp-llm` now registers **both** the structural `grove serve` server **and** the `grove-explore` MCP server. All plan steps were executed, and the code aligns with the acceptance criteria.

## Verification Checklist

### Plan Compliance

| Step | Status | Evidence |
|------|--------|----------|
| 1a. Add `EXPLORE_SERVER_KEY` | Done | `core/src/harness.rs:33` — `pub const EXPLORE_SERVER_KEY: &str = "grove-explore";` |
| 1b. Fix `expected_mcp_args(McpLlm)` | Done | Line 273: `Mode::Mcp | Mode::Both | Mode::McpLlm => Some(&["serve"])` |
| 1c. Add `expected_explore_args` | Done | Lines 282-287 — returns `Some(&[])` only for `McpLlm` |
| 1d. Update `expected_claude_marker(McpLlm)` | Done | Line 294: `Mode::McpLlm => Some("mcp__grove-explore__explore")` |
| 1e. Unit tests in harness.rs | Done | `expected_mcp_args_coverage`, `expected_explore_args_coverage`, `expected_claude_marker_coverage` |
| 2a. Import `EXPLORE_SERVER_KEY` | Done | Import visible in init.rs usage |
| 2b. `find_explore_binary()` | Done | Lines 597-603 |
| 2c. `write_json_mcp_explore_server()` | Done | Lines 607-638 |
| 2d. `strip_json_mcp_explore_server()` | Done | Lines 640-658 |
| 2e. `write_toml_mcp_explore_server()` | Done | Lines 660-695 |
| 2f. `strip_toml_mcp_explore_server()` (implied) | Done | Lines 698-722 |
| 2g. `write_explore_harness_registration()` | Done | Lines 724-740 |
| 2h. `strip_explore_harness_registration()` | Done | Lines 741-747 |
| 2i. Second reconcile loop | Done | Lines 334-345 — properly iterates all harnesses |
| 2k. `claude_section(McpLlm)` dual-surface | Done | Lines 892-918 — contains `mcp__grove-explore__explore` |
| 2l. `agents_section(McpLlm)` dual-surface | Done | Lines 813-842 |
| 3. Forward migration | Done | Test `mcp_llm_forward_migration_converges_single_to_two_registrations` at line 1648 |
| 4a. Update `assert_mcp_json_consistent` all branches | Done | Lines 1324-1386 — McpLlm asserts both servers, Mcp/Both asserts grove-explore absent, Skill/Grammars asserts both absent |
| 4h. Forward migration test | Done | Line 1648 |
| 4i. Host-content test | Done | Lines 1500-1552 — seeds and asserts strip of both grove and grove-explore |
| 6. Doctor fixtures | Done | `write_explore_mcp_json()` at line 845; `seed_harness_for_mode(McpLlm)` updated |
| AC7. Delete `write_mcp_json_explore` | Done | No grep hits — function eliminated |

### Test Evidence

- `cargo test --release --locked`: **37** CLI integration tests + **54** explore-core unit tests pass
- `cargo clippy --all-targets --workspace --locked -- -D warnings`: clean (no warnings)
- `cargo build --release --locked`: clean (no warnings)

### Architecture Alignment

- Single source of truth for harness constants remains in `core/src/harness.rs`
- `reconcile_harness_for` is the single writer (AC2)
- No new crates, no cargo dependency changes
- No schema changes

### Security Review

- No auth changes, no input validation changes, no injection vectors introduced

## Advisory Notes

1. The `find_explore_binary()` helper is Unix-only (no `.exe` suffix). This is explicitly documented in the code comment and aligns with T04 scope constraints.

2. The doctor's HTTP health-probe checks (`provider_reachable`, `model_served`) were removed from `grove doctor` because `health_probe` now lives in `grove-explore-core`, which `grove-cst` cannot depend on. The probe still fires at `grove-explore serve` startup. Test `provider_unreachable_is_fail` was removed with an appropriate comment explaining the change.

3. The explore config handling in `run()` now converts `ExploreConfig` to `serde_json::Value` via `serde_json::to_value()` when falling back to legacy `explore.json`, since `GroveConfig.explore` is now `Option<Value>` rather than `Option<ExploreConfig>`.
