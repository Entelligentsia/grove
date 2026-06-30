# Store Schema: Feature

## File Location

`.forge/store/features/{FEATURE_ID}.json`

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Format: `FEAT-NNN` |
| `title` | string | yes | Feature title |
| `description` | string | no | Feature description |
| `status` | enum | yes | See status values below |
| `requirements` | string[] | no | Plain-text requirement lines |
| `sprints` | string[] | no | Array of sprint IDs (back-refs) |
| `tasks` | string[] | no | Array of task IDs (back-refs) |
| `created_at` | string | yes | ISO date-time string |
| `updated_at` | string | no | ISO date-time string |

## Relationship Notes

- The fields `description`, `requirements`, `sprints`, `tasks`, and `updated_at` are **optional**.
- The `id`, `title`, `status`, and `created_at` fields are **required**.
- `feature_id` on sprint and task manifests is nullable for backwards compatibility.

## Status Values

```mermaid
stateDiagram-v2
    [*] --> draft
    draft --> active
    active --> shipped
    active --> retired
    shipped --> retired
```

## JSON Schema

This block is the canonical machine-readable definition embedded in `validate-store.cjs`.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "forge/feature.schema.json",
  "title": "Feature",
  "type": "object",
  "required": ["id", "title", "status", "created_at"],
  "properties": {
    "id":           { "type": "string" },
    "title":        { "type": "string" },
    "description":  { "type": "string" },
    "status": {
      "type": "string",
      "enum": ["draft", "active", "shipped", "retired"]
    },
    "requirements": { "type": "array", "items": { "type": "string" } },
    "sprints":      { "type": "array", "items": { "type": "string" } },
    "tasks":        { "type": "array", "items": { "type": "string" } },
    "created_at":   { "type": "string", "format": "date-time" },
    "updated_at":   { "type": "string", "format": "date-time" }
  },
  "additionalProperties": false
}
```
