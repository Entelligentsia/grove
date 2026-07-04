# ARCHITECT_APPROVAL — GROVE-S03-T07: `grove doctor`

**Verdict:** Approved

## Approval Rationale

The implementation is architecturally coherent with the grove single-binary,
read-only-diagnostic posture. Key alignments:

- **Constant-ownership resolved cleanly.** The new `core/src/harness.rs` becomes
  the single source of truth for sentinel strings (`GROVE_START/END`,
  `MCP_SERVER_KEY`) and mode→content tables. `cli/src/init.rs` migrates to import
  them via `grove_core::harness` — no core→cli dependency cycle, no duplication.
  This is the correct layering: shared invariants live in the core crate,
  consumed by the CLI.
- **`core::doctor::diagnose` is pure and read-only.** It composes existing core
  primitives (`active_mode`, `verify_lock` from T06, `ExploreConfig::validate`,
  health probes) rather than re-implementing them. Diagnostics never mutate — the
  right contract for a `doctor` verb.
- **Reuse over reinvention.** lock_integrity delegates to T06's `verify_lock`;
  explore checks gate on `Mode::McpLlm` and surface `HealthError` hints verbatim.
  No shadow logic that could drift from the real init/serve behavior.
- **Exit-code + `--json` dual-format** keyed off `Report::ok()` (all `!Fail`) gives
  a stable CI-consumable contract; the warn-only-exits-zero boundary is explicitly
  tested.

Independent code review isolated the true T07 footprint via `git diff HEAD`
(distinguishing it from committed T04 `reconcile_harness` work), verified clippy
`-D warnings` clean and the full test suite green (303 tests, 0 failed), and
confirmed the harness/drift/explore tests discriminate real Ok/Warn/Fail rather
than tautologies. Validation independently confirmed all 10 ACs.

## Deployment Notes

- **Version bump required at release** — this adds a new user-facing verb
  (`grove doctor`). Per the task's Operational Impact, this is the only release
  gate.
- **No regeneration, no migration, no security scan** — the command is purely
  additive and read-only. It touches no persisted state, no lockfile writes, no
  network mutation. Zero operational risk to existing deployments.
- Single-binary distribution model is unaffected: the verb ships inside the same
  static binary across all five release targets.

## Follow-up Items (future sprints)

1. **Effective-mode echo in human header** (advisory #1): `Report.mode` shows the
   *declared* config mode while harness checks validate against
   `active_mode(root, force)`. Correct-as-specified for AC1, but a future polish
   could echo the forced/effective mode when `--explore`/`--standard` override the
   declared mode, to avoid operator confusion.
2. **Named steering-legality check** (advisory #2): AC5's "steering legal" is
   covered transitively by `ExploreConfig::validate()`. A dedicated named check
   would improve traceability of doctor output, but is not required.
3. **Cosmetic**: double blank line left in `init.rs` after removing the local
   `MCP_SERVER_KEY` const (rustfmt-neutral; clean up opportunistically).

None of these block commit.
