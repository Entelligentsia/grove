---
requirements:
  reasoning: High
  context: High
  speed: Low
audience: orchestrator-only
deps:
  personas: [engineer]
  skills: [engineer, generic]
  templates: []
  sub_workflows: []
  kb_docs: []
  config_fields: [paths.engineering]
---


# Enhancement Agent

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance -->
## Iron Laws

- Orchestrator-only: this workflow runs with full tool access in the orchestrator session. NEVER delegate it to a subagent.
- Read `.forge/personas/engineer.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store` (or `node .forge/tools/store-cli.cjs`). Never edit `.forge/store/*.json` directly.
- Phase 1 only touches `{{KEY}}` token text; never rewrite persona prose, algorithm steps, or role definitions.

<!-- See _fragments/store-write-verification.md — NOTE: this file uses an intentionally abbreviated
     Store-Write Verification variant (4-line condensed form for orchestrator-only workflow).
     Canonical fragment is reference only. -->
## Store-Write Verification

Every `forge_store` write MUST succeed before advancing. If `store-cli` exits
non-zero or the `PreToolUse` write-boundary hook blocks the call (exit 2):

1. Parse the structured error.
2. Correct the JSON to satisfy the schema.
3. Retry. Repeat up to 3 times. After 3 failures, halt and escalate.

Never set `FORGE_SKIP_WRITE_VALIDATION=1` — operator-only emergency switch.
## Note on `.forge/enhancement-proposals/` directory

Phases 2 and 3 write proposal artifacts to `.forge/enhancement-proposals/`. This directory is
distinct from `.forge/enhancements/` (FR-007, S14 scope). This workflow uses `mkdir -p` before
writing the first proposal artifact to avoid assuming the directory exists. No conflict with S14.

### Sub-directory: `.forge/enhancement-proposals/queue/`

FORGE-S24-T07 introduces a project-local **enhancement queue** at
`.forge/enhancement-proposals/queue/<sprintId>/<taskId>-<ts>.json` — one file per
per-task curator run (T10). The queue is **append-only**: each curator run writes
a fresh file (the ISO compact `<ts>` suffix differentiates writes; nothing is
overwritten). Phase 2 drains the queue at sprint close, dedupes by
`{op, target_path, sha256(diff_body)}`, and feeds the merged batch into the
existing recurrence → delete-candidate → compression-gate → judge pipeline.
The result: **one batched review prompt per sprint, not one per task** (paper
§3.2.1 grouped reward). The drain is read-only — Phase 2 never deletes queue
files; operators triage them during retrospective if needed.

**Per-task curator (T10) write contract.** A curator MUST write via
`forge/tools/queue-drain.cjs` to preserve the append-only invariant:

```sh
node -e "
const { appendToQueue } = require('./forge/tools/queue-drain.cjs');
appendToQueue({
  queueRoot: '.forge/enhancement-proposals/queue',
  sprintId:  process.env.FORGE_SPRINT_ID,
  taskId:    process.env.FORGE_TASK_ID,
  ts:        new Date().toISOString().replace(/[-:]|\\.\\d{3}/g, ''),
  proposals: PROPOSALS_ARRAY,
});
"
```

`appendToQueue` throws if the exact file path already exists; curators MUST
choose a fresh `ts` per run rather than overwriting. The drain is empty-safe:
if no curator ever wrote (queue dir missing) or no files exist in the sprint
sub-dir, Phase 2 reports "no proposals" and exits cleanly (AC5).

## Confidence gating (Phase 1)

A key substitution is **high-confidence** when there is exactly one unambiguous signal source
(e.g., `scripts.test` in `package.json` is the sole candidate for `{{TEST_COMMAND}}`). It is
**low-confidence** when multiple candidates exist or no signal is found. Only high-confidence
fills are applied automatically. Low-confidence keys are listed in the Phase 1 report and left
unsubstituted for the user to fill manually.

## Phase routing

Receive the phase flag from the command invocation:

| Flag | Mode |
|------|------|
| `--phase 1` or `--auto` | Auto-apply: placeholder fills only — **use after** `/forge:init` completes to fill `{{KEY}}` placeholders from project signals |
| `--phase 2` | Propose-diffs: sprint artifact + friction scan — **use after** a sprint completes to turn friction events into persona/skill enrichments |
| `--phase 3` | Drift detection: full codebase vs structural-element comparison — **use on-demand** or after `/forge:health --fix` to detect stale references |

