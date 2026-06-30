---
requirements:
  reasoning: High
  context: Medium
  speed: Low
audience: subagent
phase: approve
context:
  architecture: true
  prior_summaries: all
  persona: summary
  master_index: false
  diff_mode: false
deps:
  personas: [architect]
  skills: [architect, generic]
  templates: []
  sub_workflows: []
  kb_docs: [architecture/stack.md]
  config_fields: [paths.engineering]
---


# Approve Task
## Iron Laws

- Approve only when the implementation is consistent with the project's architecture and the deployment posture is understood. Architectural sign-off is not a rubber stamp — it is the last point at which cross-cutting concerns can be caught cheaply.
- Read `.forge/personas/architect.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store` (or `node .forge/tools/store-cli.cjs`). Never edit `.forge/store/*.json` directly.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Algorithm

```

0a. Pre-flight Gate Check:
   - **Entity-mode resolution:** read the kickoff arguments. `--task {id}` → `entity_kind = "task"`, `record_id = {id}`. `--bug {id}` → `entity_kind = "bug"`, `record_id = {id}`. All store-cli calls below substitute `{entity_kind}` and `{record_id}` for the literal "task"/{taskId} placeholders.
   - Run: `node .forge/tools/preflight-gate.cjs --phase approve --{entity_kind} {record_id}`
   - Exit 1 (gate failed) → print stderr and HALT. Do not proceed; do not attempt to produce the artifact.
   - Exit 2 (misconfiguration) → print stderr and HALT.
   - Exit 0 → continue.

0b. Pipeline Step Guard (user-invoked state check):
   - If `--force` is present in the invocation arguments, skip this step entirely.
   - If `entity_kind == "bug"`, skip this step entirely (bug state is managed by meta-fix-bug.md).
   - Read current task state:
     `node .forge/tools/store-cli.cjs read task {record_id} --fields status`
   - Extract the `status` field from the JSON output.
   - Allowed states for this phase: `review-approved`.
   - If the current status is NOT in the allowed set:
     Print the following and HALT (do not proceed):
     `× Task {record_id} is in state '{status}' — /forge:review-code (or /forge:validate) must complete first. To run the full pipeline: /forge:run-task {record_id}`

1. Load Context:
   - Read task prompt
   - Read final PLAN.md
   - Read approved CODE_REVIEW.md
   - Read PROGRESS.md

2. Architectural Review:
   - Verify implementation aligns with project architecture
   - Check for cross-cutting concerns (impact on other modules)
   - Assess operational impact (deployment changes, migrations)

3. Sign Off:
   - Write the architect approval via:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"architect-approval", content:"<markdown>" })`
     The markdown content must contain:
     - A canonical verdict line for human readers, on its own line, in this exact form:
       ```
       **Verdict:** [Approved | Revision Required]
       ```
     - Approval status rationale
     - Deployment notes
     - Follow-up items for future sprints
   - The downstream commit-phase preflight gate does NOT read this markdown. **Task mode:** it reads `task.status === "approved"` set in step 4. **Bug mode:** it reads `bug.summaries.approve.verdict === "approved"` set in step 5. The `**Verdict:**` line is a human breadcrumb only.

4. Finalize:
   - Transitions:
     - **Task mode** — Update status: `node .forge/tools/store-cli.cjs update-status task {taskId} status approved`. The status IS the verdict signal for task-mode commit gate (`STATUS_SOURCE` in `read-verdict.cjs`).
     - **Bug mode** — NO status write. The bug remains `in-progress`. The verdict signal travels through `summaries.approve.verdict` written in step 5 below (read by `read-verdict.cjs § BUG_PHASE_VERDICT_SOURCE`). Writing `bug.status` here — especially writing `approved` or `verified` — violates `meta-fix-bug.md § Iron Laws #2` and is the trap that produced the FORGE-BUG-002 regression.
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.

5. Emit Summary Sidecar:
   - Write the approve summary via:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"approve-summary", content:"<JSON>" })`
     The JSON content must have the following shape:
     ```json
     {
       "objective":   "<one sentence — what this approval covered>",
       "findings":    ["<up to 12 bullets, 200 chars each — architectural notes, deployment concerns>"],
       "verdict":     "<approved | revision>",
       "written_at":  "<current ISO 8601 timestamp>",
       "artifact_ref":"ARCHITECT_APPROVAL.md"
     }
     ```
   - Call (task mode) — optional for tasks, since `task.status` is the canonical signal.
     The sidecar path is auto-resolved from the record's `path` — never pass it:
     ```
     node .forge/tools/store-cli.cjs set-summary {taskId} approve
     ```
     Or (bug mode) — REQUIRED for bugs, this is the canonical verdict signal:
     ```
     node .forge/tools/store-cli.cjs set-bug-summary {bugId} approve
     ```
   - In bug mode, if the set-bug-summary call exits non-zero, fix the sidecar JSON and retry. Do not return without a valid summary — the downstream commit gate has no other way to read the approval verdict.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->