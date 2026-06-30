# Tool Spec: store-cli

## Purpose

Deterministic store custodian CLI — the sole authorized gateway for the
probabilistic layer to read and write the JSON store at `.forge/store/`.
Wraps `store.cjs` facade, enforces schema validation on every write, and
enforces status transition rules on `update-status`. Deterministic — no AI
needed.

## Inputs

- `.forge/config.json` — project paths and prefix
- `.forge/schemas/*.schema.json` — canonical JSON Schema files (primary source)
- `forge/schemas/*.schema.json` — in-tree source schemas (fallback for dogfooding)
- CLI arguments — command, entity type, JSON payload, flags

## Outputs

- Entity records written to `.forge/store/` (via `store.cjs` facade)
- Event records written to `.forge/store/events/{sprintId}/`
- Sidecar files written to `.forge/store/events/{sprintId}/_{eventId}_usage.json`
- `COLLATION_STATE.json` written to `.forge/store/`
- JSON results to stdout on success
- Per-field error messages to stderr on failure
- Exit 0 on success, 1 on failure

## CLI Interface

```
<tool> store-cli write <entity> '<json>'                     Write a full entity record
<tool> store-cli read <entity> <id> [--json]                 Read an entity record
<tool> store-cli list <entity> [key=value ...]               List entities with optional filter
<tool> store-cli delete <entity> <id>                        Delete an entity record
<tool> store-cli update-status <entity> <id> <field> <value> [--force]
                                                             Update status/enum field with transition check
<tool> store-cli emit <sprintId> '<json>' [--sidecar]       Write an event (or sidecar)
<tool> store-cli merge-sidecar <sprintId> <eventId>          Merge sidecar into canonical event
<tool> store-cli purge-events <sprintId>                     Delete all events for a sprint
<tool> store-cli write-collation-state '<json>'              Write COLLATION_STATE.json
<tool> store-cli validate <entity> '<json>'                  Validate against schema without writing
<tool> store-cli nlp '<intent>'                              Query store by natural language intent (NLP)
<tool> store-cli query [--sprint|--task|--bug|--feature <id>] [--status <s>]
                       [--keyword <term>] [--type <entity>] [flags]
                                                             Query store by exact flags or intent
<tool> store-cli query --mode strict|nlp|off [flags]        Explicit mode control (strict=exact flags only)
<tool> store-cli schema                                      Dump entity schemas, status enums, NLP grammar
```

Entity types: `sprint`, `task`, `bug`, `event`, `feature`

Flags:
- `--dry-run` — validate and preview without writing (applies to all write commands)
- `--force` — bypass transition check on `update-status` (emits warning)
- `--json` — output raw JSON on `read` (no pretty-print)
- `--sidecar` — write as sidecar file on `emit` (ephemeral, `_`-prefixed)
- `--sprint <id>` — filter query by sprint ID (e.g. `S12`)
- `--task <id>` — query a specific task by ID
- `--bug <id>` — query a specific bug by ID
- `--feature <id>` — query a specific feature by ID
- `--status <value>` — filter query by status value
- `--keyword <term>` — keyword search on entity titles
- `--type <entity>` — restrict `--keyword` to `sprints|tasks|bugs|features`
- `--with-blockers` — follow `blockedBy` FK on tasks
- `--with-blocked-tasks` — follow `blocksTask` FK on bugs
- `--with-sprint` — follow `sprintId` FK on results
- `--with-feature` — follow `featureId` FK on results
- `--no-excerpts` — omit INDEX.md excerpts from results
- `--mode strict|nlp|off` — engine mode (`strict`/`off` = exact flags only; `nlp` = intent parse)

Exit codes: 0 on success, 1 on failure.

## Entity Types

| Entity | ID Field | Required Fields (minimal) | Store Path |
|--------|----------|---------------------------|------------|
| sprint | `sprintId` | `sprintId`, `title`, `status`, `taskIds`, `createdAt` | `.forge/store/sprints/{sprintId}.json` |
| task | `taskId` | `taskId`, `sprintId`, `title`, `status`, `path` | `.forge/store/tasks/{taskId}.json` |
| bug | `bugId` | `bugId`, `title`, `severity`, `status`, `path`, `reportedAt` | `.forge/store/bugs/{bugId}.json` |
| event | `eventId` | `eventId`, `taskId`, `sprintId`, `role`, `action`, `phase`, `iteration`, `startTimestamp`, `endTimestamp`, `durationMinutes`, `model` | `.forge/store/events/{sprintId}/{eventId}.json` |
| feature | `id` | `id`, `title`, `status`, `created_at` | `.forge/store/features/{id}.json` |

