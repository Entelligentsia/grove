---
requirements:
  reasoning: High
  context: Medium
  speed: Medium
audience: orchestrator-only
deps:
  personas: [bug-fixer, supervisor, architect, engineer, collator]
  skills: [bug-fixer, supervisor, architect, engineer, generic]
  templates: [PROGRESS_TEMPLATE]
  sub_workflows: [plan_task, implement_plan, review_plan, review_code, architect_approve, commit_task]
  kb_docs: [architecture/stack.md, architecture/routing.md]
  context_pack: .forge/cache/context-pack.md
  config_fields: [commands.test, paths.engineering]
---

# 🍂 Meta-Workflow: Fix Bug

## Purpose

Drive a single bug through its complete lifecycle. The pipeline mirrors
`meta-orchestrate.md` so personas, phases, gates, and event emission stay
identical — the only deviation is **the judgement step after triage**, which
branches the bug into a short Path A or the full Path B. Everything past the
branch reuses the run-task phases verbatim.

## Pipeline Phases

A fix-bug pipeline has these phases (mirrors `meta-orchestrate.md § Pipeline
Phases`):

| Phase | Role | Persona | Workflow | Path A | Path B |
|---|---|---|---|---|---|
| triage | `triage` | bug-fixer | `triage.md` | yes | yes |
| plan-fix | `plan` | engineer | `plan_task.md` (bug-mode) | no | yes |
| review-plan | `review-plan` | supervisor | `review_plan.md` | no | yes |
| implement | `implement` | engineer | `implement_plan.md` (bug-mode) | yes | yes |
| review-code | `review-code` | supervisor | `review_code.md` | yes | yes |
| approve | `approve` | architect | `architect_approve.md` (bug-mode) | yes | yes |
| commit | `commit` | engineer | `commit_task.md` (bug-mode) | yes | yes |
| finalize | `finalize` | collator | (inline algorithm) | yes | yes |

Phases past triage are the same workflows used by the run-task pipeline. The
generated orchestrator passes `--bug {bugId}` (in place of `--task {taskId}`)
to every sub-workflow and to `preflight-gate.cjs`. Sub-workflows resolve the
record kind from the flag and adjust their verdict-source mapping via
`BUG_PHASE_VERDICT_SOURCE` in `tools/read-verdict.cjs`.

## Status State Machine

Bug status writes are owned by specific phases — never by the workflow source
in finalize, never by an LLM improvising on a task workflow.

```
reported -> triaged -> in-progress -> fixed
                       (terminal)
```

| Transition | Owner | Trigger |
|---|---|---|
| `reported → triaged` | triage subagent | after reproduction confirmed |
| `triaged → in-progress` | triage subagent | after route decision recorded in `summaries.triage.route` |
| `in-progress → fixed` | commit phase | after git commit succeeds (terminal) |

The schema's `approved` and `verified` enum members are vestigial — no phase in
this workflow writes them, and the verdict gate reads
`summaries.approve.verdict`, not `bug.status`. A follow-up cleanup should drop
both members from `bug.schema.json` to remove the runtime trap. Until then,
this workflow MUST NOT write either value. The Phase Gates below `forbid`
them defensively.

## Triage Judgement (the only run-task deviation)

After the triage subagent reproduces the bug and confirms root cause, it MUST
record a **route** decision in its summary:

```json
{
  "objective": "Triage FORGE-BUG-NNN — reproduce, locate, decide route.",
  "key_changes": [...],
  "findings": [
    "Root cause: <one line>",
    "Reproduction: <one line>",
    "Route decision: A | B",
    "Rationale: <one line>"
  ],
  "verdict": "n/a",
  "written_at": "<iso>",
  "artifact_ref": "TRIAGE.md",
  "route": "A"
}
```

The `route` field is required. Allowed values: `"A"` or `"B"`.

> **Field-naming caution — runtime-tested.** The route field is named
> `route`, never `path`. The bug schema's top-level `path` field is the
> bug's **artifact directory** (e.g. `engineering/bugs/EMG-BUG-001-...`).
> Conflating the two caused EMBERGLOW-BUG-001 (v0.44.0 first run) to land
> its `TRIAGE.md` under `.forge/store/bugs/` instead of `engineering/bugs/`.
> Triage subagents MUST NOT touch `bug.path` — that field is set at bug
> creation and never modified by triage.

