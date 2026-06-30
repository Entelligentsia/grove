# Store Schema: Sprint

## File Location

`.forge/store/sprints/{SPRINT_ID}.json`

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sprintId` | string | yes | e.g. `S01` |
| `title` | string | yes | Sprint title |
| `description` | string | no | Sprint goals (prose) |
| `goal` | string | no | Sprint goal (single-sentence summary) |
| `status` | enum | yes | See status values below |
| `taskIds` | string[] | yes | Ordered list of task IDs in this sprint |
| `features` | string[] | no | Feature IDs this sprint contributes to |
| `feature_id` | string\|null | no | Primary feature linkage (nullable FK) |
| `dependencies` | object | no | Task dependency edges for wave computation |
| `executionMode` | enum | no | `sequential` / `wave-parallel` / `full-parallel` |
| `createdAt` | string | yes | ISO 8601 timestamp |
| `completedAt` | string | no | ISO 8601 timestamp |
| `path` | string | no | Relative path to sprint artifact directory |
| `humanEstimates` | object | no | `{ total: "Xh", breakdown: { taskId: "Yh" } }` |

## Status Values

`planning` → `active` → `completed` → `retrospective-done`

Failed states: `blocked`, `partially-completed`, `abandoned`

## JSON Schema

This block is the canonical machine-readable definition embedded in `validate-store.cjs`.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "forge/sprint.schema.json",
  "title": "Sprint",
  "type": "object",
  "required": ["sprintId", "title", "status", "taskIds", "createdAt"],
  "properties": {
    "sprintId":       { "type": "string" },
    "title":          { "type": "string" },
    "description":    { "type": "string" },
    "status": {
      "type": "string",
      "enum": ["planning", "active", "completed", "retrospective-done", "blocked", "partially-completed", "abandoned"]
    },
    "goal":           { "type": "string" },
    "taskIds":        { "type": "array", "items": { "type": "string" } },
    "features":       { "type": "array", "items": { "type": "string" } },
    "feature_id":     { "type": ["string", "null"] },
    "dependencies":   { "type": "object" },
    "executionMode":  { "type": "string", "enum": ["sequential", "wave-parallel", "full-parallel"] },
    "createdAt":      { "type": "string", "format": "date-time" },
    "completedAt":    { "type": "string", "format": "date-time" },
    "path":           { "type": "string" },
    "humanEstimates": { "type": "object" }
  },
  "additionalProperties": false
}
```
