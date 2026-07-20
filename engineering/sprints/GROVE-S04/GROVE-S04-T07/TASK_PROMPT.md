# GROVE-S04-T07: `doctor` re-keying — explore checks keyed on `grove-explore` registration, not mode

**Sprint:** GROVE-S04
**Estimate:** S
**Pipeline:** default

---

## Objective

Move `doctor`'s explore check group behind the new product boundary: the
checks run when the harness actually registers the `grove-explore` server,
not when `mode == mcp-llm` implies a surface (there is no implied surface
anymore). Structural checks stay as they are, with drift detection updated for
the two-registration layout.

## Acceptance Criteria

1. The explore check group (explore section validates; steering legal;
   provider reachable; model served; allowed_tools recognized; tap/trace info)
   runs iff the project's `.mcp.json` registers the `grove-explore` server —
   not from the declared mode.
2. Structural checks (config loads, legal mode, lock verify,
   registry/grammars, version) are unchanged.
3. The harness-consistency (drift) check understands the two-registration
   layout: `mode: mcp-llm` expects both registrations + two-surface steering;
   other modes expect no `grove-explore` registration. A pre-split
   single-registration layout is flagged (warn/fail with a hint to re-run
   `grove init --as mcp-llm`) — the doctor-side mitigation for the sprint's
   migration risk.
4. `doctor`'s `--explore`/`--standard` force flags are re-keyed or removed
   consistently with T03's flag deletion on `serve` (no orphaned mode-forcing
   semantics); `--json` shape and the exit-code gate are preserved.
5. Workspace green: `cargo test`,
   `cargo clippy --all-targets --workspace --locked -- -D warnings`,
   warning-clean build, files end with a newline.

## Context

Implements item 7 of `SPRINT_REQUIREMENTS.md`; ADR 0004 Stage 2 (doctor
re-keying). Depends on **T04** (the two-registration layout it verifies).
**Carry-over precondition:** S03-T07 (`grove doctor`, `core::doctor`) is
approved but not yet committed — it must be committed before this task starts;
this task edits the same check groups. `doctor` remains read-only
(reports + hints, never mutates).

## Artifacts Involved

- `core/src/doctor.rs` (or the S03-T07 module layout) — check-group keying,
  drift matrix, stale-layout hint
- `cli/src/main.rs` — doctor flag surface consistency
- Unit tests — re-keyed group selection, stale-layout detection, both-mode
  drift rows

## Operational Impact

- **Version bump:** rides the next release train.
- **Regeneration:** none; doctor gains the stale-layout hint that guides users
  through the migration.
- **Security scan:** not required.
