---
requirements:
  reasoning: High
  context: Medium
  speed: Low
deps:
  personas: [architect]
  skills: [architect, generic]
  templates: [RETROSPECTIVE_TEMPLATE]
  sub_workflows: []
  kb_docs: [architecture/stack.md]
  config_fields: [paths.engineering]
---


# Retrospective
## Iron Laws

- Never mutate JSON records during retrospective; the store is the source of truth and retrospective flows downstream from it. Retrospective operations read store data and write markdown views only.
- Read `.forge/personas/architect.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store` (or `node .forge/tools/store-cli.cjs`). Never edit `.forge/store/*.json` directly.

## Algorithm

```
1. Load Context:
   - Read all task manifests for the sprint
   - Read all event logs (including token usage)
   - Read all retrospective notes gathered during the sprint

2. Analysis:
   - Calculate total sprint cost (tokens/USD)
   - Identify "bottleneck" tasks (high iteration counts or long duration)
   - Analyze common failure modes in reviews

3. Knowledge Update:
   - Update architecture/domain docs with "lessons learned"
   - Propose improvements to meta-workflows based on analysis
   - Update stack-checklist with new verification steps

4. Finalize:
   - Write SPRINT_RETROSPECTIVE.md
   - Update sprint status via `node .forge/tools/store-cli.cjs update-status sprint {sprintId} status retrospective-done`
   - Run `node .forge/tools/collate.cjs {sprintId} --purge-events`
     This single deterministic step: generates COST_REPORT.md from all
     accumulated events, then deletes `.forge/store/events/{sprintId}/`.
     COST_REPORT.md is the durable record; the raw event files are not
     retained after retrospective close.
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.
     (tombstone — written after the purge; the only event in the directory
     going forward)
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->