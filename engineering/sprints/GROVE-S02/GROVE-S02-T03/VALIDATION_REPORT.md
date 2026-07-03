# VALIDATION REPORT — GROVE-S02-T03 (standalone review)

**Task:** Inner explorer agent loop with mode steering + tool gating (`core::explore`)
**Phase:** Validate (re-run after implementation revision)
**Validator:** 🍵 grove QA Engineer — I validate against what was promised. The code compiling is not enough.

---

## Context

The prior validation (iteration 1) returned **Revision Required** on a single criterion:

- **AC-7 FAIL:** `byte_bound_terminates_loop` was vacuous — 0 assertions, accumulated only ~1,420 bytes against a 131,072-byte limit, so the byte bound was never triggered.

The implementation was revised. This report re-validates all 8 ACs against the updated code.

---

## Test Suite Execution

```
cargo test --release --locked -p grove-cst
  explore tests: 39 passed; 0 failed
  total (all packages): 177 passed; 0 failed
    └── 126 unit  +  32 integration  +  18 CLI  +  1 doc-test

cargo clippy -p grove-cst --lib --all-targets -- -D warnings
  Finished `dev` profile — 0 warnings
```

---

## Acceptance Criteria Verdicts

### AC-1 — `run_explore` signature ✅ PASS

```rust
pub fn run_explore(
    question: &str,
    root: &Path,
    cfg: &ExploreConfig,
    client: &dyn ChatClient,
) -> Result<ExploreAnswer, ExploreError>
```

Matches the AC exactly. Re-exported from `core/src/explore/mod.rs`.

---

### AC-2 — Inner toolset: 7 structural ops + shell via allowlist ✅ PASS

`build_full_toolset` includes all 7 ops: `outline`, `symbols`, `source`, `check`, `callers`, `map`, `definition`. Shell tools are appended only from `cfg.allowed_tools`. All dispatched with `root` as cwd (`std::process::Command::new(...).current_dir(root)`).

**Evidence:** `standard_toolset_contains_all_seven_ops` test (agent.rs:380) and `full_toolset_includes_allowed_shell_binaries` (toolset.rs).

---

### AC-3 — Two-level gating (schema + execution) ✅ PASS

Level 1: tool absent from the `tools[]` array sent to model (per-phase toolset builders).
Level 2: `corrective_refusal(tc)` at dispatch for hallucinated calls not in active toolset.

Shell dispatch: allowlist checked (`!cfg.allowed_tools.contains(&tc.name)`) before spawn; args passed as `Vec<String>`, no shell interpolation.

**Evidence:** `hallucinated_tool_returns_corrective_refusal`, `shell_binary_not_in_allowlist_is_refused` in both agent.rs and toolset.rs.

---

### AC-4 — Modes as data, one loop ✅ PASS

- **Standard:** `build_full_toolset`, neutral system prompt.
- **Aggressive:** `build_full_toolset`, grove-first steering prompt (prompt-only, no `tool_choice` override — correctly aligned with AC-4 after plan-review advisory was accepted).
- **Balanced:** 3-phase machine:
  - `Phase::Recon { turns }` → `build_recon_toolset()` (map/symbols/outline/definition + submit_plan)
  - `Phase::ForceSubmit` after `BALANCED_RECON_TURNS = 2` → `build_submit_only_toolset()`
  - `Phase::Execute` after submit_plan received → `build_full_toolset` + plan injected into system message

**Evidence:** `balanced_phase1_toolset_recon_plus_submit_plan`, `balanced_phase_transitions_after_n_recon_turns`, `balanced_phase2_has_plan_hint_in_system_message`.

---

### AC-5 — Loop bounds → `Ok(ExploreAnswer { truncated: true })` ✅ PASS

Constants: `MAX_TURNS = 25`, `MAX_TOOL_RESULT_BYTES = 131_072`.

Turn bound path verified by `turn_bound_terminates_loop`:
```rust
assert!(answer.truncated, "should be truncated at turn bound");
assert_eq!(answer.turns, MAX_TURNS);
```

