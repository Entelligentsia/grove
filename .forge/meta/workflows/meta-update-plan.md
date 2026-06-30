---
requirements:
  reasoning: Medium
  context: Medium
  speed: Medium
audience: subagent
phase: update-plan
context:
  architecture: false
  prior_summaries: delta
  persona: summary
  master_index: false
  diff_mode: false
deps:
  personas: [architect]
  skills: [architect, generic]
  templates: [PLAN_TEMPLATE]
  sub_workflows: [review_plan]
  kb_docs: [architecture/stack.md]
  config_fields: [paths.engineering]
---

# 🌱 Meta-Workflow: Update Plan

## Purpose

Update the implementation plan of a task based on a "Revision Required" verdict from the plan review phase.

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance -->
## Iron Laws

- Address every numbered finding in the review artifact. Do not silently drop items; if a finding is wrong, note the reason in the revised plan rather than ignoring it.
- Read `.forge/personas/architect.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store`. Never edit `.forge/store/*.json` directly.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Algorithm

```
1. Load Context:
   - Read the original task prompt
   - Read the current PLAN.md
   - Read the review artifact (PLAN_REVIEW.md)

2. Analysis:
   - Review the numbered, actionable items in the review artifact
   - Determine where the plan was insufficient or incorrect

3. Revision:
   - Update PLAN.md to address all review findings
   - Ensure the revised plan remains aligned with the task prompt
   - Update the "Operational Impact" or "Testing Strategy" if the revision changed them

4. Finalize:
   - Update task status via `forge_store({ command: "update-status", args: ["task", "{taskId}", "status", "planned`"] })
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Generation Instructions

- **Workflow Structure:** The generated `update_plan.md` must follow the strict "Algorithm" block format.
- **Context Isolation:** Forbid inline execution of plan revision; use the `Agent` tool for sub-tasks.
- **Project Specifics:**
  - Reference the project's plan template.
- **Token Reporting:** See `_fragments/finalize.md` — wire via `file_ref:`.
- **Event Emission:** Ensure the "complete" event includes the `eventId` passed by the orchestrator.
