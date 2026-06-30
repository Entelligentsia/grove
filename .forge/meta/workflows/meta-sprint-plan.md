---
requirements:
  reasoning: High
  context: Medium
  speed: Low
deps:
  personas: [architect]
  skills: [architect, generic]
  templates: [SPRINT_MANIFEST_TEMPLATE, TASK_PROMPT_TEMPLATE]
  sub_workflows: []
  kb_docs: [MASTER_INDEX.md, architecture/stack.md]
  config_fields: [project.prefix, paths.engineering]
---

# 🗻 Meta-Workflow: Sprint Plan

## Purpose

Break sprint requirements into a set of estimated tasks with a dependency graph.

## Algorithm

```
0. Pre-flight Gate Check:
   - Probe token reporting: if the host runtime exposes a `/cost` slash command
     (Claude Code), invoke it; on any other runtime treat as unavailable. Do
     NOT search for or invent a `cost-cli.cjs` — there is no such tool.
   - If the probe succeeds → note for later (will use reported data)
   - If the probe fails or is unavailable → note for later (will use estimates)

1. Load Context:
   - Query the store to orient on current project state before reading docs:
     ```sh
     forge_store({ command: "nlp", args: ["latest sprint"] })
     forge_store({ command: "nlp", args: ["open bugs"] })
     ```
     Use results (titles, statuses, excerpts, file refs) to skip manual MASTER_INDEX.md navigation where sufficient.
   - Read SPRINT_REQUIREMENTS.md
   - Read architecture and domain docs
   - Read the stack checklist

2. Task Decomposition:
   - Break requirements into atomic tasks
   - Assign each task to a Feature (if applicable)
   - Define clear acceptance criteria for each task
   - Use the `ID-description` format (e.g., `FORGE-S05-agent-runtime-portability`) for sprint and task folders.

3. Estimation and Sequencing:
   - Estimate each task (S, M, L) based on complexity
   - Define the dependency graph (which tasks must precede others)
   - Identify the critical path

4. Documentation:
   - Write SPRINT_PLAN.md to `engineering/sprints/{sprintId}/SPRINT_PLAN.md`
   - Create each task via `forge_store({ command: "write", args: ["task", "{task-json}"] })`.
     If the tool returns an error or the PreToolUse hook blocks the write:
     parse the error, correct the JSON, and retry (see Store-Write Verification).
     Do not proceed to the next task until this write succeeds.
   - Update the sprint record with all new task IDs via `forge_store({ command: "write", args: ["sprint", "{updated-sprint-json}"] })` (the sprint JSON must include the complete `taskIds` array with all newly created task IDs).
     If the tool returns an error or the PreToolUse hook blocks the write:
     parse the error, correct the JSON, and retry (see Store-Write Verification).
     Do not proceed until this write succeeds.
   - For each task, create its task folder and write TASK_PROMPT.md:
     * Folder: `engineering/sprints/{sprintId}/{taskId}/`
     * File: `TASK_PROMPT.md` — populate from `.forge/templates/TASK_PROMPT_TEMPLATE.md`
       filling in title, objective, acceptance criteria, entities, DSL/CLI changes, and operational impact
   - Update sprint status via `forge_store({ command: "update-status", args: ["sprint", "{sprintId}", "status", "active`."] })
     If the command exits non-zero, parse the error and retry
     (see Store-Write Verification). Do not proceed until this write succeeds.

5. Finalize:
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.
```

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance (sprint-plan uses verbatim Anti-Pattern Guard pattern — orchestrator-special case) -->
## Anti-Pattern Guard

The generated workflow MUST include the following section verbatim, placed immediately
after the Purpose heading and before the Algorithm block:

```
## Iron Laws

- YOU MUST NOT write any code, pseudocode, or implementation sketch.
- YOU MUST NOT produce a plan of your own before reading this workflow.
- YOU MUST follow the Algorithm below step by step. Reading it is not optional.
- If you have already read SPRINT_REQUIREMENTS.md and feel ready to decompose tasks:
  stop. Return to step 1 of the Algorithm and proceed from there.
