# Fragment: Event Type Vocabulary

<!-- Canonical phase→type vocabulary for store events (forge-engineering#39).
     This fragment OWNS the `type` token vocabulary; `schemas/event.schema.json`
     mirrors it. Any vocabulary change lands here first, then in the schema enum
     (test-first via tools/__tests__/event-schema-variants.test.cjs), then in
     emitters (forge-cli orchestrators, wfl JS drivers, meta workflows).

     Companion: _fragments/event-emission-schema.md owns WHO emits (orchestrator
     vs subagent field ownership). This fragment owns WHAT token each phase emits.

     Invariant: every token emitted by any surface MUST appear in the schema
     enum, and every enum token MUST have a named emitter below. Dead tokens are
     removed (bug-fixed was retired by forge-engineering#39 — defined, never
     emitted by anything). -->

## Rules

1. **Emitted tokens ⊆ schema enum.** An emitter never invents a token. If a new
   phase outcome needs a token, add it here + the schema enum first.
2. **`type` is optional** on events. An untyped event is valid (task-pipeline
   events from forge-cli run-task are currently untyped). When a `type` IS set,
   it must come from the tables below.
3. **Kebab-case for lifecycle tokens.** Underscore names are reserved for the
   `friction` and `skill_usage` event families.
4. **Conditional fields.** Typed events trigger schema conditionals — see the
   "Requires" column. Driver-emitted skip events have no model/provider; emit
   the literal `"n/a"` for both (never guess a model string — see
   `_fragments/event-emission-schema.md` §Why no example record here).
5. **Skip reasons ride in `notes`.** `reason` is not a declared event property
   (`additionalProperties: false` rejects it).

## Task pipeline (orchestrate_task / run-task / wfl:run-task)

| Phase | Pass token | Fail token | Requires |
|---|---|---|---|
| plan | `task-planned` | `task-planned` | taskId, phase, iteration |
| review-plan | `plan-approved` | `review-failed` | taskId, phase, iteration |
| implement | `task-implemented` | `task-implemented` | taskId, phase, iteration |
| review-code | `review-passed` | `review-failed` | taskId, phase, iteration |
| validate | `task-validated` | `review-failed` | taskId, phase, iteration |
| approve | `task-approved` | `review-failed` | taskId, phase, iteration |
| commit | `task-committed` | `task-committed` | taskId, phase, iteration |

`plan-complete` is a legacy alias accepted by the enum for plan-phase events;
new emitters use `task-planned`.

## Bug pipeline (fix_bug / fix-bug / wfl:fix-bug)

| Phase | Pass token | Fail token | Requires |
|---|---|---|---|
| pre-loop skip | `bug-skipped` | — | bugId (no phase/iteration; model/provider `"n/a"`) |
| triage | `bug-triaged` | `bug-triaged` | bugId, phase, iteration |
| plan-fix | `fix-planned` | `fix-planned` | bugId, phase, iteration |
| review-plan | `fix-review-passed` | `fix-review-failed` | bugId, phase, iteration |
| implement | `fix-implemented` | `fix-implemented` | bugId, phase, iteration |
| review-code | `fix-code-review-passed` | `fix-code-review-failed` | bugId, phase, iteration |
| approve | `fix-approved` | `fix-revision-requested` | bugId, phase, iteration |
| commit | `bug-committed` | `bug-commit-failed` | bugId, phase, iteration |

Non-review phases emit their single token regardless of outcome (the verdict
field carries the judgement); review phases (review-plan, review-code, approve)
select pass/fail on `verdict == "revision"`.

## Sprint grain (run_sprint / run-sprint / wfl:run-sprint)

| Moment | Token | Requires |
|---|---|---|
| before wave loop | `sprint-start` | sprintId (taskCount optional) |
| before each task dispatch | `task-dispatch` | taskId (convention; phase/iteration carried but not schema-enforced) |
| sprint finished | `sprint-complete` | taskCount, completedTaskIds, verdict |
| sprint aborted | `sprint-halted` | haltedAtTaskIndex, haltedAtTaskId, lastError |

## Other families

| Token | Emitter | Spec |
|---|---|---|
| `friction` | any persona, drained by orchestrator | `_fragments/friction-emit.md` |
| `skill_usage` | forge-cli skill-curation telemetry | `event.schema.json` skill_usage conditional |

## Out of scope (known follow-ups)

- **Feature events** have no lifecycle tokens; feature-related events are
  emitted untyped. Defining a feature vocabulary is a future change to this
  fragment + the enum.
- **Task-pipeline typed emission in forge-cli** — run-task currently emits
  untyped events (valid under Rule 2); adopting the task table above is
  forward work tracked in forge-engineering#39 step 3.
