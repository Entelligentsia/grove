# Meta-Template: Plan Review

## Purpose

Defines the structure of PLAN_REVIEW.md — the Supervisor's verdict on
the implementation plan.

## Sections

### Required
- **Verdict** — `Approved` / `Revision Required`
- **Review Summary** — overall assessment of the plan
- **Feasibility** — is the approach realistic and scoped correctly?
- **Security** — are auth and validation addressed?
- **Architecture Alignment** — does it follow established patterns?
- **Testing Strategy** — is the planned coverage adequate?

### If Revision Required
- **Required Changes** — numbered, actionable items
- **Priority** — which items block approval

### If Approved
- **Advisory Notes** — suggestions for implementation (non-blocking)

## Generation Instructions
- Reference the project's architecture sub-docs for alignment checks
- Include stack-specific security considerations
- Reference the project's test expectations
