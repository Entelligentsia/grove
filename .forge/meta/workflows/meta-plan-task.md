---
requirements:
  reasoning: High
  context: Medium
  speed: Low
audience: subagent
phase: plan
context:
  architecture: true
  prior_summaries: delta
  persona: summary
  master_index: false
  diff_mode: false
deps:
  personas: [architect]
  skills: [architect, generic]
  templates: [PLAN_TEMPLATE, TASK_PROMPT_TEMPLATE]
  sub_workflows: [review_plan]
  kb_docs: [architecture/stack.md]
  config_fields: [commands.test, paths.engineering]
---

# 🌱 Meta-Workflow: Plan Task

## Purpose

The Engineer reads the task prompt, researches the codebase, and produces an implementation plan.

## Algorithm

```

0a. Pre-flight Gate Check:
   - **Entity-mode resolution:** read the kickoff arguments. `--task {id}` → `entity_kind = "task"`, `record_id = {id}`. `--bug {id}` → `entity_kind = "bug"`, `record_id = {id}`. All store-cli calls below substitute `{entity_kind}` and `{record_id}` for the literal "task"/{taskId} placeholders.
   - Run: `forge_preflight({ phase: "plan", {entity_kind}: "{record_id}`" })
   - Exit 1 (gate failed) → print stderr and HALT. Do not proceed; do not attempt to produce the artifact.
   - Exit 2 (misconfiguration) → print stderr and HALT.
   - Exit 0 → continue.

0b. Pipeline Step Guard (user-invoked state check):
   - If `--force` is present in the invocation arguments, skip this step entirely.
   - If `entity_kind == "bug"`, skip this step entirely (bug state is managed by meta-fix-bug.md).
   - Read current task state:
     `forge_store({ command: "read", args: ["task", "{record_id}", "--fields"] }) status`
   - Extract the `status` field from the JSON output.
   - Allowed states for this phase: `draft`, `planned`, `plan-revision-required`.
   - If the current status is NOT in the allowed set:
     Print the following and HALT (do not proceed):
     `× Task {record_id} is in state '{status}' — /forge:plan cannot run from this state; a reset or reassignment must complete first. To run the full pipeline: /forge:run-task {record_id}`

1. Load Context:
   - Read `.forge/personas/architect.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
   - store-cli verbs: `read` | `list` | `write` | `emit` | `update-status` | `set-summary` | `describe` | `nlp` | `query` | `delete` — there is no `get`/`set`/`find`. See `_fragments/store-cli-verbs.md` for full notes; run `--help` before improvising.
   - Read task prompt (source of truth)
   - Query the store for this task and any related entities:
     ```sh
     forge_store({ command: "nlp", args: ["{taskId} with sprint with feature"] })
     ```
     Use store results directly if they include title, status, sprint, and excerpt.
   - Read the architecture summary from your injected context (if present).
   - Read business domain docs relevant to the task
   - Read stack checklist

2. Research:
   - Identify files for modification (Glob, Grep, Read)
   - Map existing patterns in the target area
   - Identify existing tests to be maintained or expanded
   - Identify whether the change is **material** (triggers version bump) or not:
     - Bug fixes to any command, hook, tool spec, or workflow → material
     - Tool-spec changes that alter generated tool behaviour → material
     - Command-file changes that alter behaviour → material
     - Hook changes → material
     - Schema changes to `.forge/store/` or `.forge/config.json` → material
     - Docs-only changes → NOT material

3. Plan:
   - Write the plan via forge_artifact:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"plan", content:"<markdown>" })`
     where `<markdown>` follows the project plan template.
   - Ensure inclusion of: Objective, Approach, Files to Modify, Data Model Changes, Testing Strategy, Acceptance Criteria, and Operational Impact

4. Knowledge Writeback:
   - If new patterns were discovered, update architecture or business domain docs

5. Finalize:
   - Transitions:
     - **Task mode** — legal target from this step: `draft → planned`. Out-of-band escapes (any state): `plan-revision-required`, `code-revision-required`, `blocked`, `escalated`, `abandoned`.
       Update status: `forge_store({ command: "update-status", args: ["task", "{taskId}", "status", "planned`"] })
     - **Bug mode** — NO status write. The bug remains `in-progress` until the commit phase transitions it to `fixed`. Writing `bug.status` here violates `meta-fix-bug.md § Iron Laws #2`.
   - **Do NOT emit a phase event yourself.** The orchestrator owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.

6. Emit Summary Sidecar:
   - Write the plan summary via forge_artifact. Shape:
     ```json
     {
       "objective":   "<one sentence — what this plan sets out to build>",
       "key_changes": ["<up to 12 bullets, 200 chars each>"],
       "verdict":     "n/a",
       "written_at":  "<current ISO 8601 timestamp>",
       "artifact_ref":"PLAN.md"
     }
     ```
     Task mode:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"plan-summary", content:"<JSON>" })`
     Bug mode:
     `forge_artifact({ command:"write", entity:"{entity_kind}", entityId:"{record_id}", artifact:"plan-summary", content:"<JSON>" })`
   - Register the summary with the store (task mode):
     `forge_store({ command:"set-summary", args:["{record_id}", "plan"] })`
     Or (bug mode):
     `forge_store({ command:"set-bug-summary", args:["{record_id}", "plan"] })`
   - If the set-summary call exits non-zero, fix the sidecar JSON and retry. Do not proceed without a valid summary.

7. Post-Phase Output Guard: satisfy the `outputs` block before returning.
```

```outputs phase=plan
artifact {engineering}/{sprint}/{task}/PLAN.md min=200
require summaries.plan.verdict == n/a
```

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance -->
## Iron Laws

- Follow the Algorithm step by step. No code, pseudocode, or implementation sketches in the plan.
- Read `.forge/personas/architect.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store`. Never edit `.forge/store/*.json` directly.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Friction Emit
Emit `type:friction` `{workflow:plan-task, persona:architect, issue}` per `_fragments/friction-emit.md`.

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Generation Instructions

- **Workflow Structure:** Strict "Algorithm" block format.
- **Markers (required by `/forge:plan` kickoff shim):** Generated workflow MUST include the "Iron Laws" section, the "Store-Write Verification" section, the literal `forge_store` token, and the `architect.md` persona path. Missing any → kickoff shim refuses to dispatch.
- **Context Isolation:** Forbid inline execution. Delegate complex sub-tasks via the `Agent` tool.
- **Project Specifics:**
  - Replace architecture/domain doc placeholders with actual project file paths.
  - Embed the project's specific PLAN template path.
- **Token Reporting:** See `_fragments/finalize.md` — wire via `file_ref:`.
- **Event Emission:** Ensure the "complete" event includes the `eventId` passed by the orchestrator.
