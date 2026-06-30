# Tool Spec: validate-store

## Purpose

Check store integrity: required fields present, referential integrity
between sprints/tasks/bugs/events, no orphaned records.

## Inputs

- `.forge/config.json` — paths
- `.forge/schemas/` — JSON Schema files (task, event, sprint, bug, feature); written during init Phase 8
- `.forge/store/` — all JSON files including `features/`

## Outputs

- Validation report to stdout (text or JSON)
- Exit 0 if valid, 1 if errors found

## CLI Interface

```
<tool> validate-store                     # full validation (text output)
<tool> validate-store --dry-run           # same checks, just report (text)
<tool> validate-store --fix               # auto-fix where possible
<tool> validate-store --fix --dry-run     # preview fixes without writing
<tool> validate-store --json              # JSON structured output
<tool> validate-store --dry-run --json    # JSON diagnosis, no fixes
<tool> validate-store --fix --dry-run --json  # JSON preview of fixes, no writes
<tool> validate-store --fix --json        # JSON output with fixes applied
```

## JSON Output Mode (`--json`)

When `--json` is present, the tool emits a single JSON object to stdout instead
of human-readable text. The JSON structure:

```json
{
  "ok": true,
  "errors": [
    {
      "entity": "sprint",
      "id": "FORGE-S07",
      "category": "invalid-enum",
      "field": "status",
      "message": "field \"status\": value \"in-progress\" not in [planning, active, ...]",
      "value": "in-progress",
      "expected": ["planning", "active", "completed", ...]
    }
  ],
  "warnings": [
    {
      "entity": "sprint",
      "id": "FORGE-S03",
      "category": "orphan-directory",
      "field": null,
      "message": "directory \"FORGE-S03-lean-migration\" has no sprint record in store"
    }
  ],
  "fixes": [
    {
      "entity": "sprint",
      "id": "FORGE-S01",
      "category": "backfill",
      "field": "createdAt",
      "message": "backfilled \"createdAt\" = \"2026-01-15T10:00:00.000Z\"",
      "applied": true
    }
  ],
  "summary": {
    "sprints": 7,
    "tasks": 50,
    "bugs": 4,
    "features": 1,
    "errors": 3,
    "warnings": 2,
    "fixes": 1
  }
}
```

### Error Categories

| Category | Description |
|----------|-------------|
| `missing-required` | Required field is null, undefined, or empty string |
| `type-mismatch` | Field value has wrong JSON type (e.g., string where number expected) |
| `invalid-enum` | Field value is not in the declared enum set |
| `undeclared-field` | Field not in schema with `additionalProperties: false` |
| `orphaned-fk` | Foreign key references a non-existent entity |
| `filename-mismatch` | Event filename does not match its eventId |
| `minimum-violation` | Numeric value below declared minimum |
| `orphan-directory` | Filesystem directory has no corresponding store record |
| `stale-path` | `path` field references a nonexistent directory |
| `missing-optional` | Optional field is absent (warning, not error) |

### Fix Categories

| Category | Description |
|----------|-------------|
| `backfill` | Missing field filled with a derived default |
| `orphaned-fk` | Orphaned foreign key nullified |
| `filename-mismatch` | Event file renamed or eventId corrected |

The `applied` field on fixes is `true` when the fix was actually written to disk,
`false` when `--dry-run` is active (preview only).

### `--fix --dry-run` Combination

When both `--fix` and `--dry-run` are specified, the tool previews what fixes
*would* be applied without writing anything. Each fix in the JSON output has
`applied: false`. This is useful for the `/forge:store:repair` skill to assess
what auto-fixes are available before applying them.

## Validation Rules

### Required Fields

Load the JSON Schema files from `.forge/schemas/` at runtime and derive required
fields from each schema's `"required"` array. Do NOT hardcode field names.

If a schema file is missing, fall back to these defaults and warn:
- Sprint: `sprintId`, `title`, `status`
- Task: `taskId`, `sprintId`, `title`, `status`, `path`
- Bug: `bugId`, `title`, `severity`, `status`, `path`, `reportedAt`
- Event: `eventId`, `sprintId`, `role`, `action`, `startTimestamp`, `endTimestamp`, `durationMinutes`

When schemas are present, also validate field types and enum values per the
schema definitions — not just field presence.

### Nullable Foreign Keys

`sprintId` and `taskId` are nullable foreign keys — `null` means "not associated"
(e.g. standalone bug fix with no sprint, sprint-level event with no task).
The validator must accept `null` for these fields without reporting an error.

`feature_id` on sprint and task records is a nullable FK pointing to
`.forge/store/features/{FEATURE_ID}.json`. A `null` value is valid (the record is
not associated with any feature). A non-null value must match the `id` field of an
existing feature record.

### Referential Integrity
- Every task.sprintId references an existing sprint (when non-null)
- Every event.taskId references an existing task OR bug (when non-null)
- Every event.sprintId references an existing sprint OR matches the parent directory name (virtual sprint dirs like `events/bugs/`, `events/ops/`)
- Every bug.similarBugs[] references existing bugs
- Every sprint.feature_id and task.feature_id references an existing feature (when non-null)

### Orphan Detection
- Task directories in `engineering/sprints/` without corresponding store JSON
- Store JSON without corresponding artifact directory

### Status Consistency
- Task status matches artifact presence (e.g., `committed` tasks should have all artifacts)
- Sprint status consistent with task statuses

### Undeclared Fields

When a schema has `additionalProperties: false`, any field not in the schema's
`properties` is reported as an error with category `undeclared-field`. This
catches fields that were valid in older schema versions but are no longer accepted.

## Error Handling

- Wrap the entire entry point in a top-level exception handler.
- On unexpected errors (missing files, JSON parse failures, unhandled
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

## Auto-Fix Rules (--fix mode)
- Add missing optional fields with defaults
- Create missing `.gitkeep` files in empty directories
- Nullify orphaned `feature_id` references on sprint and task records (log each fix)
- Do NOT delete orphaned records — only report them
