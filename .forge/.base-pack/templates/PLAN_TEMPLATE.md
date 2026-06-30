# PLAN — {TASK_ID}: {TASK_TITLE}

🌱 *{{PROJECT_NAME}} Engineer*

**Task:** {TASK_ID}
**Sprint:** {SPRINT_ID}
**Estimate:** {S/M/L/XL}

---

## Objective

{What this plan achieves — one paragraph}

## Approach

{High-level strategy — how the task will be implemented}

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `forge/...` | {description} | {why} |

## Plugin Impact Assessment

- **Version bump required?** Yes / No — {reason}
- **Migration entry required?** Yes / No — {regenerate targets if yes}
- **Security scan required?** Yes / No — {any change to `forge/` requires scan}
- **Schema change?** Yes / No — {which schemas affected}

## Testing Strategy

- Syntax check: `node --check <modified files>`
- Store validation: `node forge/tools/validate-store.cjs --dry-run` {if schema changed}
- Manual smoke test: {describe what to verify in a test project if needed}

## Acceptance Criteria

- [ ] {Criterion 1 — concrete and verifiable}
- [ ] {Criterion 2}
- [ ] `node --check` passes on all modified JS/CJS files
- [ ] `node forge/tools/validate-store.cjs --dry-run` exits 0

## Operational Impact

- **Distribution:** {does this change require users to run `/forge:update`?}
- **Backwards compatibility:** {will existing installed instances break?}
