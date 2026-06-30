---
requirements:
  reasoning: High
  context: Medium
  speed: Medium
audience: subagent
phase: triage
context:
  architecture: false
  prior_summaries: delta
  persona: summary
  master_index: false
  diff_mode: false
deps:
  personas: [bug-fixer]
  skills: [bug-fixer, generic]
  templates: []
  sub_workflows: []
  kb_docs: [architecture/stack.md]
  config_fields: [commands.test, paths.engineering]
---


# Bug Triage
## Iron Laws

- Reproduce the bug before deciding anything. A bug without a confirmed
  reproduction has no business going to plan-fix or implement.
- Read `.forge/personas/bug-fixer.md` first; print the persona identity
  line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store` (or `node .forge/tools/store-cli.cjs`).
  Never edit `.forge/store/*.json` directly.
- **Triage NEVER writes `bug.status`.** The orchestrator (`meta-fix-bug.md`)
  owns the `reported → triaged` and `triaged → in-progress` transitions.
  Writing `bug.status` from this workflow violates `meta-fix-bug.md
  § Iron Laws #2` (parallel to `meta-review-plan.md`'s "Supervisor NEVER
  writes entity status" rule).
- Triage is a **single phase**. Do NOT call `forge_preflight` with any
  other `--phase` value. Do NOT call `forge_store update-status`,
  `set-bug-summary` for any non-triage phase, or `forge_store emit`.
  These are orchestrator-owned or other-phase-owned actions. The
  phase-ownership guard in forge-cli will reject violations at the tool
  layer, but the Iron Law names the rule.

## Store-Write Verification

<!-- See _fragments/store-write-verification.md for the canonical block content -->

## Algorithm

```

0. Pre-flight Gate Check:
   - Run: `node .forge/tools/preflight-gate.cjs --phase triage --bug {bugId}`
   - Exit 1 (gate failed) → print stderr and HALT. Do not proceed.
   - Exit 2 (misconfiguration) → print stderr and HALT.
   - Exit 0 → continue.

1. Load Context:
   - Read `.forge/personas/bug-fixer.md` first; print the persona identity
     line to stdout before any other tool use.
   - Read the bug record:
     `forge_store({ command:"read", args:["bug", "{bugId}"] })`
   - Read business domain docs relevant to the reported symptom.
   - store-cli verbs: `read` | `list` | `write` | `emit` |
     `update-status` | `set-summary` | `set-bug-summary` | `describe` |
     `nlp` | `query` | `delete` — there is no `get`/`set`/`find`. See
     `_fragments/store-cli-verbs.md` for full notes.

2. Reproduce:
   - Construct a minimal reproduction: a failing test, a short script,
     or a documented manual sequence that triggers the reported symptom.
   - If reproduction cannot be achieved with the information in the bug
     record, write what was tried in TRIAGE.md, set the route to "B"
     (any uncertainty defaults to Path B), and continue to root-cause
     research with the reporter's narrative as the working hypothesis.

3. Root-Cause Research:
   - Read the code paths implicated by the reproduction.
   - Confirm (or revise) the reporter's stated root cause via direct
     inspection of source files and tests.
   - Note collateral damage: which other call sites, schemas, or
     workflows share the defective shape.

4. Path A / Path B Eligibility:
   - Apply the criteria in § "Path A / Path B Eligibility" below.
   - Record the route decision and the explicit enumeration of each
     criterion in the findings section of the triage summary.

5. Write Triage Artifacts:
   - Write the triage artifact (markdown narrative):
     `forge_artifact({ command:"write", entity:"bug", entityId:"{bugId}",
                       artifact:"triage", content:"<markdown>" })`
   - Write the triage-summary sidecar (JSON shape below):
     `forge_artifact({ command:"write", entity:"bug", entityId:"{bugId}",
                       artifact:"triage-summary", content:"<JSON>" })`

6. Finalize:
   - **No status write.** The orchestrator (`meta-fix-bug.md`) writes the
     `reported → triaged` and `triaged → in-progress` transitions on
     return. Writing `bug.status` from this workflow is forbidden by
     Iron Laws above and is rejected by the phase-ownership guard.
   - **Do NOT emit a phase event yourself.** The orchestrator owns event
     emission — it composes the canonical event from runtime telemetry
     (model, provider, tokens, wall times) plus the SUMMARY you write
     in the next step.

7. Emit Summary Sidecar:
   - The JSON written in step 5 MUST have this shape (the `route` field
     is required; allowed values: `"A"` or `"B"`):

     ```json
     {
       "objective":   "Triage FORGE-BUG-NNN — reproduce, locate, decide route.",
       "key_changes": ["<up to 12 bullets, 200 chars each — findings or actions>"],
       "findings": [
         "Root cause: <one line>",
         "Reproduction: <one line>",
         "Route decision: A | B",
         "Rationale: <one line>"
       ],
       "verdict":     "n/a",
       "written_at":  "<current ISO 8601 timestamp>",
       "artifact_ref":"TRIAGE.md",
       "route":       "A"
     }
     ```

   - Call:
     ```
     forge_store({ command:"set-bug-summary", args:["{bugId}", "triage"] })
     // sidecar path auto-resolved from the bug record's `path` — never pass it
     ```
     `forge_store` has only `command` + `args` (positional) — no
     `entity`/`id`/`phase` field. `args[0]` is the bug id, `args[1]` is the
     LITERAL phase key `triage` (never the bug id, never a path). See
     `_fragments/store-cli-verbs.md`.
   - If the set-bug-summary call exits non-zero (phase-ownership guard:
     `expected summary key 'triage'`), `args[1]` was wrong — set it to `triage`
     and retry (up to 3 attempts per the Store-Write Verification rule). Do not
     proceed without a valid summary.

> **Field-naming caution — runtime-tested.** The route field is named
> `route`, never `path`. The bug schema's top-level `path` field is the
> bug's **artifact directory** (e.g. `engineering/bugs/EMG-BUG-001-...`).
> Conflating the two caused EMBERGLOW-BUG-001 (v0.44.0 first run) to land
> its `TRIAGE.md` under `.forge/store/bugs/` instead of `engineering/bugs/`.
> Triage MUST NOT touch `bug.path` — that field is set at bug creation
> and never modified by triage.
```

## Path A / Path B Eligibility

Path A is **eligible only when ALL** of the following hold. The triage
summary `findings` array MUST enumerate each criterion explicitly with a
pass/fail mark, so reviewers can audit the decision:

- `bug.severity ∈ {minor}`
- Fix is contained in a single file
- Estimated diff ≤ ~20 lines (judgement call; one screen)
- No schema, API, migration, security, or build-system change
- A regression test is obvious from the reproduction script (single short
  test case, no new fixtures, no test-harness change)

If any criterion fails, the triage subagent MUST select Path B.

**Path B is the default.** Any uncertainty resolves to Path B. It runs the
same plan/review/implement/review/approve/commit shape as
`meta-orchestrate.md`. Picking Path A under uncertainty is the documented
failure mode (over-eager short-circuit).

## Triage Artifact Contents (TRIAGE.md)

The narrative artifact MUST contain:

1. **Reported symptom** — one paragraph summarising the bug report.
2. **Reproduction** — exact steps, commands, or test case that triggers
   the symptom; copy of the failing output.
3. **Root cause** — one or two paragraphs naming the defective code path,
   schema, or workflow. Cite file paths and line numbers.
4. **Path A / Path B enumeration** — for each criterion above, mark
   pass/fail with one-line evidence.
5. **Route decision and rationale** — the chosen route and the
   single-sentence justification.
6. **Collateral findings** — any related shapes, call sites, or
   workflows that share the defective pattern (filed as follow-ups in
   the commit phase, not fixed here).

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Friction Emit

Emit `type:friction` `{workflow:triage, persona:bug-fixer, issue}` per
`_fragments/friction-emit.md`.
