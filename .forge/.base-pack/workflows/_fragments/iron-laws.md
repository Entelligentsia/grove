# Fragment: Iron Laws — Structural Template Guide

> **Pattern:** This fragment is a template guide, not a verbatim include.  
> Each workflow's `## Iron Laws` section is kept inline because the opening law and
> persona load line are workflow-specific. Use this guide to ensure every `## Iron
> Laws` section has the required structure.
>
> **Distinction from verbatim fragments:** CM-2 (Iron Laws structural duplication)
> and CM-3 (persona-load line repetition) are closed-as-documented, not
> closed-as-deduplicated. The prose stays inline; this guide documents the canonical
> structure so future authors follow the same pattern. See `doc/decisions/meta-fragment-includes.md`.

---

## Required structure

An Iron Laws section MUST contain exactly these three bullets in this order:

```markdown
## Iron Laws

- {WORKFLOW_SPECIFIC_LAW}
- Read `.forge/personas/{persona}.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store` (or `node .forge/tools/store-cli.cjs`). Never edit `.forge/store/*.json` directly.
```

### Slot definitions

**`{WORKFLOW_SPECIFIC_LAW}`** — first bullet; unique per workflow. Describes the single most
important constraint of *this workflow* (e.g., "Commit only the artifacts produced for this
task", "Approve only when the implementation is consistent with the project's architecture",
"Follow the plan exactly"). Never generic; never copied from another workflow.

**persona load line** — second bullet; constant prose with `{persona}` replaced by the
workflow's `deps.personas[0]` value (e.g., `engineer`, `architect`, `supervisor`,
`product-manager`, `qa-engineer`).

**store-I/O law** — third bullet; verbatim identical across all workflows.

---

## Orchestrator-special case

`meta-fix-bug.md` and `meta-orchestrate.md` have Iron Laws that *defer* to
`generic-skills.md § Orchestrator Iron Laws` via a prose comment, then add
workflow-specific supplemental laws. This is intentional and cannot be collapsed
into the standard three-bullet structure.

---

## Intentional omissions

Some workflows have no `## Iron Laws` section by design:

- `meta-check-agent.md` — utility workflow; no store writes; no output contract.
- `meta-review-sprint-completion.md` — read-only verification; no state transitions.

Intentional omissions are documented with an inline comment before the `## Algorithm`
heading:
```html
<!-- No Iron Laws section: {reason} -->
```

---

## Byte-budget note

The Iron Laws section (three bullets) contributes approximately 280–320 bytes to a
workflow file. Byte budgets in `phase-frontmatter.test.cjs` must be set ≥ the actual
byte count of the file (with ≤ 512 bytes headroom). Lowering budgets after removing
content is always safe; raising budgets requires written justification in the commit
body naming the new content added.
