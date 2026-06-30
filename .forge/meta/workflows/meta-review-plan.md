---
requirements:
  reasoning: High
  context: Medium
  speed: Low
audience: subagent
phase: review-plan
context:
  architecture: false
  prior_summaries: delta
  persona: summary
  master_index: false
  diff_mode: false
deps:
  personas: [supervisor]
  skills: [supervisor, generic]
  templates: [PLAN_REVIEW_TEMPLATE]
  sub_workflows: []
  kb_docs: [architecture/stack.md]
  config_fields: [paths.engineering]
---

# 🌿 Meta-Workflow: Review Plan

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance -->
## Iron Laws

- Evaluate the plan against what the task actually requires, not against what the plan claims to deliver. Plans routinely understate complexity, omit edge cases, or skip security steps. Your job is adversarial review, not approval.
- Read `.forge/personas/supervisor.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store`. Never edit `.forge/store/*.json` directly.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Algorithm

```

0a. Pre-flight Gate Check:
   - **Entity-mode resolution:** read the kickoff arguments. `--task {id}` → `entity_kind = "task"`, `record_id = {id}`. `--bug {id}` → `entity_kind = "bug"`, `record_id = {id}`. All store-cli calls below substitute `{entity_kind}` and `{record_id}` for the literal "task"/{taskId} placeholders.
   - Run: `forge_preflight({ phase: "review-plan", {entity_kind}: "{record_id}`" })
   - Exit 1 (gate failed) → print stderr and HALT. Do not proceed; do not attempt to produce the artifact.
   - Exit 2 (misconfiguration) → print stderr and HALT.
   - Exit 0 → continue.

0b. Pipeline Step Guard (user-invoked state check):
   - If `--force` is present in the invocation arguments, skip this step entirely.
   - If `entity_kind == "bug"`, skip this step entirely (bug state is managed by meta-fix-bug.md).
   - Read current task state:
     `forge_store({ command: "read", args: ["task", "{record_id}", "--fields"] }) status`
   - Extract the `status` field from the JSON output.
   - Allowed states for this phase: `planned`.
   - If the current status is NOT in the allowed set:
     Print the following and HALT (do not proceed):
     `× Task {record_id} is in state '{status}' — /forge:plan must complete first. To run the full pipeline: /forge:run-task {record_id}`

1. Read Review Loop Context:
   - Check the spawning prompt for a `### Review Loop Context` block.
   - If present, extract:
     - `Iteration: N of M` — current attempt number and the configured limit
     - `Is final iteration: true/false`
   - If absent (user-invoked, not orchestrated): treat as `iteration 1`, no limit — do
     NOT read any iteration cap from config. The orchestrator owns loop budgets; a human
     standalone re-run is the escape hatch for stuck items (forge-engineering#34).
   - Include `(iteration N of M)` (orchestrated) or `(standalone review)` in the opening line of the `PLAN_REVIEW.md` artifact.
   - If this is the final iteration (`N == M`) and the verdict is `Revision Required`,
     append a `### Next Steps` section to the artifact showing:
     ```
     ### Next Steps
     - Force-approve (bypass remaining reviews): `/forge:approve --force {task_id}`
     - Increase iteration limit: edit `config.pipelines.{pipeline}.phases[review-plan].maxIterations`
     - Restart from review: `/forge:review-plan {task_id}`
     ```

2. Load Context:
   - Read task prompt (source of truth)
   - Read PLAN.md (subject of review)
   - Read stack checklist if available

3. Review:
   - Evaluate feasibility, completeness, security, architecture alignment, and testing strategy
   - Identify missing edge cases or failure modes

4. Verdict:
   - Write the plan review via forge_artifact: forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"plan-review", content:"<markdown>" })
     Use the format:
     **Verdict:** [Approved | Revision Required]
     - If Revision Required: provide numbered, actionable items
     - If Approved: provide any advisory notes
     - See step 1 for iteration header and final-iteration Next Steps requirements.

5. Finalize:
   - Transitions:
     - **Task mode** — predecessor must be `planned`.
       - Approved          → `plan-approved`
       - Revision Required → `plan-revision-required`
       - Out-of-band escapes (any state): `code-revision-required`, `blocked`, `escalated`, `abandoned`
       Update status: `forge_store({ command: "update-status", args: ["task", "{taskId}", "status", "plan-approved`"] }) (if Approved) or `... status plan-revision-required` (if Revision Required)
     - **Bug mode** — NO status write. The bug remains `in-progress`. The verdict signal travels through `summaries.review_plan.verdict` (read by `read-verdict.cjs § BUG_PHASE_VERDICT_SOURCE`), not `bug.status`. Writing `bug.status` here violates `meta-fix-bug.md § Iron Laws #2`.
   - **Do NOT emit a phase event yourself.** The orchestrator owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.

6. Emit Summary Sidecar:
   - Write the review-plan summary via forge_artifact: forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"review-plan-summary", content:"<JSON>" })
     The JSON must have the following shape:
     ```json
     {
       "objective":   "<one sentence — what this review assessed>",
       "findings":    ["<up to 12 bullets, 200 chars each — key issues or confirmations>"],
       "verdict":     "<approved | revision>",
       "written_at":  "<current ISO 8601 timestamp>",
       "artifact_ref":"PLAN_REVIEW.md"
     }
     ```
   - Call (task mode):
     ```
     forge_store({ command:"set-summary", args:["{record_id}", "review_plan"] })
     ```
     Or (bug mode):
     ```
     forge_store({ command:"set-bug-summary", args:["{record_id}", "review_plan"] })
     ```
     `args[1]` is the LITERAL phase key `review_plan`, never the record id; `forge_store` has no `entity`/`id`/`phase` field (see `_fragments/store-cli-verbs.md`).
   - If set-summary exits non-zero (guard: `expected summary key 'review_plan'`), `args[1]` was wrong — fix it to `review_plan` and retry. Do not return without a valid summary; the orchestrator halts as "verdict missing" if `summaries.review_plan` is absent.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Generation Instructions

- Enforce `**Verdict:** [Approved | Revision Required]` format exactly — orchestrator branches on this.
- **Markers (required by `/forge:run-task` kickoff shim):** Generated workflow MUST include the "Iron Laws" section, the "Store-Write Verification" section, the literal `forge_store` token, and the `.forge/personas/supervisor.md` persona path. Missing any → kickoff shim refuses to dispatch.
- Token Reporting: `_fragments/finalize.md` — wire via `file_ref:`.
- Event Emission: "complete" event MUST include the `eventId` passed by the orchestrator.
