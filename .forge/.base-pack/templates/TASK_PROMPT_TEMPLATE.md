# {TASK_ID}: {TASK_TITLE}

**Sprint:** {SPRINT_ID}
**Estimate:** {S/M/L/XL}
**Pipeline:** default *(or specify a named pipeline)*

---

## Objective

{What this task achieves — user-facing value, one paragraph}

## Acceptance Criteria

1. {Criterion — specific and testable}
2. {Criterion}
3. `node --check` passes on all modified JS/CJS files
4. `node forge/tools/validate-store.cjs --dry-run` exits 0 (if schema changed)

## Context

{Relevant background — links to related tasks, bugs, GitHub issues, prior decisions}

## Plugin Artifacts Involved

{Which files in `forge/` are affected: commands/, hooks/, tools/, schemas/, meta/}

## Operational Impact

- **Version bump:** {required / not required — reason}
- **Regeneration:** {users must run `/forge:update` / no user action needed}
- **Security scan:** {required / not required}
