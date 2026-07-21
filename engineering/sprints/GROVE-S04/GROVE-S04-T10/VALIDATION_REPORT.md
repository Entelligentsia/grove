# VALIDATION REPORT — GROVE-S04-T10 (standalone review)

**Verdict:** Approved

## Scope note (read first)

This task's title is "Gate test (a) — full sidebench re-run on the split
binary (user-supervised rig)". Per intake decision **D1**, the actual rig
run is inherently a human-supervised action outside the unattended agent
loop — the agent's job is to prepare the split binary, fix the bench's
harness pointing, pin the reference combination, and hand off exact
commands; the user then runs the 347-case re-run and the score comparison
happens after that. A genuine human checkpoint (`source=user
answered=true`) confirmed deferring the actual rig run to a follow-up
rather than running it in this session.

Because of that, this report validates two different things separately:
1. **This task's own deliverable** (prep + harness fix + pinned handoff +
   honest non-fabricated reporting) — independently re-verified below.
2. **Sprint requirement #10 / D1's literal bar** ("score matches 80.6",
   "run recorded in evidence") — **NOT YET satisfied**, because no run has
   occurred. This is not a defect to send back for revision (no code/prep
   work would fix it — it requires the human to execute the rig), so it is
   recorded here as an **open blocking item for sprint closure**, not as a
   task-level failure. Anyone reviewing sprint-level completion (e.g.
   sprint-completion-review) MUST treat AC2–AC4 below as unmet until the
   follow-up run lands.

## Acceptance Criteria (from TASK_PROMPT.md)

1. **Sidebench targets split `grove-explore` binary; harness-pointing
   change made/noted, bench question set/scoring/rig config untouched.**
   → **PASS.** `harness-fix.patch` re-inspected directly:
   - `GROVE=` var repointed from `target/release/grove` to
     `target/release/grove-explore`.
   - MCP launch args changed from `["serve", "--explore", "$clone"]` to
     `["serve", "$clone"]` (grove-explore's default subcommand — matches
     independently-confirmed CLI behavior, see Test Evidence).
   - MCP server key left as `"grove"` (preserves `mcp__grove__explore`
     tool resolution / `--allowedTools` filter — the exact regression the
     plan-review and code-review advisories flagged). Confirmed in the
     diff hunk itself, plus an explanatory comment added to the script.
   - No other lines in the patch touch question set, scoring, or rig
     config — diff is a minimal 3-hunk change limited to the MCP launch
     block and one header comment.
   - Patch is prepared as a file in this task's artifact dir, **not**
     applied/committed — correct per the cross-repo boundary (the
     `is-grep-enough` checkout is not present in this environment, so
     `git apply --check` could not be independently re-run here; this is
     consistent with the task never claiming to have applied it).

2. **Run uses pinned reference combination (llama.cpp, `base-q4-v2-hf`,
   347-case holdout, same sampling settings).**
   → **Documentation PASS, execution NOT YET DONE.** PROGRESS.md's Handoff
   section gives exact commands referencing the `base-q4-v2-hf` registry
   entry; no run has occurred, so "the run uses..." is not yet true as a
   completed fact — it's a correctly-prepared precondition.

3. **Score matches 80.6 reference; any delta investigated/explained;
   unexplained regression is blocking.**
   → **NOT YET SATISFIED — no run occurred.** PROGRESS.md correctly makes
   no pass/fail claim against 80.6 (verified: no comparison table, no
   fabricated score anywhere in the artifact). This is honest and correct
   behavior for an unattended phase that was told to defer, but the
   criterion itself remains open. Flagged as a **blocking open item for
   sprint closure**, not a task defect.

4. **Run recorded in the study's evidence per its conventions; PROGRESS.md
   links the recorded evidence.**
   → **NOT YET SATISFIED — nothing to record**, no run occurred. Same
   status as AC3: correctly not fabricated, but open.

5. **Roles per D1: user runs/supervises the rig; this task prepares the
   split binary, bench pointing, and comparison/write-up.**
   → **PASS.** The checkpoint transcript in PROGRESS.md shows a genuine
   `forge_ask_user`-class checkpoint with three options (run via
   grove-explore-model harness / run via fastcontext-sidebench harness /
   defer), answered by a human (`source=user answered=true`) choosing
   defer. This is exactly the D1 role split working as intended, not a
   silent assumption.

## Independent re-verification (this phase)

- **Release binaries current:** `target/release/grove` → `grove 0.4.1`;
  `target/release/grove-explore` → `grove-explore 0.4.1`. Both run.
- **`grove serve --explore` hard error confirmed:** exit 1, message points
  to `grove-explore serve` for the LLM-delegating surface — matches the
  harness patch's replacement command exactly.
- **No grove source changed:** `git diff --stat HEAD -- cli/ explore/
  grove-explore/ core/` → empty, independently re-run. Matches the
  implementation/code-review claim; this task is legitimately a
  build+prep task with zero source risk.
- **`harness-fix.patch` re-read directly** (not just trusted from
  PROGRESS.md prose) — content matches what PROGRESS.md and CODE_REVIEW.md
  describe; both plan-review advisories (preserve `grove` server key;
  repoint the `GROVE` variable) are addressed in the actual diff, not just
  claimed.
- **Full test suite independently re-run:** `cargo test --release
  --locked` → 344 passed, 0 failed (139+65+43+42+54+1 across
  workspace crates + doc-tests) — matches PROGRESS.md's reported count
  exactly, re-derived from a fresh run rather than trusted from prose.
- **Lint independently re-run:** `cargo clippy --all-targets --workspace
  -- -D warnings` → clean, no warnings.
- **`is-grep-enough` checkout absent from this environment** — the
  cross-repo patch-apply claim (`git apply --check` passes) could not be
  re-executed here; this is an accepted limitation of the sandbox, not a
  gap in the implementation (the repo boundary is intentional per the
  task's cross-repo coordination note).

## Regression check

No grove source changed (confirmed above), so no regression surface
exists beyond the test/lint reruns already performed, both clean.

## Test quality / absence-of-test check

No test can exist for AC2–AC4 within this repo's test suite — they are
external, rig-dependent, human-supervised facts by design (D1). Their
absence here is correctly not disguised as a pass; PROGRESS.md's own
"Follow-up" section and this report both surface it as open.

## Conclusion

This task's own, unattended-achievable scope (release binaries current,
correct and minimal harness-pointing patch, pinned reference combination
documented, honest non-fabricated reporting, genuine human checkpoint
honoring the D1 role split, zero source regression, clean test/lint) is
**complete and independently re-verified — Approved.**

The literal sprint-requirement #10 / D1 bar (score matches 80.6, run
recorded in evidence) is **not yet met** and is **not** being waved
through — it is explicitly carried forward as an open, blocking follow-up
that must land before GROVE-S04 can be considered closed. This is
consistent with — and independently re-confirms — the advisory already
raised in CODE_REVIEW.md.
