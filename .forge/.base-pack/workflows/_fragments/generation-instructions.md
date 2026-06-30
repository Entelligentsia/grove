# Fragment: Generation Instructions — Structural Template Guide

> **Pattern:** This fragment is a template guide, not a verbatim include.  
> Each workflow's `## Generation Instructions` section is kept inline because the
> content is workflow-specific (slug names, persona paths, project-specific markers,
> and stack commands vary per workflow). Use this guide to ensure every `## Generation
> Instructions` section has the required subsections in the required order.
>
> **Distinction from verbatim fragments:** CM-6 (Generation Instructions repetition)
> is closed-as-documented, not closed-as-deduplicated. The prose stays inline; this
> guide documents the canonical template so future authors follow the same structure.
> See `doc/decisions/meta-fragment-includes.md`.

---

## Required subsection order

```markdown
## Generation Instructions

- **Persona Self-Load:** (if applicable — see below)
- **Workflow Structure:** The generated `{slug}.md` must follow the strict "Algorithm" block format.
- **Markers:** (if applicable — see kickoff-shim marker requirements)
- **Context Isolation:** Forbid inline execution of {domain}; use the `Agent` tool for sub-tasks.
- **Project Specifics:**
  - {stack-specific notes}
- **Token Reporting:** See `_fragments/finalize.md` — wire via `file_ref:`.
- **Event Emission:** Ensure the "complete" event includes the `eventId` passed by the orchestrator.
```

### Subsection definitions

**`Persona Self-Load`** — *conditional*. Required when the generated workflow uses the
Persona Self-Load pattern (the generated file must begin by reading a persona `.md` file
before any other tool use). Specify the exact persona path. Used in: `meta-collate.md`,
`meta-retro.md`, `meta-review-sprint-completion.md`, `meta-new-sprint.md`,
`meta-plan-sprint.md`. Omit for subagent-targeted workflows where the persona is loaded
by the orchestrator before dispatch.

**`Workflow Structure`** — *required*. Names the generated file slug and asserts the
"Algorithm" block format. Format: `The generated \`{slug}.md\` must follow the strict "Algorithm" block format.`

**`Markers`** — *conditional*. Required when a kickoff shim validates structural markers
in the generated workflow (e.g., presence of `## Iron Laws`, `## Store-Write Verification`,
`forge_store` token, persona path). Omit when no kickoff shim validates the output.

**`Context Isolation`** — *required*. Names the domain-specific operation that must NOT
run inline, and mandates the `Agent` tool for sub-tasks.

**`Project Specifics`** — *required*. One or more bullets describing the project-context
substitutions the generator must embed (stack commands, KB paths, template references,
task ID format, etc.).

**`Token Reporting`** — *required* (with one exception: `meta-check-agent.md` omits it
because check-agent is an inline helper, not an orchestrated phase). Standard form: `See
\`_fragments/finalize.md\` — wire via \`file_ref:\`.` Expanded token-reporting prose is
used in workflows where the generated file must mandate a specific sidecar-emit sequence
(retrospective, sprint-intake, sprint-plan, review-sprint-completion) — in those cases,
the full expanded form lives inline.

**`Event Emission`** — *required* (same exception as Token Reporting). Standard form:
`Ensure the "complete" event includes the \`eventId\` passed by the orchestrator.`

---

## Orchestrator-special cases

`meta-fix-bug.md` and `meta-orchestrate.md` have Generation Instructions that contain
workflow-specific structural requirements (role-to-noun mapping, PERSONA_MAP, spawn
patterns, fault tolerance rules). These cannot be collapsed into the standard subsection
template. Their Generation Instructions are long-form inline prose.

---

## Byte-budget note

A standard Generation Instructions section (five subsections, no expanded token reporting)
contributes approximately 350–450 bytes. Sections with expanded token-reporting prose
(sprint-intake, retrospective) may reach 700–900 bytes. Byte budgets in
`phase-frontmatter.test.cjs` must be set ≥ the actual byte count of the file (with
≤ 512 bytes headroom).
