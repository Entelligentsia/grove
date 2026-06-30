---
name: repair
description: Diagnose and repair corrupted store records. Use when validate-store reports errors or store data seems wrong.
---

# /forge:repair

Diagnose and repair corruption in the Forge JSON store.

## Arguments

$ARGUMENTS

| Argument | Purpose |
|----------|---------|
| `--dry-run` | Show what would be repaired without making any changes |

## How to Run

1. Resolve the plugin root:
   ```
   FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
   ```

2. Run the four-phase repair workflow below. If `--dry-run` is in `$ARGUMENTS`,
   preview all changes without writing — skip Phase 2 writes and Phase 3
   writes, and skip Phase 4 verification since nothing changed.

## Phase 1: Diagnosis

Run the deterministic validator in JSON mode:

```sh
node "$FORGE_ROOT/tools/validate-store.cjs" --dry-run --json
```

Parse the JSON output. Categorize each error by its `category` field:

| Category | Example | Deterministic fix? |
|----------|---------|--------------------|
| `missing-required` | `"sprintId": null` on a sprint | Yes — backfill |
| `type-mismatch` | `"iteration": "1"` instead of `1` | Yes — coerce |
| `invalid-enum` | `"status": "in-progress"` on a bug | No — needs judgment |
| `undeclared-field` | `"priority": "high"` on a task | No — needs judgment |
| `orphaned-fk` | `taskId` pointing to deleted task in event | No — needs judgment |
| `filename-mismatch` | Event filename != eventId | Yes — rename |
| `minimum-violation` | `"iteration": 0` | Yes — coerce |
| `orphan-directory` | Sprint dir with no sprint record | No — needs judgment |
| `stale-path` | `path` pointing to nonexistent dir | No — needs judgment |

## Phase 2: Auto-Fix

Run the deterministic validator with `--fix`:

```sh
node "$FORGE_ROOT/tools/validate-store.cjs" --fix --json
```

For dry-run, preview without writing:

```sh
node "$FORGE_ROOT/tools/validate-store.cjs" --fix --dry-run --json
```

Report each fix to the user (entity, field, what was backfilled or nullified).

## Phase 3: LLM-Judgment Fixes

For each remaining error from Phase 1 that was not auto-fixed, apply LLM judgment:

1. **Read the corrupted record**:
   ```sh
   node "$FORGE_ROOT/tools/store-cli.cjs" read <entity> <id> --json
   ```

2. **Determine the correct value** based on schema definitions, store context, and
   the judgment rules below.

3. **Present the proposed fix** to the user with:
   - Entity ID and field affected
   - Current (corrupted) value
   - Proposed (corrected) value
   - Reasoning

   If the user declines, skip the fix and note it in the report.

4. **Apply the fix** using the appropriate command:

   | Corruption Pattern | Repair Action | Command |
   |-------------------|---------------|---------|
   | Invalid enum value | Map to nearest valid enum | `store-cli.cjs update-status <entity> <id> <field> <value> [--force]` |
   | Undeclared field | Remove the field by rewriting | `store-cli.cjs write <entity> '<fixed-json>'` |
   | Type mismatch | Coerce to correct type and rewrite | `store-cli.cjs write <entity> '<fixed-json>'` |
   | Orphaned FK | Nullify or remap to existing entity | `store-cli.cjs write <entity> '<fixed-json>'` |
   | Dangling sprint.taskIds | Remove taskIds referencing non-existent tasks | `store-cli.cjs write sprint '<fixed-json>'` |
   | Illegal status transition | Force-correct to valid state | `store-cli.cjs update-status <entity> <id> status <value> --force` |
   | Stale COLLATION_STATE | Regenerate via collate | `node "$FORGE_ROOT/tools/collate.cjs"` |

### Judgment Rules: Invalid Enum Values

Map common misspellings and variant forms to canonical values:

| Entity | Field | Common Misspellings | Canonical Values |
|--------|-------|--------------------|------------------|
| Sprint | status | "in-progress" | active |
| Sprint | status | "done", "finished" | completed |
| Task | status | "in-progress" | implementing |
| Task | status | "in-review", "review" | review-approved |
| Task | status | "done", "finished" | committed |
| Bug | severity | "high", "critical" | critical |
| Bug | severity | "medium" | major |
| Bug | severity | "low" | minor |
| Bug | status | "open" | reported |
| Bug | status | "working" | in-progress |
| Feature | status | "complete", "done" | shipped |

When no reasonable mapping exists, ask the user to choose from valid values.

### Judgment Rules: Orphaned References

- **Event.taskId → deleted task**: If the event's sprintId is valid, nullify the
  taskId. If the sprint also doesn't exist, ask the user whether to delete the
  event or create a stub sprint.
- **Sprint.taskIds containing deleted task IDs**: Remove the deleted IDs from the
  array and rewrite the sprint.
- **Undeclared fields**: Remove the field and rewrite. Mention the removed data in
  the report so the user can decide whether to add it to the schema.

### Hard Rules

1. **Never fall back to direct file writes.** All repairs go through
   `store-cli.cjs`. If it rejects a write, fix the data and retry (max 2).
2. **Never delete data without confirmation.** Ask the user before deleting any
   record.
3. **Never skip Phase 4 verification.** Always re-run validate-store after
   repairs.
4. **Preserve data priority.** Prefer corrections that retain more original data.
   Mapping "in-progress" → "implementing" is better than resetting to "draft".

## Phase 4: Verification

Re-run the validator to confirm the store is clean:

```sh
node "$FORGE_ROOT/tools/validate-store.cjs" --dry-run --json
```

If `"ok": true`, report success. If errors remain, report them as unresolved
with suggestions for next steps.

## Repair Report

After all phases, output a structured report:

```
# Store Repair Report

## Phase 1: Diagnosis
- Errors found: N
- Warnings found: M
- Categories: {breakdown by category}

## Phase 2: Auto-Fix
- Fixes applied: N
- Details: {list of each fix}

## Phase 3: LLM-Judgment Fixes
- Fixes proposed: N
- Fixes applied: M (with user approval)
- Fixes skipped: K (user declined)
- Details: {list of each proposed change with reasoning}

## Phase 4: Verification
- Errors remaining: N
- Warnings remaining: M
- Status: PASS / FAIL

## Unresolved Issues
{List of issues that could not be automatically resolved, with suggestions}
```

## On Error

If `validate-store.cjs` crashes or returns unexpected output, report the error
and suggest running `/forge:report-bug` if it appears to be a Forge bug. Do NOT
attempt to continue repairs after an unexpected error in a deterministic tool —
the store state is unknown and further writes could cause additional corruption.