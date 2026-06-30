# Fragment: store-cli Verb Cheat-Sheet

<!-- Canonical store-cli verb list. Referenced by meta workflows that issue
     store-cli calls. Surface it inline near the first store-cli invocation
     so subagents stop inventing REST-style verbs (`get`, `set`, `delete`)
     when they have to improvise a follow-up call. See forge#95 and
     FORGE-S22-T02 (read-aliases). -->

store-cli verbs: `read` | `list` | `write` | `emit` | `update-status` | `set-summary` | `set-bug-summary` | `describe` | `template` | `nlp` | `query` | `delete`

Read-aliases (FORGE-S22-T02): `get` | `get-task` | `get-bug` | `get-sprint` | `get-summary` | `get-bug-summary`

Notes for subagents:

- **`read`** is the canonical "fetch one record" verb. The aliases
  `get <entity> <id>`, `get-task <id>`, `get-bug <id>`, `get-sprint <id>`
  are accepted and delegate byte-equally to `read`. Prefer the canonical
  `read` form in new code; aliases exist to reduce friction when an agent
  reaches for REST-style verbs.
- **`get-summary <taskId> <phase>`** and **`get-bug-summary <bugId> <phase>`**
  are direct summary readers — they extract `record.summaries[phase]` and
  exit 1 if the phase is absent. They are NOT write verbs (do not confuse
  with `set-summary` / `set-bug-summary`).
- **`list`** filters by entity (`sprint`, `task`, `bug`, `event`, `feature`)
  and optional flags. There is no `find` or `search` — use `nlp` for
  natural-language lookup.
- **`update-status`** is the ONLY supported task/bug status mutation path.
  Do not `write` a task back with a new `status` field; the FSM is enforced
  by `update-status`. Syntax requires the field keyword `status` as the third
  argument — four args total:
  `forge_store({ command: "update-status", args: ["task", "{taskId}", "status", "{value}"] })`
  The three-arg form (omitting the `status` keyword between the id and the value) is WRONG and will
  error. Always include `status` between the id and the value.
- **`emit`** appends an event. There is no `append-event` / `add-event`.
- **`set-summary <id> <phase>`** / **`set-bug-summary <id> <phase>`** link a
  phase summary onto the entity record. The JSON-file argument is **optional**:
  when omitted, the sidecar is auto-resolved from the record's `path` plus the
  canonical phase→filename map (so `set-summary {taskId} validation` just works).
  Never pass a hand-built `engineering/sprints/.../…-SUMMARY.json` path. Do not
  inline summaries into the entity via `write`.
  - **`forge_store` tool shape (not CLI flags).** The tool has exactly two
    fields: `command` (string) and `args` (positional string array). There is
    NO `entity` / `id` / `phase` named field — passing them silently drops them.
    The summary call is `forge_store({ command:"set-summary", args:["<id>", "<phase>"] })`
    where `args[0]` is the record id and `args[1]` is the LITERAL phase key
    (`plan`, `review_plan`, `implementation`, `code_review`, `validation`,
    `triage`, `approve`). `args[1]` is NEVER the record id and NEVER a path —
    putting the id in both slots is the canonical failure and the phase-ownership
    guard rejects it with `expected summary key '<phase>'`.
- **Artifact I/O:** Use `forge_artifact` for ALL phase artifact reads and writes
  (PLAN.md, PROGRESS.md, *-SUMMARY.json, CODE_REVIEW.md, etc.). Never construct
  artifact file paths manually — the tool resolves paths from entity IDs and
  validates JSON summary schemas on write. After writing a summary JSON via
  `forge_artifact`, link it to the store record via `forge_store set-summary {id} {phase}` (no path).
  Example: `forge_artifact({ command:"write", entity:"task", entityId:"{taskId}", artifact:"progress", content:"..." })`
- **Artifact addressing (canonical) — never reconstruct a path.** Address an
  artifact by `(entity, entityId, kind)` via `forge_artifact`, or read the
  entity's `path` field from the store record. The on-disk directory is owned by
  the record's `path`, NOT by any id template. Token glossary:
  - `{sprintId}` / `{taskId}` / `{bugId}` — the **store record filenames**
    (`.forge/store/<kind>s/<id>.json`); deterministic and safe to use as IDs.
  - `{sprint}` / `{task}` / `{bug}` — runtime path-template substitutions used by
    the **preflight gate**, derived from the record's `path` (not the bare ID).
  - The engineering artifact directory always comes from `record.path`.
  These spellings are parsed literally by tools (`preflight-gate.cjs`,
  `collate.cjs`) — do not invent new spellings or rename them in prose.
- If you need a verb not on this list, run
  `forge_store({ command: "--help" })` before improvising.
- If you supply an unknown verb, entity type, enum value, or field name,
  store-cli appends a **Did you mean?** suggestion to the error message.
  Suggestions use Levenshtein distance (≤ 2) and a curated drift map for
  common agent misconceptions (e.g., `completed` → `committed`,
  `task` → `taskId`, `set` → `set-summary`). See FORGE-S22-T03.
