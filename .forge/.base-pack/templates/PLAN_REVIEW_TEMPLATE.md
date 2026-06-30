# PLAN REVIEW — {TASK_ID}: {TASK_TITLE}

🌿 *{{PROJECT_NAME}} Supervisor*

**Task:** {TASK_ID}

---

**Verdict:** Approved / Revision Required

---

## Review Summary

{Overall assessment of the plan — 2-3 sentences}

## Feasibility

{Is the approach realistic and correctly scoped? Does it touch the right files?}

## Plugin Impact Assessment

- **Version bump declared correctly?** Yes / No
- **Migration entry targets correct?** Yes / No / N/A
- **Security scan requirement acknowledged?** Yes / No

## Security

{Are there any risks introduced by this change to a plugin shipped to all Forge users?
Prompt injection risk in new Markdown files? Data exfiltration risk in hook changes?}

## Architecture Alignment

- Does the approach follow established patterns (built-ins only, strict exit codes, etc.)?
- Does it preserve `additionalProperties: false` in any schema changes?

## Testing Strategy

{Is the planned testing adequate? Does it include syntax check + validate-store?}

---

## If Revision Required

### Required Changes

1. {Change 1 — actionable}
2. {Change 2}

### Priority

{Which items block approval}

---

## If Approved

### Advisory Notes

{Non-blocking suggestions for implementation}