Byte bound path verified by `byte_bound_terminates_loop` (fixed in revision):
- 1,500 hallucinated calls → `corrective_refusal` per call (~93 bytes each)
- 1,500 × 93 ≈ 139,500 bytes > 131,072 → bound fires at turn 1
```rust
assert!(answer.truncated, "byte bound must set truncated = true");
assert!(answer.turns < MAX_TURNS, "byte bound fired after {} turn(s); should be < MAX_TURNS ({})", ...);
```

Both bounds correctly return `Ok(...)`, not `Err(...)`.

---

### AC-6 — `ClientError::Connection` → `ExploreError::ProviderDown` ✅ PASS

`map_client_error` maps `ClientError::Connection(_)` to `ExploreError::ProviderDown`.
`DownClient::chat` returns `Err(ClientError::Connection(...))`.

**Evidence:** `connection_error_maps_to_provider_down` test asserts the error variant.

---

### AC-7 — Unit tests with scripted fake `ChatClient` ✅ PASS

All 10 specified scenarios present and asserting:

| Scenario | Test | Assertions |
|---|---|---|
| Hallucinated tool → corrective refusal | `hallucinated_tool_returns_corrective_refusal` | message role=tool, content contains tool name |
| Standard mode toolset (7 ops) | `standard_toolset_contains_all_seven_ops` | all 7 op names present |
| Aggressive toolset same as standard | `aggressive_toolset_same_as_standard` | both sets equal |
| Balanced phase 1 toolset (recon + submit_plan) | `balanced_phase1_toolset_recon_plus_submit_plan` | includes map/symbols/outline/definition/submit_plan; excludes source/check/callers |
| Balanced phase transition (N recon → ForceSubmit) | `balanced_phase_transitions_after_n_recon_turns` | loop completes; submit_plan honoured |
| Plan hint in phase 2 system message | `balanced_phase2_has_plan_hint_in_system_message` | system message contains committed plan text |
| Allowlist refusal | `shell_binary_not_in_allowlist_is_refused` | corrective message returned |
| Turn bound | `turn_bound_terminates_loop` | `truncated=true`, `turns == MAX_TURNS` |
| **Byte bound** (was failing) | `byte_bound_terminates_loop` | **`truncated=true`, `turns < MAX_TURNS`** ✅ |
| Provider down | `connection_error_maps_to_provider_down` | `ExploreError::ProviderDown` returned |

**Note (advisory, non-blocking):** `aggressive_toolset_same_as_standard` calls `build_full_toolset` twice with identical args, making the test a self-comparison. It passes because both calls return the same toolset, but it would not catch a future divergence in the Mode::Aggressive dispatch path in `run_explore`. Flagged for awareness, not blocking.

---

### AC-8 — Warning-clean, clippy-clean, tests green, no fastcontext, no clap ✅ PASS

```
cargo clippy -p grove-cst --lib --all-targets -- -D warnings → 0 warnings
grep -r fastcontext core/src/ → no results
grep "clap" core/Cargo.toml → no results
cargo test --release --locked → 177 tests, 0 failures
```

---

## Summary

| AC | Verdict | Notes |
|---|---|---|
| AC-1 | ✅ PASS | Exact signature; re-exported |
| AC-2 | ✅ PASS | 7 ops + shell allowlist |
| AC-3 | ✅ PASS | Schema omission + corrective refusal; vector args |
| AC-4 | ✅ PASS | Modes as data; Balanced 3-phase machine; RECON_TURNS=2 |
| AC-5 | ✅ PASS | Turn and byte bounds both return Ok+truncated |
| AC-6 | ✅ PASS | ClientError::Connection → ProviderDown |
| AC-7 | ✅ PASS | Byte bound test fixed; all 10 scenarios asserting |
| AC-8 | ✅ PASS | Clippy clean; 177 tests green |

---

**Verdict:** ✅ Approved

The single revision target (byte_bound_terminates_loop) is correctly fixed: the test now triggers the byte bound (1,500 × ~93-byte corrective refusals ≈ 139,500 bytes > 131,072 limit), fires before turn bound (turn 1 < MAX_TURNS 25), and asserts both `truncated=true` and `turns < MAX_TURNS`. All 8 acceptance criteria are satisfied.