```

<!-- See _fragments/store-write-verification.md — NOTE: this file uses an intentionally expanded
     Store-Write Verification variant that explains what counts as a store write in the sprint-plan
     context (includes direct Write/Edit tool calls). Canonical fragment is reference only. -->
## Store-Write Verification

Every write to the Forge store MUST succeed before the agent proceeds to the next
step. Store writes include `store-cli` commands (`write`, `update-status`, `emit`)
and direct `Write`/`Edit` tool calls against Forge-owned paths under
`.forge/store/`.

If a write is rejected — either by `store-cli` exiting non-zero or by the
`PreToolUse` write-boundary hook blocking the call with exit code 2 — the agent
MUST:

1. **Parse the error.** Both `store-cli` and the write-boundary hook emit
   structured messages naming the offending field and referencing the relevant
   schema file (e.g. `forge/schemas/task.schema.json`). Read the error carefully
   to identify which field is wrong and what the schema expects.
2. **Correct the data.** Fix the JSON payload to satisfy the schema: add missing
   required fields, fix type mismatches, remove undeclared properties, correct
   enum values, etc. If the hook message mentions a schema file, read that file
   to understand the full shape.
3. **Retry the write.** Re-attempt the same store write with the corrected payload.
4. **Repeat until success.** Do NOT advance past the current step, emit events,
   or produce further artifacts until the write is confirmed successful.

**Maximum retries: 3.** If the write still fails after 3 correction attempts,
halt and escalate to the human. Include the original payload, the corrected
payload, and all error messages in the escalation.

**Do NOT** set `FORGE_SKIP_WRITE_VALIDATION=1` to bypass a schema error. That
environment variable is reserved for emergency operator repair only.

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Generation Instructions

- **Persona Self-Load:** The generated workflow MUST begin by reading `.forge/personas/architect.md` as its first step (before any other tool use). This replaces the former inline `## Persona` section. The persona identity line (emoji, name, tagline) should be printed to stdout after reading the file.
- **Workflow Structure:** The generated `sprint_plan.md` must follow the strict "Algorithm" block format.
- **Context Isolation:** Forbid inline execution of task decomposition or estimation; use the `Agent` tool for sub-tasks.
- **Project Specifics:**
  - Use project-specific estimation guidelines.
  - Reference project's task manifest schema.
- **Token Reporting:** The generated workflow MUST mandate the following before returning:
  1. Probe session token usage: invoke `/cost` if the host runtime supports it
     (Claude Code only); on any other runtime treat as unavailable. Do NOT
     shell out to a `cost-cli.cjs` — there is no such tool.
  2. If the probe succeeds:
     - Parse: `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `estimatedCostUSD`.
     - Add `"source": "reported"` to sidecar JSON.
  3. If the probe fails or is unavailable:
     - Set token fields to `null`: `"inputTokens": null, "outputTokens": null, "estimatedCostUSD": null`.
     - Add `"source": "missing"` to sidecar JSON.
     - Log: "Token data unavailable (cost probe failed). Backfill later via estimate-usage.cjs."
  4. Write the usage sidecar via `forge_store({ command: "emit", args: ["{sprintId}", '{sidecar-json}'] }) --sidecar`.
  5. **NEVER skip sidecar write.** Always emit (reported or placeholder with nulls).
- **Event Emission:** Ensure the "complete" event includes the `eventId` passed by the orchestrator.
- **Store-Write Verification:** The generated workflow MUST include the "Store-Write
  Verification" section verbatim. Every `store-cli write`, `store-cli update-status`,
  and `store-cli emit` command in the Algorithm must be annotated with the
  parse-correct-retry instruction: "If the command exits non-zero or the
  PreToolUse hook blocks the write: parse the error, correct the JSON, and retry
  (see Store-Write Verification). Do not proceed until this write succeeds."
