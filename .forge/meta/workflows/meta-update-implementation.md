---
requirements:
  reasoning: Medium
  context: Medium
  speed: Medium
audience: subagent
phase: update-impl
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
  kb_docs: [architecture/stack.md]
  config_fields: [commands.test, paths.engineering]
---

# 🌱 Meta-Workflow: Update Implementation

## Purpose

Update the implementation of a task based on a "Revision Required" verdict from a review phase.

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance -->
## Iron Laws

- Address every "Revision Required" item from the review artifact at the correct code location; do not paper over them with comments. If a finding is wrong, escalate rather than ignore.
- Read `.forge/personas/engineer.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store`. Never edit `.forge/store/*.json` directly.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Algorithm

```
1. Load Context:
   - Read current implementation (code)
   - Read the review artifact (CODE_REVIEW.md or VALIDATION_REPORT.md)
   - Read the approved PLAN.md

2. Analysis:
   - Map the "Revision Required" items to specific code locations
   - Determine if the required changes necessitate a plan update

3. Implementation:
   - Apply the necessary fixes/changes
   - Verify the changes using the **resolved test command** from `commands.test` in `.forge/config.json` (i.e. `` `${commands.test}` ``, e.g. `.venv/bin/python -m pytest`). Template placeholder: {{TEST_COMMAND}}. Do NOT invoke bare `python` / `python3` — the project interpreter is rarely on `$PATH`.
   - Update PROGRESS.md with a summary of the revisions

4. Finalize:
   - Update task status via `forge_store({ command: "update-status", args: ["task", "{taskId}", "status", "implemented`"] })
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Generation Instructions

- **Workflow Structure:** The generated `update_implementation.md` must follow the strict "Algorithm" block format.
- **Context Isolation:** Forbid inline execution of fix logic; use the `Agent` tool for sub-tasks.
- **Project Specifics:**
  - Reference project-specific verification commands.
- **Token Reporting:** See `_fragments/finalize.md` — wire via `file_ref:`.
- **Event Emission:** Ensure the "complete" event includes the `eventId` passed by the orchestrator.
