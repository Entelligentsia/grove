# Meta-Template: Code Review

## Purpose

Defines the structure of CODE_REVIEW.md — the Supervisor's verdict on
the implementation.

## Sections

### Required
- **Verdict** — `Approved` / `Approved with supervisor corrections` / `Revision Required`
- **Review Summary** — overall assessment
- **Checklist Results** — stack-checklist.md items checked, with pass/fail
- **Issues Found** — numbered list with severity, file:line, description, fix suggestion

### If Revision Required
- **Required Changes** — numbered, actionable items
- **Priority** — which items block approval

### If Approved
- **Advisory Notes** — non-blocking observations for future consideration

## Generation Instructions
- Load the project's stack-checklist.md as the review criteria source
- Include framework-specific review categories
- Reference the project's auth pattern in the security section
