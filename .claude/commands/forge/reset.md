---
name: reset
description: Rewind a halted/blocked task, bug, or sprint to an earlier pipeline phase — a guardrailed, integrity-checked state reset. Use when a pipeline halted and you need to resume from a chosen phase, or to reopen committed work for rework.
allowed-tools:
  - Bash
---

# /forge:reset

Guardrailed, natural-language **pipeline-state reset**. Rewinds a task, bug, or
sprint to an earlier phase so its pipeline can be resumed — after computing and
**confirming the cross-entity referential-integrity implications**. This never
mutates the store until you explicitly confirm the proposed plan.

> Deterministic counterpart: `4ge reset <id> --to <phase>` (forge-cli) does a
> single-entity task/bug reset without the NLP/integrity layer. This command
> adds intent parsing, sprint cascades, and the integrity gate.

## Arguments

`$ARGUMENTS` is a free-text request, e.g.:
- "put FORGE-S32-T03 back to implement"
- "redo the fix for FORGE-BUG-046 from triage"
- "reopen sprint FORGE-S32 from task T03"

## Resolve plugin root

```
FORGE_ROOT: !`node -e "console.log(require('./.forge/config.json').paths.forgeRoot)"`
```

## Step 1 — Interpret the request

From `$ARGUMENTS`, determine:
- **entity kind**: `task` (`*-S##-T##`), `bug` (`*-BUG-###`), or `sprint`
  (`*-S##`). A `-BUG-` segment ⇒ bug.
- **id**: the canonical record id. If the id is partial or ambiguous, resolve it
  first with `node "$FORGE_ROOT/tools/store-cli.cjs" query <text>` (or
  `store-query-nlp`) and pick the unambiguous match; if ambiguous, ask the user.
- **target phase** (`--to`):
  - task: `plan | review-plan | implement | review-code | validate | approve | writeback | commit`
  - bug: `triage | plan-fix | review-plan | implement | review-code | approve | commit`
  - sprint: requires a **from-task** (the member task to rewind from); the
    cascade is computed for you.

If the target phase is missing or not valid for the entity kind, ask the user
rather than guessing.

## Step 2 — Compute the plan + integrity implications (READ-ONLY)

```sh
# task / bug
node "$FORGE_ROOT/tools/reset-plan.cjs" --entity <kind> --id <id> --to <phase> --json
# sprint
node "$FORGE_ROOT/tools/reset-plan.cjs" --entity sprint --id <sprintId> --from-task <taskId> --json
```

`reset-plan.cjs` is a **pure planner** — it reads the store and computes the
transitions + warnings, and writes nothing. Parse its JSON:

- `entity` — kind, id, currentStatus, targetStatus/targetPhase.
- `rewind` / `forceRequired` — whether this is a backward transition (needs
  `--force`).
- `transitions[]` — the exact `{id, kind, from, to}` status changes a confirmed
  reset would apply (for a sprint: the sprint reopen + each cascade task).
- `cascade[]` — (sprint) the member task ids that rewind.
- `warnings[]` — referential-integrity implications, each with a `code`:
  - `committed-work` — the entity is committed/fixed; **its git commit is NOT
    reverted** by the reset. The working tree may diverge.
  - `sprint-incoherent` / `sprint-reopen` — the parent sprint is completed; it
    must reopen to `active` (sprint resets do this automatically).
  - `dependents-affected` — other tasks depend on this id and are past planning;
    they now rest on rewound work and may need re-validation. (`ids` lists them.)

If `reset-plan.cjs` exits non-zero, surface its `error` and stop.

## Step 3 — Present the plan and REQUIRE confirmation

Show the user, in plain language:
1. The transition(s) that will be applied (`from → to` for each id).
2. Every warning, especially `committed-work` and `dependents-affected` — these
   are the referential-integrity risks they are accepting.
3. The note that this does **not** revert git commits or re-run anything; it
   only rewinds store status + (for `4ge` users) the resume-state cache.

**Do not proceed without an explicit "yes".** This is a destructive,
hard-to-reverse store mutation. If the user hesitates, stop.

## Step 4 — Apply (only after confirmation)

Apply each transition in `transitions[]` in order, via store-cli. Backward
transitions and terminal states (`blocked`, `committed`, `fixed`) require the
operator-gated force flag:

```sh
FORGE_ALLOW_FORCE=1 node "$FORGE_ROOT/tools/store-cli.cjs" \
  update-status <kind> <id> status <to> --force
```

For a **sprint** reset, apply the sprint reopen first, then each cascade task.

After the status transitions land:
- Tell the user the reset is complete and how to resume:
  - task → `/forge:run-task <id>` (or `/forge:implement <id>`)
  - bug → `/forge:fix-bug <id>`
  - sprint → `/forge:run-sprint <id>`
- If they drive Forge through **forge-cli** (`4ge`), the resume-state cache
  (`.forge/cache/*-state.json`) is rewound by `4ge reset <id> --to <phase>` —
  the LLM route here only owns the store side. Mention this so a `4ge` user runs
  `4ge reset` (or deletes the stale cache) before resuming.

## Iron rule

Never apply a transition `reset-plan.cjs` did not propose, and never skip the
confirmation gate. The planner is the single source of truth for what a reset
touches; this command's only job is to explain it and, on a clear yes, apply
exactly that.
