---
requirements:
  reasoning: Medium
  context: Low
  speed: High
deps:
  personas: [collator]
  skills: [collator, generic]
  templates: []
  sub_workflows: []
  kb_docs: [MASTER_INDEX.md]
  config_fields: [paths.engineering]
---


# Collate
## Iron Laws

- Collation is a read-and-rewrite of generated markdown. Do not mutate any JSON record under `.forge/store/`; the store is the source of truth and collation flows downstream from it.
- Read `.forge/personas/collator.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store reads via `forge_store` (or `node .forge/tools/store-cli.cjs`). Never edit `.forge/store/*.json` directly.
- Do NOT call `set-summary` or `set-bug-summary` from within collation. The collator writes markdown views and a `WRITEBACK-SUMMARY.json` disk file only. Calling `set-summary` mutates the JSON store and violates Iron Law 1 (the store is the source of truth; collation flows downstream from it, not into it). The orchestrator reads `WRITEBACK-SUMMARY.json` directly — no store write is needed.

<!-- See _fragments/store-write-verification.md — NOTE: this file uses an intentionally modified
     Store-Write Verification variant: collation typically writes markdown views (not JSON records),
     so the preamble explains when `forge_store` calls apply. Canonical fragment is reference only. -->
## Store-Write Verification

Collation typically writes markdown views, not JSON records. If a remediation
step does call `forge_store` and the call exits non-zero or the `PreToolUse`
write-boundary hook blocks the call (exit 2):

1. Parse the structured error (names the offending field + schema file).
2. Correct the JSON to satisfy the schema.
3. Retry. Repeat up to 3 times.
4. After 3 failures, halt and escalate with original payload, corrected payload, and all error messages.

Never set `FORGE_SKIP_WRITE_VALIDATION=1` — operator-only emergency switch.

## Algorithm

```
1. Preferred: Run Plugin Tool
   - Run: `node .forge/tools/collate.cjs [SPRINT_ID]`
   - If tool succeeds, proceed to Finalize

2. Fallback: Manual Collation
   - Read `.forge/config.json` for prefix, paths, project description
   - Read all sprint/task/bug/event JSONs from `.forge/store/`
   - Generate MASTER_INDEX.md (sprint registry, task registry, bug registry)
   - Generate per-sprint TIMESHEET.md (from events)
   - Generate any other configured views

3. Rebuild context pack:
   - Rebuild the architecture context pack so orchestrators have a fresh summary
     after any KB updates (architecture docs may have changed during the sprint):
     ```
     ENGINEERING=$(node .forge/tools/manage-config.cjs get paths.engineering 2>/dev/null || echo engineering)
     node .forge/tools/build-context-pack.cjs \
       --arch-dir "$ENGINEERING/architecture" \
       --out-md .forge/cache/context-pack.md \
       --out-json .forge/cache/context-pack.json
     ```
   - On exit 1 (architecture directory absent), skip silently.

4. Finalize:
   - **Do NOT emit a phase event yourself.** The orchestrator (or kickoff handler) owns event emission — it composes the canonical event from runtime telemetry (model, provider, tokens, wall times) plus the SUMMARY you write in the next step. Subagents that call `store-cli emit` for phase events hallucinate runtime facts (see Plan 11 / Slice 2). Write the SUMMARY and return.
   - Write `WRITEBACK-SUMMARY.json` to the sprint's artifact directory — use the sprint record's `path` field (read it from the store), not a reconstructed `engineering/sprints/{sprintId}/` template — with the following shape:
     ```json
     {
       "objective":   "<one sentence — what views were regenerated>",
       "key_changes": ["<up to 6 bullets — which files were written>"],
       "verdict":     "n/a",
       "written_at":  "<current ISO 8601 timestamp>"
     }
     ```
     The orchestrator reads this file directly to compose the collation event narrative. Do NOT call `set-summary` to register it — that would mutate the store in violation of Iron Law 1.
   - KB link refresh (best-effort; skip if unavailable):
     When running standalone in the Claude Code TUI, invoke via the Skill tool
     (`skill: "forge:refresh-kb-links"`).
     When running as a subagent within forge-cli (Pi runtime), the TS orchestrator
     calls `runRefreshKbLinks` directly after this phase — skip this step.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->