---
name: search
description: Query the Forge store by natural language or exact flags. Use for finding tasks, bugs, sprints, or features without manually reading the knowledge base.
allowed-tools:
  - Bash
---

# /forge:search

Query the Forge JSON store by intent, sprint, entity ID, or keyword.

## Locate plugin root

```
FORGE_ROOT: !`node -e "console.log(require('./.forge/config.json').paths.forgeRoot)"`
```

## Dispatch

If `$ARGUMENTS` starts with `--` (flags), run exact query:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" query $ARGUMENTS
```

Otherwise run NLP intent query:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" nlp "$ARGUMENTS"
```

If `$ARGUMENTS` is empty, print usage:

```
Usage: /forge:search <intent or flags>

Examples:
  /forge:search open bugs in S12
  /forge:search WI-BUG-047
  /forge:search --sprint S12 --status in-progress
  /forge:search --keyword auth
  /forge:search schema
```

## Output

Results are printed as JSON. Key fields:

| Field | Meaning |
|-------|---------|
| `results[].id` | Entity ID |
| `results[].title` | Entity title |
| `results[].status` | Current status |
| `results[].type` | `task`, `bug`, `sprint`, or `feature` |
| `results[].relationships` | FK IDs (sprintId, featureId, blockedBy, etc.) |
| `results[].fileRefs.md` | Path to INDEX.md for this entity |
| `results[].excerpt` | First 4 sentences from INDEX.md |
| `traversalTrace` | NLP parse steps and confidence |
| `meta.totalTimeMs` | Query wall-clock time |

## Schema reference

To see the project's entity schemas and NLP grammar vocabulary:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" schema
```

## Skills

- `forge:store-query-nlp` — NLP intent reference and output schema
- `forge:store-query-grammar` — Token vocabulary for constructing queries
- `forge:store-custodian` — Write/mutate operations (separate from query)
