# PROGRESS — {TASK_ID}: {TASK_TITLE}

🌱 *{{PROJECT_NAME}} Engineer*

**Task:** {TASK_ID}
**Sprint:** {SPRINT_ID}

---

## Summary

{What was done — 2-4 sentences}

## Syntax Check Results

```
$ node --check forge/...
{output — must be clean}
```

## Store Validation Results

```
$ node forge/tools/validate-store.cjs --dry-run
{output — must exit 0 if any schema was changed}
```

## Files Changed

| File | Change |
|---|---|
| `forge/...` | {description} |

## Acceptance Criteria Status

| Criterion | Status | Notes |
|---|---|---|
| {Criterion 1} | 〇 Pass / × Fail | |
| {Criterion 2} | 〇 Pass / × Fail | |
| `node --check` passes | 〇 Pass / × Fail | |
| `validate-store --dry-run` exits 0 | 〇 Pass / × Fail | |

## Plugin Checklist

- [ ] Version bumped in `forge/.claude-plugin/plugin.json` (if material change)
- [ ] Migration entry added to `forge/migrations.json` (if material change)
- [ ] Security scan run and report committed (if `forge/` was modified)

## Knowledge Updates

{Any updates written back to `engineering/architecture/` or `engineering/stack-checklist.md`}

## Notes

{Anything the reviewer should know}
