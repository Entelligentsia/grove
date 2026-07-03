# PROGRESS — GROVE-S02-T03 (Implement revision)

## Summary of Changes

**Objective**: Fix AC-7 validation failure: `byte_bound_terminates_loop` was a vacuous test that made zero assertions and never crossed the `MAX_TOOL_RESULT_BYTES` (131,072 byte) threshold.

### Root Cause

The original test used a `LargeResultClient` that returned at most 20 `outline` calls on a nonexistent file. Each call produced ~71 bytes of error output → ~1,420 bytes total, far below the 131,072 byte limit. The test always passed regardless of whether the byte-bound code path worked, violating the QA iron law: *"A test that always passes regardless of behaviour is not a test."*

### Fix Applied

**`core/src/explore/agent.rs` — `byte_bound_terminates_loop` test rewritten**

Strategy: construct one `ChatResponse` containing **1,500 tool calls** to a hallucinated tool `"x"`. The agent issues a `corrective_refusal` (~93 bytes) for each call without touching the filesystem.

- 1,500 × 93 ≈ **139,500 bytes** > `MAX_TOOL_RESULT_BYTES` = 131,072 ✓
- Byte bound fires at end of turn 1 (before `MAX_TURNS` = 25)

Added assertions:
- `assert!(answer.truncated)` — byte bound sets `truncated = true`
- `assert!(answer.turns < MAX_TURNS)` — confirms byte path (not turn path) fired

### Test Evidence

```
running 1 test
test explore::agent::tests::byte_bound_terminates_loop ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured
```

Full suite — all crates:
```
test result: ok. 126 passed; 0 failed; 0 ignored; 0 measured  (grove-cst unit)
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured   (grove-cst integration)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured   (grove CLI)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured    (doc-tests)
```

Clippy (`-p grove-cst --lib --all-targets -- -D warnings`): **clean** (0 errors, 0 warnings).

### Acceptance Criteria Status

| AC | Status | Notes |
|----|--------|-------|
| AC-1 | ✅ PASS | `run_explore` signature exact; re-exported from `mod.rs` |
| AC-2 | ✅ PASS | All 7 structural ops + shell allowlist enforced |
| AC-3 | ✅ PASS | Two-level gating (toolset array + `corrective_refusal`) |
| AC-4 | ✅ PASS | `BALANCED_RECON_TURNS=2`; 3-phase Balanced state machine |
| AC-5 | ✅ PASS | Turn bound tested; **byte bound now tested** |
| AC-6 | ✅ PASS | `ClientError::Connection` → `ExploreError::ProviderDown` |
| AC-7 | ✅ PASS | `byte_bound_terminates_loop` now asserts `truncated=true` and `turns < MAX_TURNS` |
| AC-8 | ✅ PASS | Clippy `-D warnings` clean; no fastcontext; no clap |

## Files Changed

- `core/src/explore/agent.rs` — rewrote `byte_bound_terminates_loop` test (lines 541–595)
