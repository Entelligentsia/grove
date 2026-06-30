# Meta-Template: Sprint Requirements

## Purpose

Defines the structure of `SPRINT_REQUIREMENTS.md` — the output of the sprint
intake interview. This document is the primary input to sprint planning.
Every field marked Required must be completed before planning begins.

## Sections

### Required

- **Sprint ID** — next sequential ID using project prefix (e.g. ACME-S03)
- **Sprint Goals** — 1–3 concrete outcomes; each stated as an observable result
- **In Scope** — itemised list of must-have features, fixes, or changes for this sprint
- **Out of Scope** — explicit list of things not being done this sprint
- **Acceptance Criteria** — per in-scope item: specific, testable conditions for approval

### Conditional (include when raised during intake)

- **Nice-to-Have** — items to attempt only if must-haves are complete
- **Constraints** — technical, data, dependency, or timeline constraints
- **Risks** — identified unknowns or blockers with a brief mitigation note
- **Carry-Over** — items from the previous sprint with a note on their status

### Metadata

- **Captured** — date and sprint number
- **Source** — "sprint-intake interview" (confirms document came from intake, not assumptions)

## Document Shape

```markdown
# Sprint Requirements — {SPRINT_ID}

**Captured:** {DATE}
**Source:** sprint-intake interview

## Goals

1. {GOAL_1}
2. {GOAL_2}

## In Scope

### {ITEM_TITLE} [must-have]
{One-line description}

**Acceptance criteria:**
- {CRITERION_1}
- {CRITERION_2}

### {ITEM_TITLE} [must-have]
...

## Out of Scope

- {EXPLICIT_EXCLUSION_1}
- {EXPLICIT_EXCLUSION_2}

## Nice-to-Have (attempt if time allows)

- {ITEM}

## Constraints

- **Technical:** {CONSTRAINT}
- **Data:** {CONSTRAINT}
- **Dependencies:** {CONSTRAINT}
- **Timeline:** {CONSTRAINT}

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| {RISK} | High / Medium / Low | {MITIGATION} |

## Carry-Over from {PREV_SPRINT_ID}

| Item | Status | Notes |
|---|---|---|
| {ITEM} | Partial / Blocked | {NOTE} |
```

## Generation Instructions

- Replace `{SPRINT_ID}` with the project's next sequential sprint ID
- Acceptance criteria must be concrete and testable — reject vague criteria
  (e.g. "works well") during generation
- Include only sections relevant to this sprint — omit empty conditional sections
- The Carry-Over section requires reading the previous sprint's store entry
