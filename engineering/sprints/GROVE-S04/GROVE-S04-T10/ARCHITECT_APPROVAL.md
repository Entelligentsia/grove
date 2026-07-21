# ARCHITECT APPROVAL — GROVE-S04-T10: Gate test (a) — full sidebench re-run on the split binary (user-supervised rig)

**Verdict:** Approved

## Scope of Approval

This approval covers the task's **unattended-achievable scope**: release binary
rebuild, is-grep-enough harness-pointing patch preparation, reference-combination
pinning, and user-supervised rig handoff. It does **not** close sprint
requirement #10 / D1 — the actual 347-case sidebench re-run (AC3 + AC4) remains
open as a blocking follow-up.

## Architectural Review

### Consistency with project architecture

- **ADR 0004 stages 1-2 (split-binary):** The split is correctly reflected.
  `grove serve --explore` is a hard error (verified: exit 1, correct replacement
  message). `grove-explore serve` is the default subcommand and preserves the
  MCP server key `grove` and tool `explore` (`mcp__grove__explore`). This means
  the harness's `--allowedTools` filter continues to resolve the tool — the
  false-pass risk identified in plan-review is addressed in the patch.
- **Zero source regression surface:** `git diff --stat HEAD -- cli/ explore/
  grove-explore/ core/` is empty. T10 is a mechanical rebuild + handoff task;
  T01/T02/T03/T05/T09 already shipped the split. No architectural surface was
  touched.
- **Byte-identity claim (ADR 0004):** Prompt byte-identity cross-checked
  (`explore_v2.system.md` == `explore-v2.system.txt`, 1818 B both repos). The
  "no inner-loop behavior change" guarantee has a code-level backstop.

### Cross-cutting concerns

- **Cross-repo boundary:** The harness-fix patch is saved as an artifact
  (`harness-fix.patch`) in this repo, not applied or committed in
  is-grep-enough. The patch applies cleanly (`git apply --check` passes). This
  respects the task's cross-repo coordination note.
- **Distribution channels:** No version bump, no new release channel, no
  distribution change. Binaries are local release builds for the gate test.
  Consistent with the sprint's "stage 3 is out of scope" boundary.
- **Config compatibility:** `.grove/config.json[explore]` is opaque
  `Option<serde_json::Value>` (core/src/config.rs:93); `grove-explore serve_main`
  reads it identically to the old path. No harness config-shape changes needed
  beyond the server launch command.

### Operational impact

- **Deployment:** None. This task produces no deployable artifact — it rebuilds
  local release binaries and prepares a cross-repo patch for a user-supervised
  evaluation rig.
- **Test/lint:** `cargo test --release --locked` → 344 passed, 0 failed.
  `cargo clippy --all-targets --workspace -- -D warnings` → clean. Both
  independently re-verified by code-review and validation phases.

### D1 role boundary

The task honored the D1 role split: the user runs/supervises the local rig; the
task prepares the split binary, the bench pointing, and the comparison/write-up.
A genuine human checkpoint (`source=user answered=true`) chose to defer the rig
run to a follow-up. This was not silently assumed — the scope question was
explicitly surfaced and answered by a human.

## Acceptance Criteria Assessment

| AC | Status | Notes |
|----|--------|-------|
| AC1: sidebench targets split grove-explore | ✓ PASS | Harness patch prepared: binary repointed, args swapped, MCP key preserved |
| AC2: pinned reference combination | ✓ PASS (precondition) | llama.cpp + base-q4-v2-hf + 347-case + Q4_K_M documented; not yet executed |
| AC3: score matches 80.6 | ✗ NOT YET SATISFIED | No run occurred; no pass/fail claim fabricated |
| AC4: run recorded in evidence | ✗ NOT YET SATISFIED | Nothing to record; no run occurred |
| AC5: D1 roles honored | ✓ PASS | Genuine human checkpoint deferred the rig run |

AC3 and AC4 are **blocking items for sprint closure**, not implementation
defects. The task's unattended scope is complete; the rig execution is
user-supervised by design.

## Deployment Notes

No deployment impact. This task produces local release binaries and a cross-repo
patch artifact only. No version bump, no distribution change, no migration.

## Follow-up Items

1. **BLOCKING — 347-case sidebench re-run (sprint closure gate):** The user must
   run the pinned reference combination (llama.cpp serving `base-q4-v2-hf`,
   347-case holdout) against the split `grove-explore` binary via the corrected
   `run-grove-explore.sh` harness. The score must match 80.6 within the study's
   accepted run-to-run variance. The run must be recorded in the study's evidence
   per its conventions. **GROVE-S04 cannot close until this lands.**

2. **Harness patch application (is-grep-enough):** The prepared
   `harness-fix.patch` must be reviewed, applied, and committed in the
   is-grep-enough repo by the user before the rig run. The patch is minimal
   (binary path + args swap) and applies cleanly.

3. **Sprint closure check:** Before marking GROVE-S04 complete, verify that AC3
   and AC4 are satisfied with recorded evidence. The sprint's risk register
   (item: "sidebench re-run unavailable or noisy at sprint close") should be
   consulted if the run cannot land.