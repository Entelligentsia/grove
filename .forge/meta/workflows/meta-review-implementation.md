---
requirements:
  reasoning: High
  context: Medium
  speed: Low
audience: subagent
phase: review-code
context:
  architecture: false
  prior_summaries: delta
  persona: summary
  master_index: false
  diff_mode: true
deps:
  personas: [supervisor]
  skills: [supervisor, generic]
  templates: [CODE_REVIEW_TEMPLATE]
  sub_workflows: []
  kb_docs: [architecture/stack.md, architecture/routing.md]
  config_fields: [commands.test, paths.engineering]
---

# 🌿 Meta-Workflow: Review Implementation

## Purpose

The Supervisor reviews the Engineer's implementation for correctness, quality, and compliance with the approved plan.

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance -->
## Iron Laws

- Evaluate the code against the approved PLAN.md and the original task prompt. Do not accept "it works" as a substitute for "it is correct and maintainable."
- Read `.forge/personas/supervisor.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store`. Never edit `.forge/store/*.json` directly.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Algorithm

```

0a. Pre-flight Gate Check:
   - **Entity-mode resolution:** read the kickoff arguments. `--task {id}` → `entity_kind = "task"`, `record_id = {id}`. `--bug {id}` → `entity_kind = "bug"`, `record_id = {id}`. All store-cli calls below substitute `{entity_kind}` and `{record_id}` for the literal "task"/{taskId} placeholders.
   - Run: `forge_preflight({ phase: "review-code", {entity_kind}: "{record_id}`" })
   - Exit 1 (gate failed) → print stderr and HALT. Do not proceed; do not attempt to produce the artifact.
   - Exit 2 (misconfiguration) → print stderr and HALT.
   - Exit 0 → continue.

0b. Pipeline Step Guard (user-invoked state check):
   - If `--force` is present in the invocation arguments, skip this step entirely.
   - If `entity_kind == "bug"`, skip this step entirely (bug state is managed by meta-fix-bug.md).
   - Read current task state:
     `forge_store({ command: "read", args: ["task", "{record_id}", "--fields"] }) status`
   - Extract the `status` field from the JSON output.
   - Allowed states for this phase: `implemented`, `implementing`.
   - If the current status is NOT in the allowed set:
     Print the following and HALT (do not proceed):
     `× Task {record_id} is in state '{status}' — /forge:implement must complete first. To run the full pipeline: /forge:run-task {record_id}`

1. Read Review Loop Context:
   - Check the spawning prompt for a `### Review Loop Context` block.
   - If present, extract:
     - `Iteration: N of M` — current attempt number and the configured limit
     - `Is final iteration: true/false`
   - If absent (user-invoked, not orchestrated): treat as `iteration 1`, no limit — do
     NOT read any iteration cap from config. The orchestrator owns loop budgets; a human
     standalone re-run is the escape hatch for stuck items (forge-engineering#34).
   - Include `(iteration N of M)` (orchestrated) or `(standalone review)` in the opening line of the `CODE_REVIEW.md` artifact.
   - If this is the final iteration (`N == M`) and the verdict is `Revision Required`,
     append a `### Next Steps` section to the artifact showing:
     ```
     ### Next Steps
     - Force-approve (bypass remaining reviews): `/forge:approve --force {task_id}`
     - Increase iteration limit: edit `config.pipelines.{pipeline}.phases[review-code].maxIterations`
     - Restart from review: `/forge:review-code {task_id}`
     ```

2. Load Context:
   - Read task prompt
   - Read approved PLAN.md
   - Read PROGRESS.md

   **Read mode: diff-first.** Read `git diff $(git merge-base HEAD origin/main)..HEAD -- <files-listed-in-PLAN>` first. Read full source files only when the diff context is insufficient to judge a finding (e.g., the change is an inversion of an invariant defined elsewhere). Do not pre-load full source — tool calls earn their tokens.

3. Review:
   - Verify all plan steps were executed
   - Review code for quality, security, and architecture alignment
   - Verify test evidence in PROGRESS.md is authentic and complete

4. Verdict:
   - Write the code review via forge_artifact:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"code-review", content:"<markdown>" })`
     Use the format:
     **Verdict:** [Approved | Revision Required]
     - If Revision Required: provide numbered, actionable items
     - If Approved: provide any advisory notes
     - See step 1 for iteration header and final-iteration Next Steps requirements.

5. Knowledge Writeback:
   - Update stack-checklist.md if new patterns or pitfalls were discovered

6. Finalize:
   - Transitions:
     - **Task mode** — Update status: `forge_store({ command: "update-status", args: ["task", "{taskId}", "status", "review-approved`"] }) (if Approved) or `... status code-revision-required` (if Revision Required).
     - **Bug mode** — NO status write. The bug remains `in-progress`. The verdict signal travels through `summaries.code_review.verdict` (read by `read-verdict.cjs § BUG_PHASE_VERDICT_SOURCE`), not `bug.status`. Writing `bug.status` here violates `meta-fix-bug.md § Iron Laws #2`.
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.

7. Emit Summary Sidecar:
   - Write the review summary via forge_artifact:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"review-impl-summary", content:"<JSON>" })`
     The JSON shape:
     ```json
     {
       "objective":   "<one sentence — what this review assessed>",
       "findings":    ["<up to 12 bullets, 200 chars each — key issues or confirmations>"],
       "verdict":     "<approved | revision>",
       "written_at":  "<current ISO 8601 timestamp>",
       "artifact_ref":"CODE_REVIEW.md"
     }
     ```
   - Call (task mode):
     `forge_store({ command:"set-summary", args:["{taskId}", "code_review"] })`
     Or (bug mode):
     `forge_store({ command:"set-bug-summary", args:["{bugId}", "code_review"] })`
     `args[1]` is the LITERAL phase key `code_review`, never the record id; `forge_store` has no `entity`/`id`/`phase` field (see `_fragments/store-cli-verbs.md`).
   - If set-summary exits non-zero, `args[1]` was wrong — fix it to `code_review` and retry. Do not return without a valid summary; the orchestrator halts as "verdict missing" if `summaries.code_review` is absent.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Generation Instructions

- **Workflow Structure:** The generated `review_implementation.md` must follow the strict "Algorithm" block format.
- **Markers (required by `/forge:run-task` kickoff shim):** Generated workflow MUST include the "Iron Laws" section, the "Store-Write Verification" section, the literal `forge_store` token, and the `.forge/personas/supervisor.md` persona path. Missing any → kickoff shim refuses to dispatch.
- **Verdict Detection:** The generated workflow MUST enforce the strict `**Verdict:** [Approved | Revision Required]` format.
- **Context Isolation:** Forbid inline execution of complex code review logic; use the `Agent` tool for sub-tasks.
- **Project Specifics:**
  - Embed project-specific code quality standards and linting rules.
- **Token Reporting:** See `_fragments/finalize.md` — wire via `file_ref:`.
- **Diff-mode:** Generated workflow MUST include the diff-first read mode instruction (see PLAN.md A6).
- **Event Emission:** Ensure the "complete" event includes the `eventId` passed by the orchestrator.
