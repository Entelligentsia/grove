---
requirements:
  reasoning: Medium
  context: Medium
  speed: Medium
audience: subagent
phase: implement
context:
  architecture: false
  prior_summaries: delta
  persona: summary
  master_index: false
  diff_mode: false
deps:
  personas: [engineer]
  skills: [engineer, generic]
  templates: [PROGRESS_TEMPLATE]
  sub_workflows: [review_code]
  kb_docs: [architecture/stack.md, architecture/routing.md]
  config_fields: [commands.test, paths.engineering]
---


# Implement Plan
## Algorithm

```

0a. Pre-flight Gate Check:
   - **Entity-mode resolution:** read the kickoff arguments. `--task {id}` → `entity_kind = "task"`, `record_id = {id}`. `--bug {id}` → `entity_kind = "bug"`, `record_id = {id}`. All store-cli calls below substitute `{entity_kind}` and `{record_id}` for the literal "task"/{taskId} placeholders.
   - Run: `node .forge/tools/preflight-gate.cjs --phase implement --{entity_kind} {record_id}`
   - Exit 1 (gate failed) → print stderr and HALT. Do not proceed; do not attempt to produce the artifact.
   - Exit 2 (misconfiguration) → print stderr and HALT.
   - Exit 0 → continue.

0b. Pipeline Step Guard (user-invoked state check):
   - If `--force` is present in the invocation arguments, skip this step entirely.
   - If `entity_kind == "bug"`, skip this step entirely (bug state is managed by meta-fix-bug.md).
   - Read current task state:
     `node .forge/tools/store-cli.cjs read task {record_id} --fields status`
   - Extract the `status` field from the JSON output.
   - Allowed states for this phase: `plan-approved`.
   - If the current status is NOT in the allowed set:
     Print the following and HALT (do not proceed):
     `× Task {record_id} is in state '{status}' — /forge:review-plan must complete first. To run the full pipeline: /forge:run-task {record_id}`

1. Load Context:
   - Read `.forge/personas/engineer.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
   - store-cli verbs: `read` | `list` | `write` | `emit` | `update-status` | `set-summary` | `describe` | `nlp` | `query` | `delete` — there is no `get`/`set`/`find`. See `_fragments/store-cli-verbs.md` for full notes; run `--help` before improvising.
   - Read the approved PLAN.md
   - Read business domain docs relevant to the task

2. Implementation:
   - Execute plan steps incrementally
   - Perform "compile/check" after each significant change
   - Ensure all new code follows established project patterns

3. Verification:
   - Run syntax verification: {SYNTAX_CHECK}
   - Run test suite using the **resolved test command** from `commands.test` in `.forge/config.json` (i.e. `` `${commands.test}` ``, e.g. `.venv/bin/python -m pytest`, `npm test`, `cargo test`). Do NOT invoke bare `python` / `python3` / `pytest` — the project interpreter is rarely on `$PATH`; using the resolved command avoids `python: command not found` and `No module named pytest`.
   - Template placeholder: {{TEST_COMMAND}} (resolved at materialize time from `commands.test`).
   - Run build if frontend assets modified: {BUILD_COMMAND}

4. Documentation:
   - Write PROGRESS.md via `forge_artifact`:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"progress", content:"<markdown>" })`
     Content must include: summary of changes, test evidence (copy of output), files changed manifest.

5. Knowledge Writeback:
   - Update architecture/domain/stack-checklist if discoveries were made
   - Tag updates: `<!-- Discovered during {TASK_ID} — {date} -->`

6. Finalize:
   - Transitions:
     - **Task mode** — legal predecessors are `planned`, `plan-approved`, or `implementing`; target is `implemented`.
       - `planned`        → `implemented` (workflow-prose path — direct)
       - `plan-approved`  → `implementing` → `implemented` (supervisor-review path)
       - Out-of-band escapes (any state): `plan-revision-required`, `code-revision-required`, `blocked`, `escalated`, `abandoned`
       Update status — check current state first:
       - If predecessor is `planned` or `implementing`:
         `node .forge/tools/store-cli.cjs update-status task {taskId} status implemented`
       - If predecessor is `plan-approved` (two-step mandatory — FSM forbids skipping `implementing`):
         `node .forge/tools/store-cli.cjs update-status task {taskId} status implementing`
         `node .forge/tools/store-cli.cjs update-status task {taskId} status implemented`
     - **Bug mode** — NO status write. The bug remains `in-progress` until the commit phase transitions it to `fixed`. Writing `bug.status` here violates `meta-fix-bug.md § Iron Laws #2`.
   - **Do NOT emit a phase event yourself.** The orchestrator owns event emission — it composes the canonical event from runtime telemetry plus the SUMMARY you write next. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (Plan 11 / Slice 2). Write the SUMMARY and return.

7. Emit Summary Sidecar:
   - Write `IMPLEMENTATION-SUMMARY.json` via `forge_artifact`:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"implementation-summary", content:"<JSON>" })`
     JSON shape: `{"objective":"<one sentence>", "key_changes":["<up to 12 bullets>"], "files_changed":["<path>"], "verdict":"n/a", "written_at":"<ISO 8601>", "artifact_ref":"PROGRESS.md"}`
   - `files_changed`: every repo path this phase changed (one `git status --porcelain`); `commit-task.cjs` stages from it.
   - Then link sidecar to store (task mode):
     `forge_store({ command:"set-summary", args:["{taskId}", "implementation"] })`
     Or (bug mode):
     `forge_store({ command:"set-bug-summary", args:["{bugId}", "implementation"] })`
     The sidecar path is auto-resolved from the record's `path` — never pass it.

8. Post-Phase Output Guard: the `outputs` block below is the authoritative enforcer.
   You MUST satisfy it before returning. If PROGRESS.md is missing or too small,
   re-run the relevant step before emitting the complete event.
```

```outputs phase=implement
artifact {engineering}/{sprint}/{task}/PROGRESS.md min=200
require summaries.implementation.verdict == n/a
```

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance -->
## Iron Laws

- Follow the Algorithm step by step. Execute the approved PLAN.md exactly; do not invent scope or skip steps without updating the plan first.
- Read `.forge/personas/engineer.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store` (or `node .forge/tools/store-cli.cjs`). Never edit `.forge/store/*.json` directly.
- Run the full test suite before declaring the task implemented. Silent continuation past test failures is never acceptable.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Friction Emit
Emit `type:friction` `{workflow:implement, persona:engineer, issue}` per `_fragments/friction-emit.md`.

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->