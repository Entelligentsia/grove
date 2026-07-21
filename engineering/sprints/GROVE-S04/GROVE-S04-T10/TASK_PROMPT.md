# GROVE-S04-T10: Gate test (a) — full sidebench re-run on the split binary (user-supervised rig)

**Sprint:** GROVE-S04
**Estimate:** M
**Pipeline:** default

---

## Objective

Prove ADR 0004's "no inner-loop behavior change" guarantee end-to-end (sprint
gate test a, intake decision D1): re-run the 347-case holdout through the
fastcontext sidebench against the **split** `grove-explore` binary on the
reference rig, and show the score matches the `base-q4-v2-hf` reference (80.6)
with no regression attributable to the split.

## Acceptance Criteria

1. The sidebench (`is-grep-enough/studies/fastcontext-sidebench/`) targets the
   split `grove-explore` binary: whatever harness-pointing changes the bench
   needs (binary path/invocation config) are made in that repo and noted here;
   the bench's question set, scoring, and rig config are otherwise untouched.
2. The run uses the pinned reference combination: llama.cpp serving
   `base-q4-v2-hf`, the same 347-case holdout, the same sampling settings —
   variance controls per the study's conventions.
3. The score matches the 80.6 reference (within the study's accepted
   run-to-run variance, per its conventions); any delta is investigated and
   either attributed to known variance or treated as a split regression
   (blocking — the sprint does not close over an unexplained regression).
4. The run is recorded in the study's evidence per its conventions
   (`evidence/`/`reports/` + the experiment registry in `grove-explore-model`
   if that is where reference runs are logged); this task's PROGRESS.md links
   the recorded evidence.
5. Roles per D1: the user runs/supervises the local rig; this task prepares
   the split binary, the bench pointing, and the comparison/write-up.

## Context

Implements item 10 of `SPRINT_REQUIREMENTS.md` (D1; ADR 0004 gate test a and
scope boundary "the sidebench numbers must not move"). Depends on **T02**
(split binary) and **T05** (final binary shape incl. tap/trace paths the bench
may exercise). T01's byte-identity diff is the code-level backstop if the rig
is unavailable — but per D1 the full re-run is the sprint's acceptance bar,
not the backstop. Scheduled at wave 4 so a rig problem surfaces with docs
(wave 5) as the only remaining work. Cross-repo coordination: bench changes
commit in `is-grep-enough`, not here.

## Artifacts Involved

- Split `grove-explore` binary (release build from this repo)
- `is-grep-enough/studies/fastcontext-sidebench/` — bench pointing config +
  evidence/report of the run (committed in that repo)
- This task's PROGRESS.md — comparison table + evidence links

## Operational Impact

- **Version bump:** none.
- **Regeneration:** none.
- **Security scan:** not required.
