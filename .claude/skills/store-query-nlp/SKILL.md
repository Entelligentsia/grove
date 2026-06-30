---
name: store-query-nlp
description: "Query the Forge store using natural language. Use when you need to find tasks, bugs, sprints, or features by intent — e.g. 'open bugs in S12', 'blocked tasks', 'tasks implementing the auth feature', 'how many bugs are critical'. Returns structured JSON with entity IDs, titles, statuses, relationships, file refs, and INDEX.md excerpts."
allowed-tools:
  - Bash
---

# forge:store-query-nlp

Translate natural language intent into a structured Forge store query.
Returns filtered, FK-resolved results with excerpts — replacing manual KB navigation.

**When to use:** anytime you need to find entities without knowing exact IDs, or
want filtered/sorted results from a sprint/feature/status. Replaces reading
MASTER_INDEX.md manually when a focused query is sufficient.

## Setup

Resolve the plugin root path before any invocation:

```sh
FORGE_ROOT=$(node -e "console.log(require('./.forge/config.json').paths.forgeRoot)")
```

## Invocation

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" nlp "<natural language query>"
```

Returns JSON to stdout. Exit 0 on success, 1 on error.

## Example Queries

| Intent | Example |
|--------|---------|
| Bugs in a sprint | `"open bugs in S12"` |
| Blocked tasks | `"blocked tasks"` |
| Tasks by status | `"implementing tasks in S14"` |
| Critical bugs | `"critical bugs"` |
| Tasks for a feature | `"tasks for FEAT-003"` |
| Sprint overview | `"sprint S12"` |
| Specific bug | `"WI-BUG-047"` |
| Specific task | `"WI-S12-T03"` |
| Latest sprint | `"latest sprint"` |
| Count query | `"how many open bugs"` |
| Blocking chain | `"bugs blocking tasks in S12"` |

## Output Structure

```json
{
  "query": "open bugs in S12",
  "path": "intent-nlp",
  "traversalTrace": ["..."],
  "results": [
    {
      "id": "WI-BUG-047",
      "title": "...",
      "status": "in-progress",
      "type": "bug",
      "relationships": { "sprintId": "S12", "blocksTask": ["WI-S12-T03"] },
      "fileRefs": { "json": ".forge/store/bugs/WI-BUG-047.json", "md": "engineering/bugs/..." },
      "excerpt": "First few sentences from INDEX.md..."
    }
  ],
  "relatedFileRefs": ["..."],
  "meta": { "mode": "auto", "engineVersion": "1.0.0", "totalTimeMs": 42 }
}
```

## Interpreting Results

- `traversalTrace` — explains the NLP parse path (entity detected, filters applied, FKs followed, confidence level)
- `fileRefs.md` — path to the INDEX.md for this entity; read it for full context
- `fileRefs.json` — path to the store JSON record; use with `store-cli read` for full data
- `excerpt` — first 4 sentences from the INDEX.md; often sufficient without a Read() call
- `relatedFileRefs` — flat list of all referenced files, ready for batch Read()
- `meta.totalTimeMs` — wall-clock query time; typical range 5–80ms

## Confidence Signals

Check `traversalTrace` for confidence:

- `plan confidence: high` — all filters validated against schema enums/patterns
- `plan confidence: low` — one or more filters were stripped as invalid; results may be broader than expected
- `overall confidence: low (required retry)` — primary query returned 0 results; retried as keyword-only search

When confidence is low, verify results or fall back to reading MASTER_INDEX.md.

## Mode Control

```sh
# Explicit NLP mode (default for nlp subcommand)
node "$FORGE_ROOT/tools/store-cli.cjs" nlp "open bugs in S12"

# Strict mode — exact flags only, no NLP (baseline comparison)
node "$FORGE_ROOT/tools/store-cli.cjs" query --mode strict --sprint S12 --status in-progress
```

## Fallback

If the query returns 0 results or low confidence, fall back to:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" list bug
# Then scan titles manually
```

Or read `engineering/MASTER_INDEX.md` directly.
