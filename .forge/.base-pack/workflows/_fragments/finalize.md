## Finalize: Subagent Closure

Before returning, every subagent MUST:

1. Write any phase-specific summary the workflow body requires (e.g. PLAN.md, CODE_REVIEW.md).
2. Confirm `task.status` reflects the phase outcome (via `store-cli update-status` if applicable).
3. Return cleanly.

**Subagents MUST NOT write token-usage sidecars.** Token telemetry is owned by the orchestrator, which captures provider-reported usage from the runtime as the subagent runs and emits the canonical event (with `provider`, `model`, `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `tokenSource: "reported"`) on the subagent's behalf.

If the runtime does not surface usage (rare), the orchestrator emits the event **without** the token fields — never with placeholder zeros or a `"missing"` marker. Honest absence beats misleading presence.

The `eventId` is passed by the orchestrator in the subagent prompt and is used only for non-token writes (e.g. `set-summary`, `update-status` history).
