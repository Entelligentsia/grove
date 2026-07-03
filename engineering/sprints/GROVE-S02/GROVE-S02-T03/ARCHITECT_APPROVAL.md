# ARCHITECT APPROVAL — GROVE-S02-T03

Inner explorer agent loop with mode steering + harness-enforced tool gating (`core::explore`).

**Verdict:** Approved

## Rationale

The implementation is architecturally coherent with the grove core and the T01/T02
surfaces it depends on:

- **Correct crate boundary.** `run_explore` lives in `core::explore` and calls the
  7 structural ops in-process via `grove_core::ops::…`. Shell tools are dispatched
  through `std::process::Command` with an args vector. The core crate stays
  clap-free (AC-8) — clap remains confined to the CLI crate per `stack.md`, so the
  library/CLI split is preserved.
- **Security posture sound.** Two-level gating (absent from the `tools[]` array
  sent to the model + corrective tool-result on hallucinated calls) plus an
  allowlist check *before* spawn, args as `Vec<String>`, no shell interpolation
  (AC-3). This is the right defensive shape for a model-driven agent loop.
- **Modes-as-data, one loop.** Standard / Aggressive / Balanced expressed as
  steering data over a single loop; the Aggressive `tool_choice` hint was correctly
  dropped in favour of prompt-only steering. The Balanced 3-phase machine
  (`BALANCED_RECON_TURNS = 2`) matches the approved PLAN and AC-4.
- **Bounds are graceful.** `MAX_TURNS = 25` and `MAX_TOOL_RESULT_BYTES = 131072`
  both terminate with `Ok(ExploreAnswer{truncated:true})`, not an error (AC-5) — the
  right contract for a best-effort explorer.
- **Error propagation aligned with the D3 recovery model.** `ClientError::Connection`
  → `ExploreError::ProviderDown` (AC-6), leaving T04 to map it to a recoverable
  `isError` so the MCP server stays up.

All 8 ACs validated PASS; code review Approved; validation Approved with 0 regressions
across 177 tests. The AC-7 byte-bound test that previously failed was genuinely
rewritten to exceed the 128 KiB bound and now asserts `truncated=true` with
`turns < MAX_TURNS`.

## Deployment Notes

- **Version bump:** not required (sprint-internal; no public contract change).
- **Regeneration:** none.
- **Backward compat:** additive — the 7 structural ops' contracts are untouched;
  no new Cargo dependencies introduced.
- **Operational impact:** none at deploy time. The loop is invoked behind the
  forthcoming T04 `explore` MCP call; no new binaries, migrations, or config keys.

## Follow-up Items (future sprints)

1. **Strengthen tautological tests** — `aggressive_toolset_same_as_standard` is a
   self-comparison and never constructs `Mode::Aggressive`; it would not catch
   dispatch-path divergence. Tighten to actually build the Aggressive toolset.
2. **submit_plan gating edge** — `submit_plan` special-case precedes the
   `is_in_toolset` gate, so a hallucinated `submit_plan` in Standard/Aggressive is
   honoured rather than refused. Benign today; worth closing for strict AC-3 purity.
3. **balanced_phase_transitions under-assertion** — extend to assert the phase-1
   toolset narrows to `submit_plan`-only after recon, not just loop completion.

None of these are blocking; they are test-hardening items to schedule when T04
lands and the explore path gains an integration surface.
