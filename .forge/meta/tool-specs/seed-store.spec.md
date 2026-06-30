# Tool Spec: seed-store

## Purpose

Bootstrap the JSON store from an existing `engineering/` directory structure.
Used when a project already has sprint/task artifacts but no JSON store.

## Inputs

- `.forge/config.json` — project prefix, paths
- `engineering/sprints/` — existing sprint directories
- `engineering/bugs/` — existing bug directories

## Outputs

- `.forge/store/sprints/*.json` — one per discovered sprint
- `.forge/store/tasks/*.json` — one per discovered task
- `.forge/store/bugs/*.json` — one per discovered bug

## CLI Interface

```
<tool> seed-store              # scan and create
<tool> seed-store --dry-run    # preview what would be created
```

Exit 0 on success, 1 on error.

## Output Field Schema

All JSON files use **camelCase** field names to match `validate-store`.

**Sprint** (`.forge/store/sprints/<ID>.json`):
```json
{ "sprintId": "PREFIX-S01", "title": "...", "status": "active" }
```

**Task** (`.forge/store/tasks/<ID>.json`):
```json
{ "taskId": "PREFIX-S01-1", "sprintId": "PREFIX-S01", "title": "...", "status": "planned" }
```

**Bug** (`.forge/store/bugs/<ID>.json`):
```json
{ "bugId": "PREFIX-B01", "title": "...", "severity": "medium", "status": "reported" }
```

Status defaults: sprint → `"active"`, task → `"planned"`, bug → `"reported"`.
Infer a better status from artifact presence when possible (e.g. PROGRESS.md
with "committed" → `"committed"`).

## Error Handling

- Wrap the entire entry point in a top-level exception handler.
- On unexpected errors (missing config, unreadable directories, unhandled
  exceptions), print a clear one-line message to stderr and exit 1.
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

1. Read `.forge/config.json` for prefix and paths
2. Scan `engineering/sprints/` for sprint directories (pattern: S{NN})
3. For each sprint directory:
   a. Create sprint JSON using the camelCase schema above
   b. Scan for task directories (pattern: T{NN})
   c. For each task directory:
      - Read PLAN.md, PROGRESS.md if they exist
      - Extract title, status from artifact content
      - Create task JSON using the camelCase schema above
4. Scan `engineering/bugs/` for bug directories
5. For each bug directory:
   a. Read available artifacts
   b. Create bug JSON using the camelCase schema above
6. Report: N sprints, N tasks, N bugs seeded
