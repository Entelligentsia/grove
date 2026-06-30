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


# Review Sprint Completion
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
       `node .forge/tools/store-cli.cjs update-status sprint {sprintId} status completed`
   - If step-3 verdict is `Revision Required` and orchestrator passed `mode=partial`:
     - Update sprint status to `partially-completed` via
       `node .forge/tools/store-cli.cjs update-status sprint {sprintId} status partially-completed`
   - If step-3 verdict is `Revision Required` and orchestrator passed `mode=complete`:
     - Do NOT transition status. Leave the sprint at its current status and exit;
       the orchestrator will surface the verdict to the user.
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->