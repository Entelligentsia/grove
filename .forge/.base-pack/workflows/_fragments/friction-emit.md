<!-- Canonical Friction Emit fragment.
     Referenced from meta-implement.md, meta-fix-bug.md, meta-validate.md,
     meta-plan-task.md, and meta-orchestrate.md. /forge:rebuild --enrich (phase 2)
     greps generated workflows for `## Friction Emit` to discover the channel.

     PLAN-11 / SLICE-2 (2026-05-14): friction is now recorded via the
     `friction-emit.cjs` tool which appends judgement-only records to
     `.forge/cache/FRICTION-{workflow}.jsonl`. The orchestrator drains this
     file at phase-end, stamps runtime attribution (model/provider/usage/
     timestamps/eventId) onto each record, and emits the events. The LLM
     never hand-builds a friction event JSON. -->

# Friction Emit (Fragment)

When the persona detects skill friction during the workflow — a referenced
skill is unused, fails on invocation, is missing from the registry, has gone
stale relative to current architecture, or is redundant with another skill —
record a judgement-only friction signal. The orchestrator drains the
signals and emits the corresponding events so `/forge:rebuild --enrich` (phase 2)
can act on them.

## Trigger conditions

Set `--issue` on the emitted record to one of the following tokens:

| Token              | When to emit                                                                     |
|--------------------|----------------------------------------------------------------------------------|
| `skill_unused`     | A skill listed in the persona's skill block was loaded but never consulted.      |
| `skill_failed`     | A skill was consulted but its guidance produced an error or required correction. |
| `skill_missing`    | The workflow needed guidance the available skills did not cover.                 |
| `skill_stale`      | A skill's guidance contradicts current architecture / supersedes its own advice. |
| `skill_redundant`  | Two skills provided overlapping or conflicting guidance for the same decision.   |

Emit one record per distinct friction signal — do not coalesce multiple
findings into a single record.

## Judgement-only contract

The friction recorder accepts only judgement fields. Runtime-attribution
fields (`model`, `provider`, `eventId`, timestamps, token counts) are
**rejected** if passed — they belong to the orchestrator and are stamped on
at drain time.

```sh
node .forge/tools/friction-emit.cjs \
  --workflow {workflow-key} \
  --persona  {persona-noun} \
  --issue    skill_unused \
  [--subkind  skill_unused] \
  [--evidence '{"trajectory_excerpt":"...","tool_errors":["..."],"retrieval_score":0.0,"skillId":"..."}']
```

Required flags: `--workflow`, `--persona`, `--issue`.
Optional flags: `--subkind` (frozen enum
`skill_unused|skill_failed|skill_missing|skill_stale|skill_redundant`
or experimental `^x_[a-z_]+$`), `--evidence` (JSON object with
`trajectory_excerpt`, `tool_errors`, `retrieval_score` (0..1), `skillId`).

The tool appends one line of judgement-only JSON to
`.forge/cache/FRICTION-{workflow}.jsonl`. The orchestrator reads this file
after the phase completes, stamps each record with the captured runtime
attribution (model, provider, usage, wall times, eventId), and emits the
resulting events via `store-cli emit`.

## Per-workflow values

| Workflow         | `workflow` | `persona`     | `phase`      |
|------------------|------------|---------------|--------------|
| meta-implement   | implement  | engineer      | implement    |
| meta-fix-bug     | fix-bug    | bug-fixer     | fix-bug      |
| meta-validate    | validate   | qa-engineer   | validate     |
| meta-plan-task   | plan-task  | architect     | plan         |
| meta-orchestrate | orchestrate| orchestrator  | orchestrate  |