Note: `feature` uses `id` (not `feature_id`) as its primary key. The
`feature_id` field on sprint/task is a foreign key pointing to `feature.id`.

Nullable fields (accepted as `null` without error): `sprintId`, `taskId`,
`endTimestamp`, `durationMinutes`, `feature_id`, `description`, `completedAt`,
`resolvedAt`.

## Schema Validation

The CLI validates every write payload against the canonical JSON Schema before
writing. Schemas are loaded from:

1. `.forge/schemas/{type}.schema.json` (project-installed, primary)
2. `forge/schemas/{type}.schema.json` (in-tree source, for dogfooding)
3. Minimal built-in fallback (required fields only, with stderr warning)

Validation rules:
- All `required` fields must be present and non-null (except nullable fields)
- Field types must match schema declarations (including multi-type)
- Enum values must be in the declared set
- `additionalProperties: false` — reject records with undeclared fields
- `minimum` constraints enforced for numeric fields

On validation failure: exit 1, one error per line on stderr (prefixed with
field name), no partial write.

## Status Transitions

The `update-status` command enforces legal state transitions. Illegal
transitions are rejected (exit 1 with `"Illegal transition: ..."` on stderr).
Use `--force` to bypass (emits warning).

### Task

```
draft -> planned -> plan-approved -> implementing -> implemented
      -> review-approved -> approved -> committed

Failed states (enterable from any non-terminal state):
  plan-revision-required, code-revision-required, blocked, escalated, abandoned

Terminal states (no transitions out):
  committed, abandoned
```

### Sprint

```
planning -> active -> completed -> retrospective-done

Failed states:
  blocked, partially-completed, abandoned

Terminal states:
  retrospective-done, abandoned
```

### Bug

```
reported -> triaged -> in-progress -> fixed

Terminal state:
  fixed
```

The architect-approve verdict for bugs travels through
`bug.summaries.approve.verdict` (read by `read-verdict.cjs §
BUG_PHASE_VERDICT_SOURCE`), not `bug.status`. Earlier revisions of this
spec listed `approved` and `verified` between `fixed` and the terminal;
they were removed because no workflow phase wrote them and their presence
in the enum invited LLM-translated task workflows to attempt
`update-status bug ... approved` — the trap that produced FORGE-BUG-002.

### Feature

```
draft -> active -> shipped / retired

Terminal states:
  shipped, retired
```

## Sidecar Pattern

Events support a sidecar mechanism for passing out-of-band data from subagents
back to the orchestrator.

### `emit --sidecar`

Writes a `_{eventId}_usage.json` ephemeral file alongside the canonical event.
Sidecar files require only an `eventId` field at minimum. Accepted fields:

| Field | Notes |
|-------|-------|
| `eventId` | Required — matches the canonical event |
| `inputTokens` | Token count |
| `outputTokens` | Token count |
| `cacheReadTokens` | Cache hit tokens |
| `cacheWriteTokens` | Cache miss tokens (alias: `cacheCreationTokens`) |
| `estimatedCostUSD` | Cost estimate (alias: `cost`) |
| `model` | Full model identifier |
| `durationMinutes` | Decimal minutes |
| `startTimestamp` | ISO 8601 |
| `endTimestamp` | ISO 8601 |
| `tokenSource` | `reported` or `estimated` |

Alias mapping at merge time: `cacheCreationTokens` -> `cacheWriteTokens`,
`cost` -> `estimatedCostUSD`.

### `merge-sidecar`

Reads the sidecar, merges token fields into the canonical event via
`store.writeEvent()`, and deletes the sidecar file. Fails if either file is
missing.

## Error Handling

- Wrap the entire entry point in a top-level exception handler.
- On unexpected errors (missing store files, bad JSON, unhandled exceptions),
  print a clear one-line message to stderr and exit 1.
