---
requirements:
  reasoning: High
  context: Medium
  speed: Low
deps:
  personas: [architect]
  skills: [architect, generic]
  templates: []
  sub_workflows: []
  kb_docs: [architecture/stack.md]
  config_fields: [paths.engineering]
---

# 🗻 Meta-Workflow: Review Sprint Completion

## Purpose

Verify that all tasks in a sprint have been completed, committed, and validated before closing the sprint.

<!-- No Iron Laws section: review-sprint-completion is a read-only verification workflow. No store writes; no entity state transitions (status update in step 4 is the only write — it matches the standard store-I/O law applied by the generated workflow). Intentional omission per N-CM-5 audit. -->

## Algorithm

```
1. Load Context:
   - Read sprint manifest
   - Read all task manifests in the sprint
   - Check VCS for all expected commit hashes

2. Verification:
   - Confirm every task is in `committed` status
   - Verify all `approved` tasks have a corresponding commit
   - Check for any lingering "escalated" tasks

3. Verdict:
   - Write SPRINT_COMPLETION_REVIEW.md using the format:
     **Verdict:** [Approved | Revision Required]
     - If Revision Required: list missing commits or unresolved tasks
     - If Approved: confirm sprint is ready for retrospective

4. Finalize:
   - If step-3 verdict is `Approved`:
     - Update sprint status to `completed` via
       `forge_store({ command: "update-status", args: ["sprint", "{sprintId}", "status", "completed`"] })
   - If step-3 verdict is `Revision Required` and orchestrator passed `mode=partial`:
     - Update sprint status to `partially-completed` via
       `forge_store({ command: "update-status", args: ["sprint", "{sprintId}", "status", "partially-completed`"] })
   - If step-3 verdict is `Revision Required` and orchestrator passed `mode=complete`:
     - Do NOT transition status. Leave the sprint at its current status and exit;
       the orchestrator will surface the verdict to the user.
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Generation Instructions

- **Persona Self-Load:** The generated workflow MUST begin by reading `.forge/personas/architect.md` as its first step (before any other tool use). This replaces the former inline `## Persona` section. The persona identity line (emoji, name, tagline) should be printed to stdout after reading the file.
- **Workflow Structure:** The generated `review_sprint_completion.md` must follow the strict "Algorithm" block format.
- **Verdict Detection:** The generated workflow MUST enforce the strict `**Verdict:** [Approved | Revision Required]` format.
- **Context Isolation:** Forbid inline execution of sprint-wide audits; use the `Agent` tool for sub-tasks.
- **Token Reporting:** The generated workflow MUST mandate the following before returning:
  1. Probe session token usage: invoke `/cost` if the host runtime supports it
     (Claude Code only); on any other runtime treat as unavailable and proceed.
     Do NOT shell out to a `cost-cli.cjs` — there is no such tool.
  2. Parse: `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `estimatedCostUSD`.
  3. Write the usage sidecar via `forge_store({ command: "emit", args: ["{sprintId}", '{sidecar-json}'] }) --sidecar`.
- **Event Emission:** Ensure the "complete" event includes the `eventId` passed by the orchestrator.
