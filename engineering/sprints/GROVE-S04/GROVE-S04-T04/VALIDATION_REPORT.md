# VALIDATION_REPORT — GROVE-S04-T04 (iteration 1 of 3)

**Verdict:** Approved

## Acceptance Criteria Verification

### AC1 — Both servers registered + dual steering ✅
- `EXPLORE_SERVER_KEY` constant added to `core/src/harness.rs:33`
- `expected_explore_args(Mode::McpLlm)` returns `Some(&[])` for the grove-explore binary
- Second reconcile loop (lines 334-345) writes/strips grove-explore for all harnesses
- Steering content contains `mcp__grove-explore__explore` and names both surfaces with recommended flow

### AC2 — Single reconcile_harness writer ✅
- `reconcile_harness_for` owns all writes + strip logic (both structural and explore servers)
- Leaving `mcp-llm` strips the grove-explore registration via `strip_explore_harness_registration`
- `assert_mcp_json_consistent` updated for all three branches (McpLlm, Mcp/Both, Skill/Grammars) to verify grove-explore presence/absence

### AC3 — Forward migration ✅
- Test `mcp_llm_forward_migration_converges_single_to_two_registrations` exists (line 1648)
- Seeds old `["serve", "--explore"]` layout and verifies it converges to two registrations
- Test passes: old `--explore` arg removed, `grove` key updated to `["serve"]`, `grove-explore` key added

### AC4 — Gate test (b) — full transition matrix ✅
- `assert_mcp_json_consistent` verifies:
  - McpLlm: grove args = `["serve"]`, grove-explore present
  - Mcp/Both: grove args = `["serve"]`, grove-explore absent
  - Skill/Grammars: both grove and grove-explore absent
- All 20 ordered A→B mode transitions covered by existing matrix test

### AC5 — Mode::LEGAL unchanged ✅
- No modes added or removed; `Mode` enum unchanged

### AC6 — Host content preserved ✅
- `reconcile_harness_preserves_host_content` test updated and passes
- Only sentinel-delimited grove-authored blocks and grove's own server entries are touched

### AC7 — Workspace green ✅
- `cargo test --release --locked`: 92 tests pass (37 CLI + 54 explore-core + 1 doc-test)
- `cargo clippy --all-targets --workspace --locked -- -D warnings`: clean (no warnings)
- Old `write_mcp_json_explore` function removed (no dead code)

## Test Evidence

```
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Clippy: Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

## Conclusion

All 7 acceptance criteria are satisfied. The implementation correctly:
1. Registers both MCP servers for mcp-llm mode
2. Maintains the single reconcile_harness writer pattern
3. Provides forward migration from pre-split layouts
4. Extends the transition matrix test coverage
5. Preserves host-authored content
6. Passes all tests and lint checks