### Path A / Path B eligibility

See `triage.md § Path A / Path B Eligibility` for the criteria the triage
subagent applies. The criteria are single-sourced in the triage workflow;
this orchestrator only reads the resulting `summaries.triage.route` value.

### Pipeline selection by path

```
phases_A = [triage, implement, review-code, approve, commit, finalize]
phases_B = [triage, plan-fix, review-plan, implement, review-code, approve, commit, finalize]

if summaries.triage.route == "A": phases = phases_A
else:                            phases = phases_B
```

The orchestrator MUST read `summaries.triage.route` from the bug record after
the triage subagent returns and select the phase list before entering the main
loop. The selection is final for the run — no mid-pipeline switching.

## Pipeline Resolution

Fix-bug does **not** read `task.pipeline` from config. The path-branch decision
above replaces the task pipeline lookup. The orchestrator MAY honour
`config.pipelines.bug` to override the default Path A / Path B phase lists,
mirroring `meta-orchestrate.md § Pipeline Resolution`; if unset, the lists
above are used.

## Algorithm

The fix-bug orchestrator MUST follow this procedure exactly. The structure
mirrors `meta-orchestrate.md § Execution Algorithm` — same persona-map, same
banner-map, same cluster detection, same preflight gate, same event emission.
Differences are confined to the **triage** step and the **path branch**.

