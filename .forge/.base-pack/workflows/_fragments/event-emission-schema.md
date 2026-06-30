# Fragment: Event Emission Schema

<!-- Canonical contract: subagents write judgement-only SUMMARY files; the
     orchestrator stitches runtime telemetry + judgement into the canonical
     event and emits it. Referenced by meta-orchestrate.md and meta-fix-bug.md.

     Companion: _fragments/event-vocabulary.md owns the `type` token
     vocabulary (phase→type tables for task/bug/sprint events). This fragment
     owns WHO emits; that one owns WHAT token.

     PLAN-11 / SLICE-2 (2026-05-14): the LLM no longer hand-builds event JSON.
     Runtime facts (model, provider, eventId, timestamps, iteration, tokens)
     are owned by the orchestrator, never the subagent. The subagent only
     produces judgement and lets the orchestrator complete the record. -->

## Who writes what

| Actor              | Owns (writes)                                                                                   | Never touches |
|--------------------|-------------------------------------------------------------------------------------------------|---------------|
| **Subagent (LLM)** | judgement fields: `verdict`, `notes`, `findings`, `objective`, `type`                           | `eventId`, `model`, `provider`, `startTimestamp`, `endTimestamp`, `durationMinutes`, `iteration`, `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `tokenSource` |
| **Orchestrator**   | everything else — composes the canonical event from runtime telemetry + the subagent's SUMMARY  | the judgement fields themselves (copies them through unchanged) |

The LLM is the wrong actor for runtime facts: it has no privileged access to
the model/provider it ran under, the wall clock at spawn time, or the token
counts reported by the runtime stream. Every LLM guess at these fields is
wrong by construction.

## What the subagent does

After completing its phase, the subagent writes one file:

```
.forge/cache/{PHASE}-SUMMARY.json
```

Examples: `PLAN-SUMMARY.json`, `REVIEW-PLAN-SUMMARY.json`,
`IMPLEMENT-SUMMARY.json`, `REVIEW-CODE-SUMMARY.json`, `COMMIT-SUMMARY.json`.

The SUMMARY contains judgement only. Required keys are phase-specific
(see `forge/schemas/phase-summary.schema.json` for the exact shape per
phase) but typically include `verdict`, `notes`, and any `findings`
the phase produces. The subagent **must not** include runtime fields —
adding `model`, `provider`, timestamps, or token counts to the SUMMARY is
ignored at best and rejected at worst.

The subagent **does not** call `store-cli emit` for phase events. That
shell-out is reserved for the orchestrator.

## What the orchestrator does

After the subagent returns, the orchestrator constructs the event from:

1. **Runtime telemetry** captured during the subagent run:
   `model`, `provider`, token usage (`inputTokens`, `outputTokens`,
   `cacheReadTokens`, `cacheWriteTokens`, `tokenSource: "reported"`).
2. **Known task context** the orchestrator already tracks for run-task:
   `taskId`, `sprintId`, `phase`, `iteration`.
3. **Bracketed wall times** the orchestrator records around the subagent
   call: `startTimestamp`, `endTimestamp`, `durationMinutes`.
4. **Judgement blob** read from `{PHASE}-SUMMARY.json`: `verdict`, `notes`,
   `findings`, etc.

The orchestrator then emits via:

```
node .forge/tools/store-cli.cjs emit {sprintId} '{complete-event-json}'
```

## Why no example record here

This fragment intentionally contains **no hardcoded example model strings,
provider names, or timestamps**. Such examples were the historical source of
LLM hallucination — subagents would copy the example verbatim ("the model
is claude-sonnet-4-6 because the workflow says so") even when running on a
completely different runtime. The schema lives at
`.forge/schemas/event.schema.json`; consult it directly when verifying a
field set.