Default to `--phase 3` if no phase flag is given.

---

## Step 0 — Resolve roots

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT}"`
```

Read `.forge/config.json`. Resolve:
- `PROJECT_ROOT` = current working directory (absolute).
- `ENGINEERING_PATH` = `paths.engineering` from config (default `engineering`).

---

## Phase 1 — Auto-apply placeholder fills

### When to run

Invoked by T09 post-init hook (`--auto`) or manually via `/forge:rebuild --enrich --phase 1`.

### Algorithm

1. **Scan structural elements** for `{{KEY}}` patterns:
   ```sh
   grep -r '{{' "$PROJECT_ROOT/.forge/personas/" "$PROJECT_ROOT/.forge/skills/" \
        "$PROJECT_ROOT/.forge/workflows/" "$PROJECT_ROOT/.forge/templates/" \
        --include="*.md" -l 2>/dev/null
   ```
   Collect each unique `{{KEY}}` token found.

2. **Skip runtime passthrough keys** — keys used at runtime (e.g., `{{TASK_ID}}`, `{{SPRINT_ID}}`,
   `{{ARGUMENTS}}`) are intentional and must not be substituted. Read
   `.forge/tools/substitute-placeholders.cjs` to identify the RUNTIME_PASSTHROUGH_KEYS list.
   Exclude them from the fill candidates.

3. **Derive values from codebase signals** — for each remaining `{{KEY}}`, attempt to derive a
   value with high confidence:

   | Key | Signal source |
   |-----|--------------|
   | `{{STACK_SUMMARY}}` | `package.json` dependencies field (list top-level frameworks); or dominant file extension survey |
   | `{{BRANCHING_CONVENTION}}` | `git branch -a` output pattern or `.git/config` |
   | `{{TEST_COMMAND}}` | `package.json` → `scripts.test` |
   | `{{BUILD_COMMAND}}` | `package.json` → `scripts.build` |
   | `{{ENTITY_MODEL}}` | Scan source for ORM model files or type definitions |
   | `{{KEY_DIRECTORIES}}` | Top-level directory listing (exclude `.git`, `node_modules`, `.forge`) |
   | `{{IMPACT_CATEGORIES}}` | Derive from project type (web app → UI/API/DB/Infra; library → API/Docs/Tests) |

   Any key without a single unambiguous signal → mark as **low-confidence** (skip auto-fill).

4. **Apply high-confidence fills** — for each high-confidence key, perform in-place substitution
   in all structural element files that contain the key. Use `substitute-placeholders.cjs` if it
   supports a targeted single-key mode; otherwise apply the substitution directly with a read/write
   cycle per file.

   **Minimal modification principle**: Phase 1 only touches `{{KEY}}` token text. It never rewrites
   persona prose, algorithm steps, or role definitions.

5. **Update `project-context.json`** — write the newly discovered values into
   `$PROJECT_ROOT/.forge/project-context.json` so that future invocations of
   `substitute-placeholders.cjs` use the derived values.

6. **Snapshot gate** — if at least one fill was applied:
   ```sh
   node .forge/tools/manage-versions.cjs add-snapshot \
     --source post-init \
     --enhanced-elements "<comma-separated list of relative .forge/ paths that were modified>"
   ```
   If no fills were applied, skip the snapshot call entirely.

7. **Emit enhancement event** to the store:
   ```sh
   node .forge/tools/store-cli.cjs emit enhancement '{
     "eventId": "<ISO timestamp slug>_enhance_phase1",
     "taskId": "enhancement",
     "sprintId": "enhancement",
     "role": "enhancement-agent",
     "action": "enhancement-completed",
     "phase": "post-init",
     "iteration": 1,
     "notes": "{\"phase\":1,\"fillCount\":<N>,\"snapshotCreated\":<true|false>}"
   }'
   ```

8. **Report**:
   ```
   ## Phase 1 Enhancement Complete

   Fills applied: N key(s) — {{KEY1}}, {{KEY2}}, ...
   Uncertain keys (not filled): M — {{KEY3}}, ...  (manual intervention needed)
   Snapshot: [written as snap-{index}] | [skipped — no fills]
   ```

---

## Phase 2 — Propose enrichment diffs (post-sprint)

### When to run

Invoked by T09 post-sprint hook or manually via `/forge:rebuild --enrich --phase 2`.

### Algorithm

1. **Collect friction events**:
   ```sh
   node -e "
   const fs = require('fs'), path = require('path');
   const eventsDir = '.forge/store/events';
   const allFiles = fs.readdirSync(eventsDir, { withFileTypes: true })
     .flatMap(d => d.isDirectory()
       ? fs.readdirSync(path.join(eventsDir, d.name)).map(f => path.join(eventsDir, d.name, f))
       : [path.join(eventsDir, d.name)]
     )
     .filter(f => f.endsWith('.json'));
   const friction = allFiles
     .map(f => { try { return JSON.parse(fs.readFileSync(f,'utf8')); } catch { return null; } })
     .filter(e => e && e.type === 'friction');
   console.log(JSON.stringify(friction));
   "
   ```

1a. **Drain enhancement queue** (FORGE-S24-T07) — read per-task curator
   proposals from `.forge/enhancement-proposals/queue/<sprintId>/`, dedupe by
   `{op, target_path, sha256(diff_body)}`, and produce a `queuedProposals`
   array that joins the synthesised proposals from step 5. This is what makes
   the review **batched** rather than per-task (paper §3.2.1):

   ```sh
   node -e "
   const { drainQueue } = require('./forge/tools/queue-drain.cjs');
   const drained = drainQueue({
     queueRoot: '.forge/enhancement-proposals/queue',
     sprintId:  process.env.FORGE_SPRINT_ID,
   });
   process.stdout.write(JSON.stringify(drained));
   "
   ```

   Contract (per `forge/tools/queue-drain.cjs`):
   - Returns `{ proposals: [...], files: [...], errors: [...] }`. `proposals`
     is the deduped union of every per-task curator file in the sprint
     sub-dir. `files` is the lexicographic-sorted list of source paths (used
     by step 6 to log provenance). `errors` carries any malformed JSON files
     skipped during read — log them, do not abort.
   - Empty / missing queue → empty result. The drain never throws on absent
     queue dir (first-run or no curators registered yet, AC5).
   - The drain is read-only. Operators are responsible for queue triage
     after sprint close.

2. **Zero-input guard**: If both the friction event list AND `queuedProposals`
   are empty, print:
   ```
   No friction events or queued proposals for the active sprint — nothing to enhance.
   ```
   and exit Phase 2 immediately (skip steps 3–9; emit the enhancement event with `"notes": "{\"phase\":2,\"frictionCount\":0,\"queuedCount\":0}"`). Do not create `.forge/enhancement-proposals/` when there are no proposals.

3. **Deduplicate** friction events by composite key `workflow + persona + issue`. Keep the most
   recent occurrence of each composite key.

4. **Read most recent completed sprint** from `.forge/store/sprints/` (status `done` or
   `retrospective-done`), sorted by completion date. Read its task records from
   `.forge/store/tasks/` filtered by the sprint ID.

5. **Synthesize enrichment proposals** — for each friction event, classify the proposed
   change into exactly one of three ops (see `forge/schemas/proposal.schema.json`):

   | `op`            | When to use                                                                 |
   |-----------------|-----------------------------------------------------------------------------|
   | `insert_skill`  | A new skill / persona / kb_docs reference is needed; target file does not yet carry the guidance. |
   | `update_skill`  | An existing skill or persona file needs revised guidance — e.g., add a routing pattern reference to `deps.kb_docs`, replace a stale instruction. |
   | `delete_skill`  | A skill is unused, redundant, or stale (`skill_unused` / `skill_redundant` / `skill_stale` friction subkinds); target file or section should be removed. |

   For each proposal capture **at minimum** the schema-required triplet
   `{op, target_path, diff_body}` plus optional `rationale` and `sourceFrictionIds`.
   `sourceFrictionIds` MUST carry the `eventId` of every friction event that
   contributed to the proposal — the next step depends on it to resolve the
   originating task for the recurrence scan.
   For large committed file sets (> 5 files in the sprint), also check whether
   `engineer-skills.md` or `architect-skills.md` should be updated (`update_skill`).
   The op classification is the foundation for the downstream judge (T03),
   delete-candidate detection (T05), compression gate (T06), and queue drain (T07).

   **Merge with queued proposals (T07).** Concatenate the synthesised
   proposals built in this step with the `queuedProposals` array from
   step 1a, then dedupe the combined array with the same key the drain
   uses (`{op, target_path, sha256(diff_body)}`) so a friction-synthesised
   proposal that happens to be byte-identical to a curator-queued one
   collapses. Use:

   ```sh
   node -e "
   const { dedupeProposals } = require('./forge/tools/queue-drain.cjs');
   // synthesised = proposals built above from friction events.
   // queued      = drained.proposals from step 1a.
   const merged = dedupeProposals(synthesised.concat(queued));
   process.stdout.write(JSON.stringify(merged));
   "
   ```

   The merged array is what feeds steps 5a (recurrence) → 5b (delete
   candidates) → 5b.5 (compression gate) → 5c (judge) — a single batched
   pipeline, never one per task (AC4).

5a. **Cross-task replay scoring (recurrence boost)** — before writing the
   artifact, stamp each proposal with `recurrence_count` and
   `recurrence_task_ids` so the T03 judge can score "this friction recurred
   across N tasks" rather than treating every signal as a singleton:

   ```sh
   node -e "
   const { annotateProposals } = require('./forge/tools/replay-scoring.cjs');
   // friction = deduped friction events from step 3, each carrying eventId,
   //            taskId, subkind, evidence.skillId (orchestrator-stamped).
   // proposals = array built in step 5.
   // taskOrder = task IDs of the most-recent sprint sorted by completion
   //             order — same source as step 4.
   const annotated = annotateProposals(proposals, friction, taskOrder);
   process.stdout.write(JSON.stringify(annotated));
   "
   ```

   Contract (per `forge/tools/replay-scoring.cjs`):
   - `recurrence_count` is the number of distinct tasks (origin task + later
     tasks in `taskOrder`) whose friction events match the proposal's
     originating `(subkind, evidence.skillId)` pair. Always `>= 1`.
   - `recurrence_task_ids` is the `taskOrder`-sorted list of those task IDs.
   - Proposals whose `sourceFrictionIds` cannot be resolved (no matching
     `eventId` in the friction set, or the resolved event lacks
     `subkind`/`evidence.skillId`) receive `recurrence_count: 1` and an empty
     `recurrence_task_ids: []` — neutral signal, not silent failure.
   - The annotator returns new proposal objects; the input array is not
     mutated.

5b. **Delete-candidate detection (3-sprint zero-use)** — scan `skill_usage`
   events across the trailing 3 sprints and emit a `delete_skill` proposal
   for every skill with zero retrieval AND zero invocation across the
   window. This is the only mechanism by which the skill repository shrinks:

   ```sh
   node -e "
   const { buildDeleteProposals } = require('./forge/tools/delete-candidate-detector.cjs');
   // skillUsageEvents = all events with type === 'skill_usage' across the
   //                    sprints in scope (collected via the same Step 1
   //                    walker, filtered by type instead of friction).
   // sprintOrder      = sprint IDs sorted by completion order (oldest →
   //                    newest). The detector takes the trailing windowSize
   //                    entries.
   // windowSize       = 3 by default; configurable. Defined as the trailing
   //                    N sprints of sprintOrder.
   // targetPathFor    = (skillId) => the on-disk path of the skill file to
   //                    delete. Workflow chooses the mapping convention.
   const deletes = buildDeleteProposals({
     events:        skillUsageEvents,
     sprintOrder,
     windowSize:    3,
     targetPathFor: (skillId) => 'forge/skills/' + skillId + '.md',
   });
   process.stdout.write(JSON.stringify(deletes));
   "
   ```

   Append the resulting `delete_skill` proposals to the proposal array from
   step 5/5a before step 6. Each delete proposal already carries
   `recurrence_count: 1` and `recurrence_task_ids: []` (the annotator from
   step 5a is for friction-derived proposals; delete candidates come from
   usage telemetry, not friction, so recurrence is neutral by construction).

5b.5. **Compression gate (reject >20% growth without 3+ frictions)** — a cheap
   deterministic filter that runs BEFORE the LLM judge (step 5c).

   **Before writing any rejections**, ensure the target directory exists:
   ```sh
   mkdir -p "$PROJECT_ROOT/.forge/enhancement-proposals"
   ```

   All gate and judge rejections MUST be written to:
   ```
   $PROJECT_ROOT/.forge/enhancement-proposals/phase2-<timestamp>-rejections.json
   ```
   NEVER write a bare `rejections.json` to the project root or any other location.

   Any
   `update_skill` proposal that would grow the target file by more than 20%
   (byte-wise, UTF-8) must be backed by at least 3 supporting friction events;
   otherwise it is rejected here and never reaches the judge. `insert_skill`
   and `delete_skill` proposals pass through unconditionally — insert growth
   is handled by the judge's `body_under_2kb` axis and delete only shrinks.

   Why a pre-judge gate? Judging is expensive. Unbounded skill-body growth is
   the classic SkillOS failure mode — pasting pages of trajectory copy-paste to
   "patch" a friction. It is cheap to detect deterministically and wasteful to
   ask the judge to rule on.

   ```sh
   node -e "
   const fs = require('node:fs');
   const path = require('node:path');
   const { filterProposals } = require('./forge/tools/compression-gate.cjs');
   // proposals = post-5b array (synthesis + recurrence + delete-candidates).
   // PROJECT_ROOT resolves the target_path; forge plugin source is the source
   // of truth for current bodies. The workflow renders the diff via its own
   // applyProposalDiff helper (left abstract here — the gate is body-agnostic).
   const projectRoot = process.env.PROJECT_ROOT;
   const result = filterProposals({
     proposals,
     currentBodyFor: (p) => {
       const abs = path.join(projectRoot, p.target_path);
       try { return fs.readFileSync(abs, 'utf8'); }
       catch (e) { return ''; } // insert_skill or missing file → empty
     },
     newBodyFor: (p) => applyProposalDiff(currentBodyFor(p), p),
     // Default supporting count = proposal.sourceFrictionIds.length. Override
     // if the policy is 'count frictions citing the same skill across the
     // sprint' rather than 'count citations on the proposal itself'.
   });
   const proposalsAfterGate = result.admitted;
   const compressionRejections = result.rejected; // [{ proposal, ...evaluation }]
   process.stdout.write(JSON.stringify({ kept: proposalsAfterGate, rejected: compressionRejections }));
   "
   ```

   **Logging gate rejections.** Append every rejection from this step to
   `$PROJECT_ROOT/.forge/enhancement-proposals/phase2-<timestamp>-rejections.json`
   (the same file the LLM-judge gate in step 5c writes to). The rejection
   record carries `{ proposal, admit: false,
   reason: 'compression_gate_growth_unsupported', growthRatio, currentBytes,
   newBytes, supportingFrictionCount, threshold, minSupportingFrictions }`.
   This keeps every drop — gate or judge — traceable in one place.

   Contract (per `forge/tools/compression-gate.cjs`):
   - `GROWTH_THRESHOLD === 0.20`; comparison is **strict** (`> 0.20`). A
     proposal at exactly 20% growth admits without friction support.
   - `MIN_SUPPORTING_FRICTIONS === 3`. Two or fewer citations is not enough.
   - An update on an empty current body yields `growthRatio: Infinity`; the
     friction-support rule still applies.
   - Negative growth (shrink) admits unconditionally.
   - `filterProposals` partitions the input array preserving order; the
     output `rejected` array carries the structured evaluation alongside the
     original proposal.

5c. **LLM-judge gate (Sonnet rubric, drop <3/5)** — score every proposal
   against the 5-axis rubric and drop low-signal proposals before
   presentation. The rubric is single-sourced in
   `forge/tools/judge-proposal.cjs`:

   | Axis (0..5) | What it measures |
   |---|---|
   | `specificity` | Names a concrete target_path beyond `forge/skills/*` floor; carries a non-trivial rationale; recurrence trail boosts. |
   | `when_not_to_use` | Body contains a literal "When NOT to use" section. |
   | `no_trajectory_copy_paste` | No long verbatim runs or unbroken non-whitespace blocks (>= 400 bytes) that suggest pasted trajectory log. |
   | `body_under_2kb` | `Buffer.byteLength(diff_body, 'utf8') <= 2048`. |
   | `cites_friction` | Proposal carries at least one `sourceFrictionIds` entry; multiple citations or recurrence boost the score. |

   For each proposal in the post-5b array, the workflow asks Sonnet to
   apply the rubric and emit per-axis 0..5 scores; in the absence of an
   LLM call, the deterministic `scoreProposal(proposal)` helper in
   `judge-proposal.cjs` is used as both the fallback scorer and the
   validation contract for Sonnet-produced scores (single source of truth
   for the rubric definition).

   ```sh
   node -e "
   const {
     scoreProposal,
     decideJudgement,
   } = require('./forge/tools/judge-proposal.cjs');
   // proposals = post-5b array of proposal records.
   const judged = proposals.map((p) => {
     const scored   = scoreProposal(p);
     const decision = decideJudgement(scored);
     return { proposal: p, ...decision };
   });
   const kept    = judged.filter((j) => j.verdict === 'keep').map((j) => j.proposal);
   const dropped = judged.filter((j) => j.verdict === 'drop');
   process.stdout.write(JSON.stringify({ kept, dropped }));
   "
   ```

   Contract (per `forge/tools/judge-proposal.cjs`):
   - `scoreProposal(proposal)` returns `{ axes, average }` with `axes`
     keyed by every entry in `RUBRIC_AXES` and `average` rounded to one
     decimal place.
   - `decideJudgement({ axes })` returns
     `{ verdict, average, axes, reason }`. `verdict === 'drop'` iff
     `average < 3` (strictly less than); ties at exactly 3.0 keep.
   - `decideJudgement` fails loud on missing or out-of-range axes — the
     judge will NOT silently coerce a malformed score sheet into a verdict.

   **Logging dropped proposals (AC3).** Every rejection MUST be persisted
   for retro review. Replace the proposal array passed to step 6 with the
   `kept` list, and append the `dropped` list to
   `$PROJECT_ROOT/.forge/enhancement-proposals/phase2-<timestamp>-rejections.json`
   as a sibling artifact. Each rejection record carries the original
   proposal alongside `{ verdict: 'drop', average, axes, reason }`. The
   markdown summary written in step 6 SHOULD include a "Dropped (N)" line
   pointing at the rejections file when N > 0.

   **Carry-over caveat** — the rubric is deterministic; Sonnet's role is
   to add semantic judgement to axes that the heuristic scorer
   approximates (specificity in particular). When Sonnet is invoked, its
   per-axis scores MUST be validated against the 0..5 range via the same
   `validateAxes` invariant `decideJudgement` enforces. Operators
   investigating an unexpected drop should consult the per-axis trace in
   `reason`.

   Contract (per `forge/tools/delete-candidate-detector.cjs`):
   - A skill qualifies for deletion iff it has at least one `skill_usage`
     event inside the trailing window AND every in-window observation has
     `retrieved === false` AND `used === false`. Any single `retrieved: true`
     or `used: true` event disqualifies the skill.
   - Skills with zero observations in the window are NOT proposed — this
     case is indistinguishable from a newly-added skill that hasn't been
     loaded yet, so silence is the safe default.
   - Each proposal carries `window_size`, `window_sprint_ids`, and a
     `sourceFrictionIds: []` (delete candidates derive from usage telemetry,
     not friction).

   **Carry-over caveat** — the trailing-3-sprint window is only meaningful
   once 3 sprints have actually elapsed since `skill_usage` event emission
   landed in FORGE-S24-T01 (forge 0.45.1). During the carry-over period the
   detector still runs over whatever sprintOrder it receives, but the
   signal is noisier: a skill flagged after only one or two sprints of
   history may simply be new or temporarily idle. Operators should treat
   delete proposals from short-history runs as advisory until the full
   window is populated.

   **Verification (after steps 5b.5 + 5c).** If any proposals were
   rejected by either gate, verify the rejections file landed at the
   contracted path:
   ```sh
   ls "$PROJECT_ROOT/.forge/enhancement-proposals/phase2-"*"-rejections.json"
   ```
   If the file is missing but rejections were recorded, the write targeted
   the wrong path. Check for a stray `rejections.json` at the project root
   and move it:
   ```sh
   [ -f "$PROJECT_ROOT/rejections.json" ] && \
     mv "$PROJECT_ROOT/rejections.json" \
        "$PROJECT_ROOT/.forge/enhancement-proposals/phase2-${TIMESTAMP}-rejections.json"
   ```
   NEVER leave a bare `rejections.json` at the project root.

6. **Write proposal artifact**:
   ```sh
   mkdir -p "$PROJECT_ROOT/.forge/enhancement-proposals"
   ```
   Write **two** outputs for each Phase 2 run (using the `kept` list from
   step 5c — dropped proposals are persisted separately to the
   `phase2-<timestamp>-rejections.json` sibling described in step 5c):

   - `phase2-<timestamp>.md` — human-readable markdown, one section per proposal,
     showing op + target_path + a fenced diff block.
   - `phase2-<timestamp>.json` — machine-readable array of proposal records, each
     conforming to `forge/schemas/proposal.schema.json` (required keys: `op`,
     `target_path`, `diff_body`; `op` ∈ {insert_skill, update_skill, delete_skill};
     optional `recurrence_count` ≥ 1 and `recurrence_task_ids` populated by step 5a).

   **Back-compat on read** — pre-0.45.2 proposal records lack `op`. Downstream
   consumers MUST route legacy records through
   `forge/tools/proposal-normalize.cjs:normaliseProposal()` which defaults the
   missing `op` to `insert_skill` (the only op the prior insert-biased flow
   could produce). Do NOT silently coerce — call the helper explicitly so the
   normalisation is auditable.

7. **Present to user**:
   ```
   ## Phase 2 Enhancement Proposals

   N change(s) proposed — review: .forge/enhancement-proposals/phase2-<timestamp>.md

   [A] Apply all  [r] Review individually  [n] Skip
   ```

8. **On approval** — for each approved change:
   - Apply the edit in-place.
   - Call `manage-versions.cjs add-snapshot --source post-sprint:<SPRINT_ID> --enhanced-elements <list>`.

9. **Emit enhancement event** (same schema as Phase 1, with `"phase": "post-sprint"`).

10. **Report**: N changes applied, M skipped, snapshot written or skipped.

---

## Phase 3 — Drift detection (on-demand / delegated from calibrate)

### When to run

Invoked by `/forge:rebuild --enrich --phase 3` (default when no phase given), or delegated by
`/forge:health --fix` after its Step 4 drift categorization.

### Algorithm

1. **Read codebase state** from `$PROJECT_ROOT/.forge/project-context.json`:
   key directories, entities, commands, stack summary, test command, build command.

2. **Read all structural elements** from `.forge/personas/`, `.forge/skills/`,
   `.forge/workflows/`, `.forge/templates/`.

3. **Compare**: for each structural element, verify:
   - It correctly references known entities and key directories.
   - It uses valid `{{KB_PATH}}` references (paths that exist in `engineering/`).
   - Its `deps.kb_docs` list includes docs referenced in its body.

4. **Read friction events** (same collection as Phase 2 Step 1).

5. **Read `calibrationBaseline`** from `$PROJECT_ROOT/.forge/config.json` to understand what
   was last confirmed correct.

6. **Write drift report**:
   ```sh
   mkdir -p "$PROJECT_ROOT/.forge/enhancement-proposals"
   ```
   Write to `$PROJECT_ROOT/.forge/enhancement-proposals/phase3-<timestamp>.md`.

7. **Present to user**:
   ```
   ## Phase 3 Drift Report

   N discrepancy(ies) found — review: .forge/enhancement-proposals/phase3-<timestamp>.md

   [A] Apply all  [r] Review individually  [n] Skip
   ```

8. **On approval** — apply changes and call `manage-versions.cjs add-snapshot --source on-demand`.

9. **Emit enhancement event** (`"phase": "on-demand"`).

10. **Report**: N changes applied, snapshot written or skipped.

---

## On error

If any step fails unexpectedly, describe what went wrong and offer:

> "This looks like a Forge bug. Would you like to file a report? Run `/forge:report-bug`."

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->