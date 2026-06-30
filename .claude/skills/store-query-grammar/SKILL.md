---
name: store-query-grammar
description: "Reference for the Forge NLP query grammar — entity synonyms, status synonyms, ordering tokens, FK phrases, and ID patterns. Use when constructing or debugging store-query-nlp queries, or to understand why a query returned unexpected results."
allowed-tools:
  - Bash
---

# forge:store-query-grammar

NLP grammar reference for the Forge store query engine.

The query engine (`store-cli.cjs nlp`) uses a 5-stage deterministic rule-based
parser — no LLM, no network. This skill documents the token vocabulary so you
can construct precise queries and debug unexpected parse results.

## Setup

Dump the live grammar for the current project (includes project-specific ID patterns):

```sh
FORGE_ROOT=$(node -e "console.log(require('./.forge/config.json').paths.forgeRoot)")
node "$FORGE_ROOT/tools/store-cli.cjs" schema
```

The `schema` command returns entity schemas, status enums, and the full grammar
reference for the installed project prefix.

## Parser Stages

### Stage 1 — ID Patterns (highest priority)

Matched first; consumed tokens are excluded from later stages.

| Pattern | Example | Resolves to |
|---------|---------|------------|
| `{PREFIX}-S##-T##` | `WI-S12-T03` | task filter |
| `{PREFIX}-BUG-###` | `WI-BUG-047` | bug filter |
| `FEAT-###` | `FEAT-003` | feature filter |
| `S##` | `S12` | sprint filter |
| `sprint N` | `sprint 12` | sprint filter → `S12` |

### Stage 2 — Entity Detection

The parser identifies the primary entity type from synonyms. First match wins.

| Entity | Synonyms |
|--------|---------|
| sprints | sprint, sprints, release, releases, iteration, iterations |
| tasks | task, tasks, item, items, work item, work items, todo, todos |
| bugs | bug, bugs, defect, defects, issue, issues, problem, problems |
| features | feature, features, epic, epics, capability, capabilities |

Bigrams are matched before unigrams (e.g. `work item` → tasks).

### Stage 3 — Status / Severity Filters

Maps natural-language status phrases to schema enum values per entity type.

| Input phrase | tasks | bugs | sprints | features |
|-------------|-------|------|---------|---------|
| open / active / in progress | implementing | in-progress | active | active |
| completed / done | committed | fixed | completed | shipped |
| fixed | — | fixed | — | — |
| planned / planning | planned | — | planning | — |
| implementing | implementing | — | — | — |
| implemented | implemented | — | — | — |
| committed | committed | — | — | — |
| draft | draft | — | — | draft |
| abandoned | abandoned | — | abandoned | — |
| retired | — | — | — | retired |
| shipped | — | — | — | shipped |
| triaged | — | triaged | — | — |
| reported | — | reported | — | — |
| blocked | blocked | — | — | — |

Severity (bugs only, sets `severity` field not `status`):

| Input | Severity |
|-------|---------|
| critical | critical |
| major | major |
| minor | minor |

### Stage 4 — FK Follow Phrases

Trigger FK traversal in the result set.

| Phrase | FK followed |
|--------|-----------|
| with sprint / sprint for / which sprint | `sprintId` |
| with feature / feature for | `featureId` |
| block / blocking / blocked | `blocksTask` (for bugs) or `blockedBy` (for tasks) |

### Stage 4b — Ordering and Limit Tokens

| Token(s) | Effect |
|----------|--------|
| latest / newest / recent / most recent | sort desc, limit 1 |
| oldest / earliest / first | sort asc, limit 1 |
| last | sort desc, limit 1 |
| top N / first N | sort desc/asc, limit N |
| last N | sort desc, limit N |
| how many / count of / number of / count | count mode (returns `{count: N}`) |

### Stage 5 — Keyword Extraction

Remaining tokens (not consumed by stages 1–4, not in stop words, length > 1)
become keyword match terms on the `title` field (word-boundary match).

Stop words include all entity synonyms plus: list, all, the, show, find, what,
which, are, in, for, about, related, to, of, and, with, details, status, how,
many, there, a, an, is, that, this, on, by, me, give, get, tell, please, can,
do, does, did, was, were, been, being, have, has, had, will, would, could,
should, may, might, blocking, blocked, block, severity, titles, title.

## Debugging Queries

If a query returns unexpected results, use `traversalTrace` in the output:

```json
"traversalTrace": [
  "intent parsed via NLP rules",
  "listed tasks with filter {\"status\":\"implementing\"}: 7 results",
  "sorted tasks desc",
  "limited to 3 (of 7)",
  "plan confidence: high"
]
```

Common debug patterns:

| Symptom | Likely cause |
|---------|-------------|
| 0 results, retried | Status phrase didn't map to a valid enum for the detected entity type |
| confidence: low | Filter value stripped — check the status mapping table above |
| Wrong entity type | Entity synonym matched a stop word before the intended synonym |
| Keyword matching too broadly | Term is short (≤1 char) or in stop words; try a more specific term |
| Missing FK traversal | FK phrase not in stage 4 vocabulary; use `--with-sprint` / `--with-feature` flags instead |

## Query Composition Tips

- Put entity type first: `"bugs in S12"` not `"S12 bugs"` — entity is detected in order
- Use exact ID when known: `"WI-BUG-047"` is faster and more precise than `"bug about auth"`
- Count before listing: `"how many open bugs"` before `"open bugs"` for large stores
- Combine stages naturally: `"critical open bugs in S12 blocking tasks"` uses stages 1, 2, 3, 4 together