```
1. Pre-loop setup (mirrors meta-orchestrate.md):
   - Session Preflight (see `meta-orchestrate.md § Session Preflight`):
     Read `.forge/cache/preflight-status.json` if present and fresh; otherwise
     run `node .forge/tools/forge-preflight.cjs --bug {bugId}` and read
     its stdout. Use `blob.forgeRoot` as FORGE_ROOT for the remainder of the
     run — do not re-derive it. Branch on `blob.ok`: ok:true → proceed;
     ok:false → halt (fast-fail-safe), emit escalation event, instruct operator
     to fix the surfaced preflight warning before re-running.
   - Detect execution cluster from ANTHROPIC_DEFAULT_*_MODEL env vars.
   - Clear progress log: store-cli progress-clear bugs
   - Read bug record. If status ∈ {blocked, escalated, fixed, abandoned}:
     skip the run, emit a single `bug-skipped` event (token + payload shape per
     `_fragments/event-vocabulary.md` § Bug pipeline: requires `bugId`, no
     phase/iteration; model/provider `"n/a"`; skip reason in `notes`), return.

2. Triage:
   - Locate or create the bug record (MANDATORY — do this before anything else):
     a. Determine the bug ID: if $ARGUMENTS is an existing FORGE-BUG-NNN ID, use it.
        Otherwise derive the next available ID by listing .forge/store/bugs/.
     b. If .forge/store/bugs/{BUG_ID}.json does NOT exist, write a fresh record
        via store-cli with status="reported".
     c. Read the now-guaranteed record.
   - Spawn the triage subagent (workflow: `triage.md`, persona: bug-fixer).
     It MUST write `TRIAGE.md` + `TRIAGE-SUMMARY.json` with a `route` field
     (`"A"` or `"B"`) and call `set-bug-summary {bugId} triage` per
     `triage.md`. The triage workflow is `audience: subagent`, `phase: triage`;
     the orchestrator MUST NOT pass any other workflow body to this subagent.
   - On return, orchestrator transitions status:
       store-cli update-status bug {bugId} status triaged
       store-cli update-status bug {bugId} status in-progress
     # Digest-compliance note (FORGE-S27-T05): update-status is already silent on
     # success (no stdout on the success path). The two-step triaged→in-progress
     # transition is a required state-machine contract per Iron Law 2 — only triage
     # and commit own bug.status writes. Collapsing these two writes into one would
     # be a behavioral violation. This is not LLM-hand-run glue; it executes as a
     # direct tool call after the triage subagent returns.
   - Read summaries.triage.route. If neither "A" nor "B": escalate
     (verdict_malformed). Do not guess.

3. Path selection:
   - phases = phases_A if route == "A" else phases_B
   - Begin main phase loop.

4. Phase loop (identical to meta-orchestrate.md § Execution Algorithm):
   for each phase in phases[1:]:    # triage already done
     - Resolve model (cluster + ROLE_TIER).
     - Compute eventId, agent_name, banner_name (from PERSONA_MAP /
       BANNER_MAP below).
     - Announce phase:
       run_bash(f'forge_banner({ name: "--badge" }) {banner_name} --quiet')
       # Digest-compliance note (FORGE-S27-T05): --quiet makes banners.cjs emit
       # zero stdout (unconditional; no isTTY branch). The badge is fully
       # suppressed during the automated run_bash call — it does not enter the
       # LLM context window. Mirrors meta-orchestrate.md:399–411 treatment.
       print(f"  → {bugId}  [{display_model}]\n")
     - Start progress Monitor on .forge/store/events/bugs/progress.log.
     - Preflight gate: preflight-gate.cjs --phase {role} --bug {bugId}
       Exit 1 → structured JSON on stdout ({ phase, reasonCode, detail, remediation });
       parse and render it exactly as meta-orchestrate.md § Execution Algorithm
       does for tasks (safe_parse_json(stdout) → format reason + remediation).
       Exit 2 → escalate (misconfiguration). In both cases:
       see meta-orchestrate.md § Escalation Procedure with bug_id substituted
       for task_id. Update bug.status to "escalated" only if it is currently
       "in-progress" (do not downgrade other states).
     - Compose role-block, architecture-block, summary-block, overlay (via
       build-overlay.cjs --bug {bugId}).
     - Spawn subagent via Agent tool. Subagent prompt passes:
         sprint_or_bug_id = "bugs"   # virtual sprint dir for emit/sidecar
         record_id        = {bugId}
         sidecar_path     = .forge/store/events/bugs/_{event_id}_usage.json
     - On return: merge sidecar, emit canonical event (orchestrator-owned),
       stop progress Monitor, print phase-exit signal (✓ / ↻ / ⚠), run
       /compact with checkpoint line.
     - If phase is a review and verdict == "revision": re-enter the loop
       on the on_revision predecessor up to max_iterations. Exhaust →
       escalate (see meta-orchestrate.md § Escalation Procedure).

5. Phase-specific responsibilities (sub-workflow contracts):
   - plan-fix (Path B): engineer writes BUG_FIX_PLAN.md and BUG-FIX-PLAN-SUMMARY.json
     (verdict: "n/a"). No status write.
   - review-plan (Path B): supervisor writes REVIEW-PLAN-SUMMARY.json
     (verdict: approved | revision). No status write.
   - implement: engineer (or bug-fixer for Path A) applies the fix, runs the
     regression test, writes IMPLEMENTATION-SUMMARY.json (verdict: "n/a").
     No status write — bug stays at "in-progress".
   - review-code: supervisor reads the actual diff and the regression test,
     writes REVIEW-CODE-SUMMARY.json (verdict: approved | revision).
   - approve: architect writes ARCHITECT_APPROVAL.md and APPROVE-SUMMARY.json
     (verdict: approved | revision). No status write — the verdict signal is
     the summary, not bug.status (see read-verdict.cjs:44).
   - commit: engineer makes the git commit and runs:
       store-cli update-status bug {bugId} status fixed
     Then writes COMMIT-SUMMARY.json (verdict: "n/a"). This is the ONLY
     phase that writes bug.status post-triage.

6. Finalize (collator, housekeeping):
   - Run `forge_collate({ args: ["{bugId}", "--purge-events`.", "#", "Note", "(FORGE-S27-T05):", "collate.cjs", "--purge-events", "already", "handles", "cost", "#", "aggregation", "and", "embeds", "the", ""##", "Cost"", "section", "into", "the", "bug's", "INDEX.md", "#", "automatically", "(see", "collate.cjs", "buildBugIndex).", "Do", "NOT", "attempt", "a", "separate", "#", "cost-aggregation", "round-trip", "before", "calling", "collate", "—", "it", "is", "redundant", "and", "#", "may", "produce", "a", "duplicate", "section.", "The", "collate", "call", "is", "the", "single", "step.", "Collate", "purges", "only", "this", "bug's", "events", "from", "the", "shared", "bugs/", "dir", "(filtered", "by", "bugId", "reference)", "—", "it", "does", "NOT", "purge", "other", "bugs'", "events.", "-", "Run", "preflight", "finalize", "gate:", "preflight-gate.cjs", "--phase", "finalize", "--bug", "{bugId}.", "Exit", "1", "→", "escalate.", "Do", "NOT", "downgrade", "bug.status", "(it", "is", "already", ""fixed").", "-", "Do", "NOT", "emit", "a", "phase", "event", "yourself.", "The", "orchestrator", "owns", "event", "emission", "for", "finalize", "as", "it", "does", "for", "every", "other", "phase", "—", "composed", "from", "runtime", "telemetry", "plus", "the", "collator's", "summary.", "```", "##", "Persona", "and", "Banner", "Maps", "Mirrors", "`meta-orchestrate.md`", "for", "shared", "roles.", "Bug-only", "role", "is", "`triage`.", "```", "#", "---", "Role-to-noun", "mapping", "(persona", "and", "skill", "file", "lookups)", "---", "ROLE_TO_NOUN", "=", "{", ""triage":", ""bug-fixer",", ""plan":", ""engineer",", "#", "Path", "B", "only", ""review-plan":", ""supervisor",", "#", "Path", "B", "only", ""implement":", ""engineer",", ""review-code":", ""supervisor",", ""approve":", ""architect",", ""commit":", ""engineer",", ""finalize":", ""collator",", "}", "#", "Default", "fallback:", ""bug-fixer"", "#", "---", "Persona", "symbol", "lookup", "(emoji,", "name,", "tagline)", "---", "PERSONA_MAP", "=", "{", ""triage":", "("🍂",", ""Bug", "Fixer",", ""I", "find", "what", "has", "decayed", "and", "decide", "the", "path."),", ""plan":", "("🌱",", ""Engineer",", ""I", "plan", "what", "will", "be", "built", "before", "any", "code", "is", "written."),", ""review-plan":", "("🌿",", ""Supervisor",", ""I", "review", "before", "things", "move", "forward.", "I", "read", "the", "actual", "fix,", "not", "just", "the", "plan."),", ""implement":", "("🌱",", ""Engineer",", ""I", "build", "what", "was", "planned.", "I", "do", "not", "move", "forward", "until", "the", "code", "is", "clean."),", ""review-code":", "("🌿",", ""Supervisor",", ""I", "review", "before", "things", "move", "forward.", "I", "read", "the", "actual", "code,", "not", "the", "report."),", ""approve":", "("🗻",", ""Architect",", ""I", "hold", "the", "shape", "of", "the", "whole.", "I", "give", "final", "sign-off", "before", "commit."),", ""commit":", "("🌱",", ""Engineer",", ""I", "close", "out", "completed", "work", "with", "a", "clean,", "honest", "commit."),", ""finalize":", "("🍃",", ""Collator",", ""I", "gather", "what", "exists", "and", "arrange", "it", "into", "views."),", "}", "#", "Default", "fallback:", "("🍂",", ""Bug", "Fixer",", ""I", "find", "what", "has", "decayed", "and", "decide", "the", "path.")", "#", "---", "Banner", "identity", "map", "(banner", "name", "per", "phase", "role)", "---", "BANNER_MAP", "=", "{", ""triage":", ""rift",", ""plan":", ""forge",", ""review-plan":", ""oracle",", ""implement":", ""forge",", ""review-code":", ""oracle",", ""approve":", ""north",", ""commit":", ""forge",", ""finalize":", ""drift",", "}", "#", "Default", "fallback:", ""rift"", "```", "##", "Subagent", "Prompt", "Composition", "Identical", "pattern", "to", "`meta-orchestrate.md", "§", "Execution", "Algorithm`.", "The", "only", "differences", "are:", "-", "`--bug", "{bugId}`", "flag", "passed", "to", "preflight-gate.cjs", "and", "sub-workflows.", "-", "`sprint_or_bug_id", "=", ""bugs"`", "for", "emit/sidecar/progress", "(virtual", "sprint", "dir).", "-", "`build-overlay.cjs", "--bug", "{bugId}`", "for", "the", "overlay", "(matches", "the", "task", "pattern", "`build-overlay.cjs", "--task", "{taskId}`).", "-", "Sidecar", "path", "uses", "`.forge/store/events/bugs/_{event_id}_usage.json`", "—", "the", "shared", "bugs", "virtual", "dir.", "Collate", "filters", "by", "bug", "reference", "at", "purge", "time.", "```", "#", "---", "Materialize", "project", "overlay", "(replaces", "MASTER_INDEX.md", "read", "in", "subagent)", "---", "overlay_result", "=", "run_bash(", "f'node", ".forge/tools/build-overlay.cjs", "--bug", "{bug_id}", "--format", "md'", ")", "bug_overlay_md", "=", "overlay_result.stdout", "if", "overlay_result.exit_code", "==", "0", "else", """", "spawn_subagent(", "prompt=compose_subagent_prompt(", "agent_name=agent_name,", "progress_log_path=progress_log_path,", "banner_name=banner_name,", "sprint_or_bug_id="bugs",", "phase_role=phase.role,", "architecture_block=bug_architecture_block,", "summary_block=bug_summary_block,", "role_block=role_block,", "overlay_md=bug_overlay_md,", "context={"Bug", "Root":", "bug_root_path,", ""Store", "Root":", "store_root_path,", ""Events", "Root":", "".forge/store/events/bugs/"},", "workflow=phase.workflow,", "record_id=bug_id,", "sidecar_path=f".forge/store/events/bugs/_{event_id}_usage.json"", "),", "description=f"{emoji}", "{persona_name}", "—", "{phase.name}", "for", "{bug_id}",", "model=phase_model", ")", "```", "##", "Phase", "Gates", "Declarative", "pre-flight", "gates.", "Evaluated", "by", "`forge/tools/preflight-gate.cjs`", "before", "every", "subagent", "spawn.", "Grammar", "identical", "to", "`meta-orchestrate.md", "§", "Phase", "Gates`.", "Gates", "encode", "both", "the", "path-A/path-B", "split", "(via", "`after`", "predecessors", "that", "differ", "per", "path)", "and", "the", "status-trap", "defences.", "```gates", "phase=triage", "forbid", "bug.status", "==", "blocked", "forbid", "bug.status", "==", "escalated", "forbid", "bug.status", "==", "fixed", "forbid", "bug.status", "==", "abandoned", "forbid", "bug.status", "==", "approved", "forbid", "bug.status", "==", "verified", "```", "```gates", "phase=plan", "artifact", "{engineering}/bugs/{bug}/TRIAGE.md", "min=200", "after", "triage", "=", "n/a", "forbid", "bug.status", "==", "fixed", "forbid", "bug.status", "==", "approved", "forbid", "bug.status", "==", "verified", "forbid", "bug.status", "==", "blocked", "forbid", "bug.status", "==", "escalated", "```", "```gates", "phase=review-plan", "artifact", "{engineering}/bugs/{bug}/BUG_FIX_PLAN.md", "min=200", "forbid", "bug.status", "==", "fixed", "forbid", "bug.status", "==", "approved", "forbid", "bug.status", "==", "verified", "forbid", "bug.status", "==", "blocked", "forbid", "bug.status", "==", "escalated", "```", "```gates", "phase=implement", "artifact", "{engineering}/bugs/{bug}/TRIAGE.md", "min=200", "forbid", "bug.status", "==", "fixed", "forbid", "bug.status", "==", "approved", "forbid", "bug.status", "==", "verified", "forbid", "bug.status", "==", "blocked", "forbid", "bug.status", "==", "escalated", "```", "```gates", "phase=review-code", "after", "implement", "=", "n/a", "forbid", "bug.status", "==", "fixed", "forbid", "bug.status", "==", "approved", "forbid", "bug.status", "==", "verified", "forbid", "bug.status", "==", "blocked", "forbid", "bug.status", "==", "escalated", "```", "```gates", "phase=approve", "after", "review-code", "=", "approved", "forbid", "bug.status", "==", "fixed", "forbid", "bug.status", "==", "approved", "forbid", "bug.status", "==", "verified", "forbid", "bug.status", "==", "blocked", "forbid", "bug.status", "==", "escalated", "```", "```gates", "phase=commit", "after", "approve", "=", "approved", "forbid", "bug.status", "==", "fixed", "forbid", "bug.status", "==", "approved", "forbid", "bug.status", "==", "verified", "forbid", "bug.status", "==", "blocked", "forbid", "bug.status", "==", "escalated", "```", "```gates", "phase=finalize", "artifact", "{engineering}/bugs/{bug}/INDEX.md", "```", "Note:", "the", "`forbid", "bug.status", "==", "approved", "|", "verified`", "rows", "are", "defensive", "—", "no", "phase", "in", "this", "workflow", "writes", "those", "values,", "and", "a", "follow-up", "cleanup", "should", "drop", "them", "from", "`bug.schema.json`", "entirely.", "Until", "then,", "these", "gates", "halt", "any", "LLM-improvised", "attempt", "to", "land", "in", "the", "run-task", "trap", "(see", "today's", "regression).", "<!--", "See", "_fragments/generation-instructions.md", "for", "Generation", "Instructions", "template", "(fix-bug", "uses", "orchestrator-special", "long-form", "prose", "—", "cannot", "be", "reduced", "to", "standard", "subsections)", "-->", "##", "Generation", "Instructions", "Mirrors", "`meta-orchestrate.md", "§", "Generation", "Instructions`", "with", "these", "adjustments:", "-", "The", "generated", "`fix_bug.md`", "must", "include", "the", "**Triage", "Judgement**", "section", "verbatim,", "including", "the", "JSON", "shape,", "the", "Path", "A", "eligibility", "criteria,", "and", "the", "path-selection", "algorithm.", "-", "`ROLE_TO_NOUN`", "MUST", "include", "`triage`", "and", "`finalize`", "in", "addition", "to", "the", "five", "run-task", "roles.", "-", "The", "generated", "orchestrator", "MUST", "read", "`summaries.triage.route`", "and", "select", "`phases_A`", "or", "`phases_B`", "before", "entering", "the", "main", "loop.", "-", "Sub-workflow", "invocations", "pass", "`--bug", "{bugId}`", "(mirroring", "`--task", "{taskId}`).", "-", "Event", "emission", "uses", "`sprint_or_bug_id="bugs"`", "(the", "reserved", "virtual", "sprint", "dir;", "matches", "`validate-store.spec.md`", "and", "the", "reservation", "in", "`store-cli.cjs`).", "-", "Build-overlay", "uses", "`--bug", "{bugId}`", "and", "falls", "back", "to", "the", "bug", "record's", "`path`", "field", "for", "KB", "context", "if", "the", "overlay", "is", "empty.", "-", "Subagent", "personas,", "banners,", "and", "skills", "are", "loaded", "by", "noun", "via", "`{persona_noun}.md`", "and", "`{persona_noun}-skills.md`", "—", "never", "by", "phase", "role.", "-", "Phase", "banners", "are", "orchestrator-owned", "(see", "`meta-orchestrate.md`).", "Subagent", "prompts", "MUST", "NOT", "contain", "banner", "commands.", "-", "Include", "the", "cluster-detection", "block,", "model-resolution", "dispatch,", "progress", "IPC,", "phase-exit", "signals,", "post-phase", "`/compact`", "with", "checkpoint", "line", "`[checkpoint]", "bug={bug_id}", "phase={phase.role}", "iterations={iteration_counts}`,", "and", "context-pack", "injection", "—", "identical", "to", "`meta-orchestrate.md`.", "-", "Token", "reporting:", "see", "`_fragments/finalize.md`", "—", "wire", "via", "`file_ref:`", "in", "each", "subagent", "prompt.", "-", "Sub-workflows", "(`plan_task.md`,", "`implement_plan.md`,", "`review_plan.md`,", "`review_code.md`,", "`architect_approve.md`,", "`commit_task.md`)", "MUST", "detect", "the", "`--bug`", "flag", "and", "resolve", "their", "entity", "context", "against", "the", "bug", "record;", "they", "share", "the", "gate-and-verdict", "machinery", "with", "their", "`--task`", "paths", "via", "`BUG_PHASE_VERDICT_SOURCE`", "in", "`read-verdict.cjs`.", "<!--", "See", "_fragments/iron-laws.md", "for", "Iron", "Laws", "section", "structure", "guidance", "(fix-bug", "uses", "orchestrator-special", "deferral", "to", "generic-skills.md", "§", "Orchestrator", "Iron", "Laws)", "-->", "##", "Iron", "Laws", "<!--", "Shared", "orchestrator", "laws", "live", "in", "generic-skills.md", "§", "Orchestrator", "Iron", "Laws.", "-->", ">", "See", "`generic-skills.md", "§", "Orchestrator", "Iron", "Laws`", "for", "the", "six", "universal", ">", "laws", "that", "apply", "to", "all", "orchestrators.", "**Additional", "laws", "specific", "to", "fix-bug:**", "1.", "**Path", "is", "decided", "once.**", "The", "triage", "subagent", "records", "`summaries.triage.route`.", "The", "orchestrator", "selects", "the", "phase", "list", "and", "does", "not", "switch", "paths", "mid-run.", "If", "the", "architect", "or", "supervisor", "concludes", "Path", "A", "was", "wrong,", "the", "verdict", "is", "`revision`", "—", "re-enter", "the", "loop,", "escalate", "on", "exhaustion.", "Never", "silently", "promote", "a", "Path", "A", "run", "into", "Path", "B.", "2.", "**No", "status", "writes", "outside", "owned", "phases.**", "Only", "`triage`", "(`reported", "→", "triaged", "→", "in-progress`)", "and", "`commit`", "(`in-progress", "→", "fixed`)", "write", "`bug.status`.", "No", "phase", "writes", "`approved`", "or", "`verified`.", "No", "phase", "writes", "anything", "in", "finalize.", "LLM", "improvisation", "that", "mirrors", "a", "task", "workflow's", "status", "writes", "is", "a", "violation;", "the", "gates", "catch", "it,", "the", "iron", "law", "names", "it.", "3.", "**No", "silent", "skipping.**", "A", "bug", "at", "`fixed`/`abandoned`/`blocked`/`escalated`", "is", "skipped", "at", "pre-loop", "with", "one", "`bug_skipped`", "event.", "Skipping", "inside", "the", "phase", "loop", "(writing", ""phase", "skipped"", "summaries)", "is", "forbidden", "—", "that", "pattern", "produced", "the", "inconsistent-skip", "drift", "that", "surfaced", "today's", "regression.", "##", "Friction", "Emit", "When", "the", "Bug", "Fixer,", "Supervisor,", "Architect,", "Engineer,", "or", "Collator", "detects", "skill", "friction", "during", "fix-bug", "—", "a", "referenced", "skill", "is", "unused,", "fails", "on", "invocation,", "is", "missing", "from", "the", "registry,", "has", "gone", "stale", "relative", "to", "current", "architecture,", "or", "is", "redundant", "with", "another", "skill", "—", "emit", "a", "`friction`", "event", "so", "`/forge:rebuild", "--enrich`", "(phase", "2)", "can", "act", "on", "the", "signal.", "**Trigger", "conditions**", "(set", "`issue`", "to", "the", "matching", "token):", "|", "Token", "|", "When", "to", "emit", "|", "|--------------------|----------------------------------------------------------------------------------|", "|", "`skill_unused`", "|", "A", "skill", "listed", "in", "the", "persona's", "skill", "block", "was", "loaded", "but", "never", "consulted.", "|", "|", "`skill_failed`", "|", "A", "skill", "was", "consulted", "but", "its", "guidance", "produced", "an", "error", "or", "required", "correction.", "|", "|", "`skill_missing`", "|", "The", "workflow", "needed", "guidance", "the", "available", "skills", "did", "not", "cover.", "|", "|", "`skill_stale`", "|", "A", "skill's", "guidance", "contradicts", "current", "architecture", "/", "supersedes", "its", "own", "advice.", "|", "|", "`skill_redundant`", "|", "Two", "skills", "provided", "overlapping", "or", "conflicting", "guidance", "for", "the", "same", "decision.", "|", "**Recording", "friction", "(subagent", "side):**", "call", "`node", ".forge/tools/friction-emit.cjs`", "`--workflow", "{workflow}", "--persona", "{persona-noun}", "--issue", "{token}", "[--subkind", "{token}]", "[--evidence", "'{...}']`.", "The", "tool", "appends", "one", "judgement-only", "record", "to", "`.forge/cache/FRICTION-{workflow}.jsonl`.", "The", "orchestrator", "drains", "the", "file", "at", "phase-end,", "stamps", "runtime", "attribution", "(model,", "provider,", "usage,", "wall", "times,", "eventId)", "onto", "each", "record,", "and", "emits", "the", "events", "via", "`store-cli", "emit`", "as", "event", "type", "`"friction"`.", "The", "schema", "enforces", "`{workflow,", "persona,", "issue}`", "as", "required", "when", "`type", "===", ""friction"`;", "`subkind`", "is", "the", "frozen", "enum", "`skill_unused|skill_failed|skill_missing|skill_stale|skill_redundant`", "or", "experimental", "`^x_[a-z_]+$`.", "Emit", "one", "record", "per", "distinct", "friction", "signal", "—", "do", "not", "coalesce.", "##", "Progress", "Reporting", "<!--", "See", "_fragments/progress-reporting.md", "for", "canonical", "definition", "-->", ">", "See", "`_fragments/progress-reporting.md`", "for", "the", "full", "progress", "log", "format", "and", "`store-cli", "progress`", "command", "reference.", "Log", "path:", "`.forge/store/events/bugs/progress.log`.", "Agent", "name", "format:", "`{bugId}:{persona_noun}:{phase.role}:{iteration}`.", "Clear", "at", "bug", "start:", "`store-cli", "progress-clear", "bugs`.", "##", "Phase-Exit", "Signals", "After", "each", "subagent", "returns:", "`✓`", "for", "completed/approved,", "`↻`", "for", "revision", "required", "(with", "iteration", "count),", "`⚠`", "for", "escalated.", "Format", "mirrors", "`meta-orchestrate.md", "§", "Phase-Exit", "Signals`", "with", "`bug_id`", "in", "place", "of", "`task_id`.", "##", "Event", "Emission", "<!--", "See", "_fragments/event-emission-schema.md", "for", "canonical", "contract", "-->", ">", "See", "`_fragments/event-emission-schema.md`", "for", "the", "actor", "split", "(subagent", ">", "writes", "judgement-only", "SUMMARY;", "orchestrator", "composes", "the", "canonical", "event", ">", "from", "runtime", "telemetry", "+", "SUMMARY", "and", "emits", "it),", "and", ">", "`_fragments/event-vocabulary.md`", "§", "Bug", "pipeline", "for", "the", "canonical", ">", "phase→`type`", "token", "table", "(pass/fail", "tokens", "per", "phase,", "`bug-skipped`", ">", "pre-loop", "skip", "shape).", "Emitted", "tokens", "MUST", "come", "from", "that", "table.", "The", "orchestrator", "is", "the", "only", "actor", "that", "calls", "`store-cli", "emit`", "for", "phase", "events.", "All", "bug-phase", "events", "use", "`sprintId="bugs"`", "(the", "reserved", "virtual", "sprint", "dir).", "The", "schema's", "`event.bugId`", "field", "carries", "the", "originating", "bug", "ID", "for", "cross-bug", "filtering", "at", "collate", "time.", "Subagents", "write", "`{PHASE}-SUMMARY.json`", "and", "return;", "the", "orchestrator", "composes", "the", "canonical", "event", "and", "emits", "it."] })
