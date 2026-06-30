---
requirements:
  reasoning: Low
  context: Low
  speed: High
audience: subagent
phase: commit
context:
  architecture: false
  prior_summaries: none
  persona: summary
  master_index: false
  diff_mode: false
deps:
  personas: [engineer]
  skills: [engineer, generic]
  templates: [PROGRESS_TEMPLATE]
  sub_workflows: []
  kb_docs: []
  config_fields: [commands.test, paths.engineering]
---

# 🌱 Meta-Workflow: Commit Task

## Purpose

Seal a completed and approved task by committing its artifacts to the VCS and updating the store.

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance -->
## Iron Laws

- Commit only the artifacts produced for this task; do not sweep unrelated working-tree changes into the commit. The commit boundary mirrors the task boundary.
- Read `.forge/personas/engineer.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store`. Never edit `.forge/store/*.json` directly.
- **Never run `git add`/`git commit`/`git reset` yourself** — `commit-task.cjs` owns staging, boundary checks, committing, and the terminal transition (#40). Your judgement input is the message.
- **Commit writes NO summary** (`commit` ∉ `VALID_SUMMARY_PHASES` — any `set-summary` is rejected); the tool's terminal `update-status` is this phase's only store write.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Algorithm

```

0. Entity-mode resolution:
   - Read the kickoff arguments. `--task {id}` → `entity_kind = "task"`, `record_id = {id}`. `--bug {id}` → `entity_kind = "bug"`, `record_id = {id}`.

1. Inspect ONCE (message material only — #40 batched-inspection rule):
   - One `git diff --stat`; at most ONE combined `git diff` if the message needs detail.
     Never per-file diffs, never repeated `git status` — each extra turn re-pays full context.
   - Staging is NOT your decision — the tool derives it from the store record.

2. Craft the commit message:
   - Follow project conventions; include the record ID ({taskId} / {bugId}) in the subject.
   - `Co-authored-by:` trailer from the host runtime: Claude Code →
     `Co-authored-by: Claude <noreply@anthropic.com>`; pi / Ollama / other →
     `Co-authored-by: {modelId} <noreply@{provider}.ai>` from the session's `provider` and
     `modelId`; if unresolvable, omit rather than guess. Never hardcode
     `Claude Opus 4.6 <noreply@anthropic.com>` (forge#82 regression).
   - Git's configured `user.name`/`user.email` own authorship; never `--author`.

3. Commit via the tool — ONE call:
   - If the `forge_commit` named tool is available (forgecli): call it —
     `forge_commit({ entity:"{entity_kind}", id:"{record_id}", message:"<message>", trailer:"<line>" })`.
     Never pass the message through a bash string when the typed tool exists.
   - Otherwise (Claude Code): `forge_commit({ entity: "{entity_kind}", id: "{record_id}", message: "<message>" }) [--trailer "<Co-authored-by line>"]`
   - The tool owns the choreography: preflight gate (`preflight-gate.cjs --phase commit`
     internally), status precondition (task `approved` / bug `in-progress` — wrong-state runs
     halt with `× {record_id} is in state '{status}' …; /forge:approve must complete first`),
     staging (artifact dir + `summaries.implementation.files_changed` provenance),
     commit-boundary guard (aborts on a pre-staged index), `git commit`, terminal transition
     (task → `committed`; bug → `fixed`, the ONLY post-triage `bug.status` write).
   - On `no files_changed provenance` warning: ONE `git status --porcelain`, then re-run the
     tool with `--also <path>` per source file. Never manual `git add`.
   - Success → JSON with `ok:true`. `committed:true` carries `sha` + `staged`;
     `committed:false, reason:"nothing-to-commit"` is ALSO success (fix already at HEAD /
     staging set clean — the tool still sealed the record's terminal status). Do not
     "fix" a no-op by staging things yourself.
   - Failure (exit 1 / ok:false) → print stderr and HALT — no manual staging, no
     `git reset`, no `--force` retry (operator-gated). Tool file missing → HALT;
     instruct `/forge:update` + `/forge:rebuild tools`.
   - NEVER commit before the tool reports `ok: true` — the premature-commit/reset/redo loop
     is forbidden.

4. Finalize:
   - No summary, no `set-summary` (see Iron Laws). **Do NOT emit a phase event yourself** —
     the orchestrator owns event emission. Return the tool's JSON result as your phase output.
```

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Generation Instructions

- **Workflow Structure:** The generated `commit_task.md` must follow the strict "Algorithm" block format.
- **Tool Ownership:** All git operations route through `commit-task.cjs` (forge-engineering#40) — the generated workflow must never instruct manual `git add`/`git commit`/`git reset`.
- **Project Specifics:**
  - Embed project's commit message conventions.
- **Token Reporting:** See `_fragments/finalize.md` — wire via `file_ref:`.
- **Event Emission:** Ensure the "complete" event includes the `eventId` passed by the orchestrator.