- Never let the tool crash with an unhandled exception or stack trace visible
  to the caller — all errors are caught and reported cleanly.
- JS/TS pattern:
  ```js
  process.on('uncaughtException', (e) => {
      process.stderr.write(`Error: ${e.message}\n`);
      process.exit(1);
  });
  ```

## Algorithm

### `write`
1. Parse entity type and JSON payload from CLI args.
2. Validate entity type is one of: sprint, task, bug, event, feature.
3. Parse JSON payload (exit 1 on parse error).
4. Validate payload against schema for the entity type.
5. If `--dry-run`, report what would be written and exit 0.
6. Write entity via `store.write{Entity}()` facade.

### `read`
1. Parse entity type and ID from CLI args.
2. For events, scan sprint directories to locate the event file.
3. Output the record (pretty-print or `--json` raw).
4. Exit 1 if entity not found.

### `list`
1. Parse entity type and optional key=value filter pairs.
2. List entities via `store.list{Entities}()` with filter.
3. Output JSON array.

### `update-status`
1. Read the current record via `store.get{Entity}()`.
2. Check that `current[field] -> new` is a legal transition.
3. If `--force`, bypass with warning on stderr.
4. Apply the update and write back via `store.write{Entity}()`.
5. Output JSON result with `from`/`to` fields.

### `emit`
1. Parse sprintId and JSON payload.
2. If `--sidecar`: write `_{eventId}_usage.json` file directly.
3. If canonical: validate against event schema, write via `store.writeEvent()`.
4. Output JSON result.

### `merge-sidecar`
1. Read sidecar file. Read canonical event file.
2. Merge token fields from sidecar into event (with alias resolution).
3. Write updated event via `store.writeEvent()`.
4. Delete sidecar file.

### `validate`
1. Parse entity type and JSON payload.
2. Validate against schema (same logic as `write`).
3. Exit 1 on errors (no write), exit 0 with `{"ok":true,"valid":true}`.

### `nlp '<intent>'`
1. Spawn `store-query.cjs nlp` with the intent string.
2. Output JSON to stdout: `{query, path, traversalTrace, results, relatedFileRefs, meta}`.
3. `results[]` contains: `{id, title, status, type, relationships, fileRefs, excerpt}`.

### `query [flags|intent]`
1. If exact entity flags present (`--sprint/--task/--bug/--feature`), run exact-args path.
2. If `--keyword` present, run keyword search path.
3. If intent string present (no flags), run NLP path.
4. `--mode strict|off` rejects intent strings; `--mode nlp` forces NLP path.
5. Returns same JSON structure as `nlp`.

### `schema`
1. Load project config (prefix, paths).
2. Return entity schemas, status/severity enums, FK relationships, and NLP grammar vocabulary.
3. Output: `{project, entities, entitySynonyms, statusSynonyms, grammar}`.

## Query Output Schema

```json
{
  "query": "<input string>",
  "path": "exact | keyword | intent-nlp",
  "traversalTrace": ["<step descriptions>"],
  "results": [
    {
      "id": "<entity ID>",
      "title": "<entity title>",
      "status": "<current status>",
      "type": "task | bug | sprint | feature",
      "relationships": {
        "sprintId": "<sprint ID>",
        "featureId": "<feature ID>",
        "blockedBy": ["<bug IDs>"],
        "blocksTask": ["<task IDs>"]
      },
      "fileRefs": {
        "json": "<store JSON path>",
        "md": "<INDEX.md path>"
      },
      "storeRef": "<store JSON path>",
      "indexRef": "<INDEX.md path>",
      "excerpt": "<first 4 sentences from INDEX.md, or null>"
    }
  ],
  "relatedFileRefs": ["<all md and json paths>"],
  "totalMatched": 7,
  "returned": 3,
  "limit": 3,
  "sort": "desc",
  "meta": {
    "mode": "auto | strict | nlp | off",
    "engineVersion": "1.0.0",
    "totalTimeMs": 42
  }
}
```

Count mode response (when intent contains `how many`, `count of`, etc.):

```json
{
  "query": "how many open bugs",
  "path": "intent-nlp",
  "traversalTrace": ["..."],
  "count": 7,
  "results": [],
  "totalMatched": 7
}
```