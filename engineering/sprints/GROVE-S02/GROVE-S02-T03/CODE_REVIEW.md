# CODE REVIEW — GROVE-S02-T03 (standalone review)

**Verdict:** Approved

## Scope

Re-review after the validation `revision` verdict, which raised exactly one blocker:
AC-7's `byte_bound_terminates_loop` was vacuous (0 assertions, ~1,420 bytes
accumulated vs the 131,072-byte limit — the byte path was never exercised). All
other ACs were already PASS at validation and Approved at the prior code review.

## Blocker Resolution — AC-7 byte bound (VERIFIED FIXED)

Read the rewritten test at `core/src/explore/agent.rs:550` and the loop it
exercises (`run_explore` @112, `corrective_refusal` @247). Confirmed independently:

1. **Bound is genuinely crossed.** The test scripts a single `ChatResponse`
   carrying 1,500 hallucinated tool calls named `"x"`. Each unknown call routes
   through `corrective_refusal`, whose output for name `"x"` is 91 bytes. The loop
   accumulates `total_tool_bytes += result_bytes` for every call →
   1,500 × 91 ≈ 136,500 bytes > `MAX_TOOL_RESULT_BYTES` (131,072). The bounds
   check `total_tool_bytes >= MAX_TOOL_RESULT_BYTES` fires on turn 1.
2. **Real assertions.** `assert!(answer.truncated)` and
   `assert!(answer.turns < MAX_TURNS)` — the latter proves the *byte* path fired,
   not the turn path (turns == 1). Ok-not-Err is enforced via `.expect(...)`.
3. **No filesystem dependency.** `"x"` is never in the active toolset and is not
   `submit_plan`, so `corrective_refusal` is returned in-process — `dispatch_tool`
   is never reached. Deterministic, hermetic.

Empirically verified:
- `cargo test -p grove-cst --lib explore::` → 39 passed, 0 failed
  (incl. `byte_bound_terminates_loop`, `turn_bound_terminates_loop`).
- `cargo clippy -p grove-cst --lib --all-targets -- -D warnings` → clean (AC-8 holds
  after the test-file edit).

## AC Confirmation (unchanged since prior Approved review, re-spot-checked)

- **AC-1** `run_explore` signature exact; re-exported from `mod.rs`.
- **AC-2/AC-3** Two-level gating intact: inactive tools omitted from the `tools`
  array; `corrective_refusal()` on hallucinated calls; shell allowlist checked
  before dispatch, args passed as a vector (no interpolation).
- **AC-4** Modes-as-data; `BALANCED_RECON_TURNS = 2`; 3-phase Balanced machine.
- **AC-5** Both bounds → `Ok(ExploreAnswer{truncated:true})` — turn path AND byte
  path now both tested.
- **AC-6** `ClientError::Connection` → `ExploreError::ProviderDown`.

## Advisory Notes (non-blocking, carried from prior review — optional)

1. `aggressive_toolset_same_as_standard` remains tautological (asserts a build-time
   equality without constructing `Mode::Aggressive` through `run_explore`).
2. `submit_plan` special-case precedes the `is_in_toolset` gate, so a hallucinated
   `submit_plan` in Standard/Aggressive is honoured rather than refused — benign
   minor AC-3 edge.
3. `balanced_phase_transitions_after_n_recon_turns` under-asserts (checks completion,
   not the narrowing to a submit-only toolset).

None affect correctness or the acceptance criteria; safe to defer to a follow-up.
