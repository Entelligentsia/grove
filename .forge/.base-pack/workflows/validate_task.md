---
requirements:
  reasoning: High
  context: Medium
  speed: Low
audience: subagent
phase: validate
context:
  architecture: false
  prior_summaries: none
  persona: summary
  master_index: false
  diff_mode: true
deps:
  personas: [qa-engineer]
  skills: [qa-engineer, generic]
  templates: []
  sub_workflows: []
  kb_docs: [architecture/stack.md]
  config_fields: [commands.test, paths.engineering]
---


# Validate Task
## Iron Laws

- Validate against the acceptance criteria as written; do not soften, expand, or reinterpret them. The validator's job is to catch what the implementer optimistically considered "done".
- Read `.forge/personas/qa-engineer.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store` (or `node .forge/tools/store-cli.cjs`). Never edit `.forge/store/*.json` directly.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Algorithm

```

0a. Pre-flight Gate Check:
   - **Entity-mode resolution:** read the kickoff arguments. `--task {id}` → `entity_kind = "task"`, `record_id = {id}`. `--bug {id}` → `entity_kind = "bug"`, `record_id = {id}`. All store-cli calls below substitute `{entity_kind}` and `{record_id}` for the literal "task"/{taskId} placeholders.
   - Run: `node .forge/tools/preflight-gate.cjs --phase validate --{entity_kind} {record_id}`
   - Exit 1 (gate failed) → print stderr and HALT. Do not proceed; do not attempt to produce the artifact.
   - Exit 2 (misconfiguration) → print stderr and HALT.
   - Exit 0 → continue.

0b. Pipeline Step Guard (user-invoked state check):
   - If `--force` is present in the invocation arguments, skip this step entirely.
   - If `entity_kind == "bug"`, skip this step entirely (bug state is managed by meta-fix-bug.md).
   - Read current task state:
     `node .forge/tools/store-cli.cjs read task {record_id} --fields status`
   - Extract the `status` field from the JSON output.
   - Allowed states for this phase: `implemented`, `review-approved`.
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
   - Include `(iteration N of M)` (orchestrated) or `(standalone review)` in the opening line of the `VALIDATION_REPORT.md` artifact.
   - If this is the final iteration (`N == M`) and the verdict is `Revision Required`,
     append a `### Next Steps` section to the artifact showing:
     ```
     ### Next Steps
     - Force-approve (bypass remaining reviews): `/forge:approve --force {task_id}`
     - Increase iteration limit: edit `config.pipelines.{pipeline}.phases[validate].maxIterations`
     - Restart from validation: `/forge:validate {task_id}`
     ```

2. Load Context:
   - Read task prompt
   - Read approved PLAN.md
   - Read the implementation
   - Read PROGRESS.md

3. Validation:
   - Execute the "Acceptance Criteria" checklist from the plan
   - Verify that all technical constraints (e.g., performance, security) are met
   - Check for any regressions in related functionality
   - When re-running the test suite, use the **resolved test command** from `commands.test` in `.forge/config.json` (i.e. `` `${commands.test}` ``, e.g. `.venv/bin/python -m pytest`). Template placeholder: {{TEST_COMMAND}}. Do NOT invoke bare `python` / `python3` — the project interpreter is rarely on `$PATH`.

4. Verdict:
   - Write the validation report via:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"validation-report", content:"<markdown>" })`
     The markdown content must use the format:
     **Verdict:** [Approved | Revision Required]
     - If Revision Required: list the failed criteria and required fixes
     - If Approved: confirm the task is validated
     - See step 1 for iteration header and final-iteration Next Steps requirements.

5. Finalize:
   - Update task status via `node .forge/tools/store-cli.cjs update-status task {taskId} status review-approved` (if Approved) or `node .forge/tools/store-cli.cjs update-status task {taskId} status code-revision-required` (if Revision Required)
   - **Do NOT emit a phase event yourself.** The orchestrator owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.

6. Emit Summary Sidecar:
   - Write the validation summary via:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"validation-summary", content:"<JSON>" })`
     The JSON content must have the following shape:
     ```json
     {
       "objective":   "<one sentence — what acceptance criteria were validated>",
       "findings":    ["<up to 12 bullets, 200 chars each — pass/fail per criterion>"],
       "verdict":     "<approved | revision>",
       "written_at":  "<current ISO 8601 timestamp>",
       "artifact_ref":"VALIDATION_REPORT.md"
     }
     ```
   - Call (the sidecar path is auto-resolved from the task record's `path` — never pass it):
     ```
     node .forge/tools/store-cli.cjs set-summary {task_id} validation
     ```
   - If set-summary exits non-zero, fix the sidecar JSON and retry. Do not proceed without a valid summary.
```

## Friction Emit
Emit `type:friction` `{workflow:validate, persona:qa-engineer, issue}` per `_fragments/friction-emit.md`.

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->