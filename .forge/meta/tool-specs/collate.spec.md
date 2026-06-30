# Tool Spec: collate

## Purpose

Regenerate markdown views from the JSON store. Deterministic — no AI needed.

## Inputs

- `.forge/config.json` — project prefix, paths, description
- `.forge/store/sprints/*.json`
- `.forge/store/tasks/*.json`
- `.forge/store/bugs/*.json`
- `.forge/store/features/*.json`
- `.forge/store/events/{SPRINT_ID}/*.json`
- Existing `engineering/MASTER_INDEX.md` (to preserve static sections)

## Outputs

1. `engineering/MASTER_INDEX.md`
2. `engineering/sprints/{SPRINT_ID}/TIMESHEET.md` (per sprint)
3. `engineering/bugs/TIMESHEET.md`
4. `engineering/features/INDEX.md` (feature registry)
5. `engineering/features/{FEATURE_ID}/INDEX.md` (per-feature page)
6. `INDEX.md` (per sprint, task, bug directory)
7. `.forge/store/COLLATION_STATE.json`

## CLI Interface

```
<tool> collate              # all sprints
<tool> collate S01          # single sprint + master index
<tool> collate --dry-run    # preview only
```

Exit 0 on success, 1 on validation error.

## Error Handling

- Wrap the entire entry point in a top-level exception handler.
- On unexpected errors (missing store files, bad JSON, unhandled exceptions),
  print a clear one-line message to stderr and exit 1.
- Never let the tool crash with an unhandled exception or stack trace visible
  to the caller — all errors are caught and reported cleanly.
- Python pattern:
  ```python
  if __name__ == "__main__":
      try:
          sys.exit(main())
      except Exception as e:
          print(f"Error: {e}", file=sys.stderr)
          sys.exit(1)
  ```
- JS/TS pattern:
  ```js
  process.on('uncaughtException', (e) => {
      process.stderr.write(`Error: ${e.message}\n`);
      process.exit(1);
  });
  ```

## Algorithm

1. Read `.forge/config.json` for prefix, paths, project description
2. Validate store: tasks/ has JSON files, required fields present
3. Load all sprint JSON, sort by sprint number ascending
4. Load all task JSON, group by sprintId
5. Load all bug JSON, sort by bug number
6. Load all feature JSON from `.forge/store/features/`
7. Read existing `engineering/MASTER_INDEX.md`, extract preserved sections by `##` heading
8. Build Feature Registry section: link to `features/INDEX.md` (or placeholder if none)
9. Build Sprint Registry: table with progress (completed/total)
10. Build Task Registry: grouped by sprint (most recent first)
11. Build Bug Registry: open first (asc), then resolved (desc)
12. Write `engineering/MASTER_INDEX.md`: config header → preserved → generated
    - Generated sections order: Feature Registry → Sprint Registry → Task Registry → Bug Registry
13. Write `engineering/features/INDEX.md`: table of all features (ID, title, status, sprint count, task count)
14. For each feature: write `engineering/features/{FEATURE_ID}/INDEX.md` with detail page
15. For each sprint: events → estimates table + activity log → TIMESHEET.md
16. For each directory: discover artifacts → INDEX.md navigation hub
17. Write `.forge/store/COLLATION_STATE.json`

## Formatting Rules

- Markdown pipe tables
- Timestamps truncated to minutes
- Duration: <60m → "Nm", >=60m → "Nh Mm"
- IDs hyperlink to INDEX.md via relative paths
- Generated files start with `<!-- GENERATED -->` comment
