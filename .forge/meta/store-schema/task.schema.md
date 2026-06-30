# Store Schema: Task

## File Location

`.forge/store/tasks/{TASK_ID}.json`

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `taskId` | string | yes | e.g. `ACME-S01-T01` |
| `feature_id` | string\|null | no | Primary feature linkage (nullable FK) |
| `sprintId` | string | yes | e.g. `S01` |
| `title` | string | yes | Task title |
| `description` | string | no | Detailed description |
| `status` | enum | yes | See status values below |
| `path` | string | yes | Relative path to task artifact directory |
| `estimate` | enum | no | `S` / `M` / `L` / `XL` |
| `dependencies` | string[] | no | Task IDs this task depends on |
| `knowledgeUpdates` | object[] | no | Files updated during writeback |
| `planIterations` | integer | no | Number of plan review loops |
| `codeReviewIterations` | integer | no | Number of code review loops |
| `assignedModel` | string | no | Model used for implementation |
| `pipeline` | string | no | Named pipeline to use instead of `default`. Must match a key in `config.pipelines`. When absent, the orchestrator uses the `default` pipeline. |

## Status Values

`draft` → `planned` → `plan-approved` → `implementing` → `implemented` → `review-approved` → `approved` → `committed`

Failed states: `plan-revision-required`, `code-revision-required`, `blocked`, `escalated`, `abandoned`

## Knowledge Updates Schema

```json
{
  "file": "architecture/stack.md",
  "section": "Celery Patterns",
  "type": "addition | correction | removal"
}
```

## JSON Schema

This block is the canonical machine-readable definition embedded in `validate-store.cjs`.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "forge/task.schema.json",
  "title": "Task",
  "type": "object",
  "required": ["taskId", "sprintId", "title", "status", "path"],
  "properties": {
    "taskId":               { "type": "string" },
    "feature_id":           { "type": ["string", "null"] },
    "sprintId":             { "type": "string" },
    "title":                { "type": "string" },
    "description":          { "type": "string" },
    "status": {
      "type": "string",
      "enum": [
        "draft", "planned", "plan-approved", "implementing",
        "implemented", "review-approved", "approved", "committed",
        "plan-revision-required", "code-revision-required", "blocked", "escalated", "abandoned"
      ]
    },
    "path":                 { "type": "string" },
    "estimate":             { "type": "string", "enum": ["S", "M", "L", "XL"] },
    "dependencies":         { "type": "array", "items": { "type": "string" } },
    "knowledgeUpdates":     { "type": "array" },
    "planIterations":       { "type": "integer", "minimum": 0 },
    "codeReviewIterations": { "type": "integer", "minimum": 0 },
    "assignedModel":        { "type": "string" },
    "pipeline":             { "type": "string" }
  },
  "additionalProperties": false
}
```
