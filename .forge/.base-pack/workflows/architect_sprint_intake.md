---
requirements:
  reasoning: High
  context: Medium
  speed: Low
deps:
  personas: [product-manager]
  skills: [architect, generic]
  templates: [SPRINT_REQUIREMENTS_TEMPLATE, SPRINT_MANIFEST_TEMPLATE]
  sub_workflows: []
  kb_docs: [MASTER_INDEX.md, architecture/stack.md]
  config_fields: [project.prefix, paths.engineering]
---


# Sprint Intake
## Iron Laws

- Capture requirements accurately; do not editorialize or pre-select options on the user's behalf. The product manager documents what the user says, not what the agent thinks is best.
- Read `.forge/personas/product-manager.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store` (or `node .forge/tools/store-cli.cjs`). Never edit `.forge/store/*.json` directly.

## Algorithm

```
0. Project Orientation:
   - Your current working directory is the project root.
   - Forge config lives at `.forge/config.json` (relative to cwd); the `forge_config` MCP tool returns canonical values.
   - Engineering knowledge lives under `engineering/` (relative to cwd) — `MASTER_INDEX.md`, `architecture/`, `business-domain/`, `sprints/`, `features/`.
   - Paths in subsequent steps resolve against this cwd.

1. Pre-flight Gate Check:
   - Probe token reporting: if the host runtime exposes a `/cost` slash command
     (Claude Code), invoke it; on any other runtime treat as unavailable and
     proceed. Do NOT search for or invent a `cost-cli.cjs` — there is no such tool.
   - If the probe succeeds → note for later (will use reported data)
   - If the probe fails or is unavailable → note for later (will use estimates)

2. Load Context:
   - Read `engineering/MASTER_INDEX.md` (relative to cwd)
   - Read any pending feature requests or bug reports under `engineering/`

3. Requirements Interview:
   - Conduct a structured interview with the user
   - Capture: Objectives, Constraints, Deliverables, and Success Criteria
   - Clarify ambiguous requirements through iterative questioning

4. Document Requirements:
   - Generate `engineering/sprints/<SPRINT_ID>/SPRINT_REQUIREMENTS.md`
   - Map requirements to existing Features if applicable
   - Ensure all deliverables are measurable and testable

5. Finalize:
   - Update sprint status via `node .forge/tools/store-cli.cjs update-status sprint {sprintId} status planning`
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->