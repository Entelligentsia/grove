export const meta = {
  name: 'wfl:fix-bug',
  description: 'Code-orchestrated port of /forge:fix-bug — drive a bug through the triage→[plan-fix→review-plan→]implement→review-code→approve→commit→finalize pipeline, with Path A (short) or Path B (full) selected once from summaries.triage.route, all escalation and revision loops held in JS.',
  whenToUse: 'Fix a single Forge bug through the full triage→fix→commit pipeline via a deterministic JS driver. Pass the bug id as args, e.g. args: "FORGE-BUG-001".',
  phases: [
    { title: 'Resolve', detail: 'read the bug record and check the pre-run status guard' },
    { title: 'Triage', detail: 'one subagent reproduces the bug, records root cause, and writes summaries.triage.route=A|B' },
    { title: 'Pipeline', detail: 'per phase: one subagent runs the gate + phase workflow + emits its own event; JS owns the phase index, revision counters, verdict routing, and escalation decision' },
    { title: 'Finalize', detail: 'collate + preflight finalize gate; gate failure escalates without touching bug.status=fixed' },
    { title: 'Report', detail: 'summarise the terminal outcome — fixed / escalated' },
  ],
};

// wfl:fix-bug — a code-orchestrated port of .forge/workflows/fix_bug.md
//
// Why a script: fix_bug.md is a deterministic phase FSM with a triage-branch
// decision (Path A vs Path B), review→revision back-edges, per-phase iter caps,
// declarative pre-flight gates, and escalate-don't-continue on any failure. In the
// LLM orchestrator that loop is hand-run turn-by-turn. Here the JS holds the phase
// index, the revision counters, the verdict routing, and the escalation decision;
// subagents only run a single phase's workflow and write artifacts/events to disk.
//
// SIDE-EFFECT OWNERSHIP — READ BEFORE EDITING:
//   This script has NO filesystem/shell access. Each per-phase subagent owns:
//   preflight-gate, the phase workflow (which writes its own artifacts +
//   {PHASE}-SUMMARY.json + status), read-verdict (review phases), token sidecar,
//   friction drain, AND its own canonical phase event.
//   *** DELIBERATE DEVIATION from fix_bug.md's "the orchestrator is the sole actor
//   that calls store-cli emit" rule: here each phase subagent emits its OWN phase
//   event. This is defensible — the subagent is the only actor that holds its own
//   runtime attribution (model, provider, token usage). The JS driver cannot run
//   store-cli. This is a control-flow-authoritative port with delegated telemetry,
//   NOT a byte-for-byte reproduction of the emit contract.
//   Split start/complete emit contract: per fix_bug.md §Event Emission, each phase
//   subagent emits a start event (action="start") BEFORE executing its phase
//   workflow, then a complete event (action="complete") AFTER. The JS driver
//   delegates wall-time bracketing to the subagent: subagent notes startTimestamp,
//   runs the workflow, then computes durationMinutes = (endMs - startMs) / 60000
//   and includes it in the complete event. The start event carries a 0-duration
//   placeholder (startTimestamp == endTimestamp); the complete event carries the
//   real bracket. This mirrors the fix_bug.md start+complete pattern. ***
//
// MODEL TIERING: each phase dispatched on the tier fix_bug.md § Role-to-Tier
// Mapping prescribes. The Workflow agent() hook takes the tier NAME and the host
// (Claude Code) resolves it to the configured model for that tier.
//
// PATH A / PATH B: triage subagent writes summaries.triage.route = "A" or "B".
// The JS driver reads it after triage and selects the phase list. Selection is
// FINAL — no mid-pipeline switching. If route is neither "A" nor "B", escalate
// verdict_malformed.
//
// VIRTUAL SPRINT DIR: all bug events use sprintId="bugs" (virtual sprint dir).
// The event schema's bugId field carries the originating bug ID for cross-bug
// filtering at collate time.
//
// Invocation (Workflow tool):  { name: 'wfl:fix-bug', args: 'FORGE-BUG-001' }
// args may also be an object: { bugId: 'FORGE-BUG-001' }

// Bug statuses that mean "do not run any phase" — fix_bug.md pre-loop guard.
// Note: terminal status for bugs is "fixed" (not "committed" — that is for tasks).
const SKIP_STATUS = ['blocked', 'escalated', 'fixed', 'abandoned']

// Phase roles whose artifact carries a **Verdict:** that routes the FSM.
// No 'validate' phase in fix_bug — drop it from the task template's list.
// NOTE: 'approve' is NOT here — fix_bug advances it on completion like a
// non-review phase (the approve workflow self-escalates if it rejects).
const REVIEW_ROLES = ['review-plan', 'review-code']

// Bug-phase `type` tokens — verbatim port of the canonical table in
// .forge/workflows/_fragments/event-vocabulary.md § Bug pipeline
// (forge-engineering#39). The COMPLETE event carries the pass token (or the
// fail token when a review phase's verdict is "revision"); the START event is
// always untyped. Keyed by phase ROLE (matches PHASES_A/PHASES_B role names;
// triage is keyed separately since it runs outside the phase loop).
const BUG_TYPE_TOKENS = {
  'triage':      { pass: 'bug-triaged',            fail: 'bug-triaged' },
  'plan':        { pass: 'fix-planned',            fail: 'fix-planned' },
  'review-plan': { pass: 'fix-review-passed',      fail: 'fix-review-failed' },
  'implement':   { pass: 'fix-implemented',        fail: 'fix-implemented' },
  'review-code': { pass: 'fix-code-review-passed', fail: 'fix-code-review-failed' },
  'approve':     { pass: 'fix-approved',           fail: 'fix-revision-requested' },
  'commit':      { pass: 'bug-committed',          fail: 'bug-commit-failed' },
}

// Per-phase model tier — verbatim port of fix_bug.md § Role-to-Tier Mapping.
// Passed as the `model` opt to agent(); the host resolves the tier name to a model.
const ROLE_TIER = {
  'triage':      'sonnet',
  'plan':        'sonnet',
  'review-plan': 'opus',
  'implement':   'sonnet',
  'review-code': 'opus',
  'approve':     'opus',
  'commit':      'haiku',
  'finalize':    'haiku',
}

const tierFor = (role) => ROLE_TIER[role] || 'sonnet'

// Path A phase list (short): implement → review-code → approve → commit → finalize
// Triage is handled separately before this array — do NOT include it here.
const PHASES_A = [
  { role: 'implement',   command: 'implement',   workflow: 'implement_plan.md',    maxIterations: 1 },
  { role: 'review-code', command: 'review-code',  workflow: 'review_code.md',       maxIterations: 3 },
  { role: 'approve',     command: 'approve',      workflow: 'architect_approve.md', maxIterations: 1 },
  { role: 'commit',      command: 'commit',        workflow: 'commit_task.md',        maxIterations: 1 },
  // finalize is NOT a subagent phase — handled inline after the loop
]

// Path B phase list (full): plan-fix → review-plan → implement → review-code → approve → commit → finalize
// Triage is handled separately before this array — do NOT include it here.
const PHASES_B = [
  { role: 'plan',        command: 'plan-fix',     workflow: 'plan_task.md',         maxIterations: 1 },
  { role: 'review-plan', command: 'review-plan',  workflow: 'review_plan.md',       maxIterations: 3 },
  ...PHASES_A,
]

// Schema for a single resolved bug context (pre-triage read).
const BUG_RESOLVE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['bugId', 'bugStatus'],
  properties: {
    bugId:      { type: 'string' },
    bugStatus:  { type: 'string' },
    note:       { type: 'string' },
  },
}

// Schema for a single phase subagent return value.
const BUG_PHASE_RESULT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['phase', 'role', 'gatePassed', 'verdict', 'escalated', 'taskStatus'],
  properties: {
    phase:      { type: 'string' },
    role:       { type: 'string' },
    gatePassed: { type: 'boolean' },
    verdict:    { type: 'string', enum: ['approved', 'revision', 'malformed', 'none'] },
    escalated:  { type: 'boolean' },
    taskStatus: { type: 'string' },   // bug.status read back after the phase
    note:       { type: 'string' },
  },
}

// Schema for the triage phase result.
const TRIAGE_RESULT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['phase', 'role', 'gatePassed', 'verdict', 'escalated', 'taskStatus'],
  properties: {
    phase:      { type: 'string' },
    role:       { type: 'string' },
    gatePassed: { type: 'boolean' },
    verdict:    { type: 'string', enum: ['approved', 'revision', 'malformed', 'none'] },
    escalated:  { type: 'boolean' },
    taskStatus: { type: 'string' },
    note:       { type: 'string' },
  },
}

// Port of fix_bug.md: a "Revision Required" verdict routes back to the nearest
// earlier phase whose role is NOT a review role (i.e. the producer).
function revisionTarget(phases, reviewIdx) {
  for (let j = reviewIdx - 1; j >= 0; j--) {
    if (!REVIEW_ROLES.includes(phases[j].role)) return j
  }
  return 0   // degenerate pipeline with no producer before the review — loop to start
}

// runBugPhase — mirrors runPhase() from wfl-run-task.js but adapted for bug semantics:
//   - --bug {bugId} instead of --task {taskId}
//   - sprintId = 'bugs' (virtual sprint dir)
//   - store/bugs/{bugId}.json instead of store/tasks/{taskId}.json
//   - read-verdict --bug {bugId}
// The subagent owns ALL shell-dependent side-effects for this phase.
function runBugPhase(bugId, phase, iter) {
  const sprintId = 'bugs'   // virtual sprint dir for all emit/sidecar/progress
  // forge-engineering#39: explicit type-token guidance per
  // .forge/workflows/_fragments/event-vocabulary.md § Bug pipeline. Without
  // it, subagents guessed and leaked the action value into `type`
  // ("start"/"complete" — schema-rejected store residue in events/bugs/).
  const typeTokens = BUG_TYPE_TOKENS[phase.role]
  const typeTokenLine = typeTokens
    ? (REVIEW_ROLES.includes(phase.role)
        ? 'set type="' + typeTokens.pass + '" when your verdict is Approved, type="' + typeTokens.fail + '" when it is Revision Required.'
        : 'set type="' + typeTokens.pass + '".')
    : 'omit the "type" field entirely (untyped events are valid; this role has no table entry).'
  return agent(
    [
      `You are running a SINGLE pipeline phase for Forge bug ${bugId} (sprint bugs).`,
      `Phase: role="${phase.role}", command="${phase.command}", workflow="${phase.workflow}", iteration=${iter}.`,
      '1. PRE-FLIGHT GATE. Call the MCP tool mcp__forge__preflight { "phase": "' + phase.role + '", "bug": "' + bugId + '" }.',
      '   If the result indicates failure: do NOT run the phase. Set status via',
      '   mcp__forge__store { "command": "update-status", "args": ["bug", "' + bugId + '", "status", "escalated"] }, emit an escalation event,',
      '   and return gatePassed=false, escalated=true, verdict="none", with the gate failure detail in note.',
      '2. RUN THE PHASE. Read `.forge/workflows/' + phase.workflow + '` and follow it for bug ' + bugId + '.',
      '   The workflow writes its own artifacts, {PHASE}-SUMMARY.json, and any bug-status changes.',
      '   Also read the task-scoped slice of `engineering/MASTER_INDEX.md` for project context.',
      '3. EMIT YOUR PHASE EVENTS. You are the only actor that knows your runtime attribution.',
      '   3a. BEFORE running the phase workflow: note the start timestamp (startTimestamp = new Date().toISOString()).',
      '   Emit a start event via mcp__forge__store { "command": "emit", "args": ["bugs", "<event-json-string>"] }',
      '   with action="start", role="' + phase.role + '", iteration=' + iter + ', startTimestamp and endTimestamp both equal to startTimestamp (0-duration placeholder).',
      '   The start event MUST NOT include a "type" field.',
      '   3b. AFTER the phase workflow completes: note the end timestamp (endTimestamp = new Date().toISOString()).',
      '   Compute durationMinutes = (new Date(endTimestamp) - new Date(startTimestamp)) / 60000.',
      '   Emit a complete event via mcp__forge__store { "command": "emit", "args": ["bugs", "<event-json-string>"] }',
      '   conforming to `.forge/schemas/event.schema.json` (role, action="complete", phase, iteration=' + iter + ', bugId,',
      '   startTimestamp, endTimestamp, durationMinutes, plus your own model/provider/token usage — do NOT invent placeholder model strings).',
      '   COMPLETE-event type (per .forge/workflows/_fragments/event-vocabulary.md): ' + typeTokenLine,
      '   NEVER copy the action value ("start"/"complete") into "type" — those tokens are schema-rejected and the event would be dropped.',
      '   Then drain any `.forge/cache/FRICTION-*.jsonl` friction records you produced and emit them as type "friction".',
      phase.role && REVIEW_ROLES.includes(phase.role)
        ? '4. READ VERDICT. This is a REVIEW phase. The phase workflow records its verdict into the store '
          + 'summary (`summaries.' + phase.role + '.verdict`) via set-bug-summary — make sure that write happened. '
          + 'Then resolve it by calling mcp__forge__store { "command": "read", "args": ["bug", "' + bugId + '", "--json"] } '
          + 'and reading `summaries.' + phase.role + '.verdict` directly from the returned record '
          + '(the structured summary, NOT a markdown artifact path). '
          + 'Map the verdict value → result verdict: "approved"→"approved", "revision"→"revision", '
          + 'missing / "n/a" / any other value→"malformed". NEVER guess.'
        : '4. NON-REVIEW phase: return verdict="none".',
      '5. Read `.forge/store/bugs/' + bugId + '.json` and return its final status as taskStatus, plus a one-line note.',
    ].join('\n'),
    { label: `${bugId}:${phase.role}:${iter}`, phase: 'Pipeline', schema: BUG_PHASE_RESULT_SCHEMA, model: tierFor(phase.role) }
  )
}

// escalateBug — analogous to escalateTask() from wfl-run-task.js.
// The JS driver cannot write the store, so a tiny haiku agent performs the status
// write and event emit. Uses --bug flag and 'bugs' virtual sprint dir throughout.
function escalateBug(bugId, reason) {
  return agent(
    [
      `Escalate Forge bug ${bugId} to a human.`,
      `Call mcp__forge__store { "command": "update-status", "args": ["bug", "${bugId}", "status", "escalated"] }`,
      `and emit one event (sprint bugs) with verdict="escalated" and notes="${reason}".`,
      `Return the bug's final status as taskStatus, gatePassed=true, verdict="none", escalated=true, phase="escalate", role="escalate".`,
    ].join(' '),
    { label: `${bugId}:escalate`, phase: 'Pipeline', schema: BUG_PHASE_RESULT_SCHEMA, model: 'haiku' }
  )
}

// --- Main body ---

const bugId = (typeof args === 'string' ? args : args?.bugId)
if (!bugId) throw new Error('wfl:fix-bug requires a bug id — pass args: "FORGE-BUG-001"')

// Step 0 — Read the bug record to check pre-run status guard.
phase('Resolve')
const resolved = await agent(
  [
    `Read the bug record for ${bugId}..`,
    `Call mcp__forge__store { "command": "read", "args": ["bug", "${bugId}", "--json"] } and return bugId and bugStatus.`,
    `Read-only — do NOT modify anything.`,
  ].join(' '),
  { label: `resolve:${bugId}`, phase: 'Resolve', schema: BUG_RESOLVE_SCHEMA }
)

if (!resolved) throw new Error(`Could not resolve bug record for ${bugId}`)

// Pre-loop status guard — fix_bug.md: skip at pre-loop with one bug-skipped event.
// Token + payload per _fragments/event-vocabulary.md (forge-engineering#39):
// bug-skipped requires bugId but no phase/iteration; driver-emitted skips carry
// model/provider "n/a"; the skip reason rides in "notes" (declared property).
if (SKIP_STATUS.includes(resolved.bugStatus)) {
  log(`⚠ ${bugId} — status is ${resolved.bugStatus}, skipping run.`)
  // Emit bug-skipped event (not a silent return — Iron Law 3).
  await agent(
    [
      `Emit a bug-skipped event for ${bugId}.`,
      `Call mcp__forge__store { "command": "emit", "args": ["bugs", "<event-json-string>"] } where <event-json-string> is the JSON string \`{"eventId":"<uuid-v4>","type":"bug-skipped","sprintId":"bugs","bugId":"${bugId}","role":"orchestrator","action":"skipped","notes":"status=${resolved.bugStatus}","startTimestamp":"<now-iso>","endTimestamp":"<now-iso>","durationMinutes":0,"model":"n/a","provider":"n/a"}\`.`,
      'Replace <uuid-v4> with a UUID v4 string (e.g. crypto.randomUUID()) and both <now-iso> with the current UTC timestamp (new Date().toISOString()). Do NOT modify any other store records.',
      `Best-effort. Return taskStatus="${resolved.bugStatus}", gatePassed=true, verdict="none", escalated=false, phase="skip", role="orchestrator".`,
    ].join(' '),
    { label: `${bugId}:skip`, phase: 'Skip', schema: BUG_PHASE_RESULT_SCHEMA, model: 'haiku' }
  )
  return { bugId, skipped: true, bugStatus: resolved.bugStatus, results: [] }
}

log(`Bug ${bugId} — starting triage`)

// Step 1 — Triage subagent.
// Triage is handled outside the phase loop: it writes summaries.triage.route and
// determines Path A vs Path B. The orchestrator (not the subagent) owns the two
// status updates: triaged then in-progress (Iron Law 2 of fix_bug.md).
phase('Triage')
const triagePhase = { role: 'triage', command: 'triage', workflow: 'triage.md', maxIterations: 1 }
let triageResult = await agent(
  [
    `You are running the TRIAGE phase for Forge bug ${bugId} (sprint bugs).`,
    `Phase: role="triage", command="triage", workflow="triage.md", iteration=1.`,
    '1. PRE-FLIGHT GATE. Call the MCP tool mcp__forge__preflight { "phase": "triage", "bug": "' + bugId + '" }.',
    '   If the result indicates failure: do NOT run the phase. Return gatePassed=false, escalated=true, verdict="none".',
    '2. RUN THE PHASE. Read `.forge/workflows/triage.md` and follow it for bug ' + bugId + '.',
    '   The triage workflow writes TRIAGE.md + TRIAGE-SUMMARY.json with a route field ("A" or "B").',
    '   Call mcp__forge__store { "command": "set-bug-summary", "args": ["' + bugId + '", "triage", "<summary-json-string>"] } with the summary.',
    '   CRITICAL: write summaries.triage.route (field name is "route" NOT "path").',
    '   Do NOT write bug.status in this phase — the orchestrator owns the status writes.',
    '3. EMIT YOUR PHASE EVENTS. You are the only actor that knows your runtime attribution.',
    '   3a. BEFORE running the triage workflow: note the start timestamp (startTimestamp = new Date().toISOString()).',
    '   Emit a start event via mcp__forge__store { "command": "emit", "args": ["bugs", "<event-json-string>"] }',
    '   with action="start", role="triage", iteration=1, startTimestamp and endTimestamp both equal to startTimestamp (0-duration placeholder).',
    '   The start event MUST NOT include a "type" field.',
    '   3b. AFTER the triage workflow completes: note the end timestamp (endTimestamp = new Date().toISOString()).',
    '   Compute durationMinutes = (new Date(endTimestamp) - new Date(startTimestamp)) / 60000.',
    '   Emit a complete event via mcp__forge__store { "command": "emit", "args": ["bugs", "<event-json-string>"] }',
    '   conforming to `.forge/schemas/event.schema.json` (role, action="complete", phase, iteration=1, bugId,',
    '   startTimestamp, endTimestamp, durationMinutes, plus your own model/provider/token usage — do NOT invent placeholder model strings).',
    '   COMPLETE-event type (per .forge/workflows/_fragments/event-vocabulary.md): set type="bug-triaged".',
    '   NEVER copy the action value ("start"/"complete") into "type" — those tokens are schema-rejected and the event would be dropped.',
    '4. NON-REVIEW phase: return verdict="none".',
    '5. Read `.forge/store/bugs/' + bugId + '.json` and return its final status as taskStatus, plus a one-line note.',
  ].join('\n'),
  { label: `${bugId}:triage:1`, phase: 'Triage', schema: TRIAGE_RESULT_SCHEMA, model: tierFor('triage') }
)

// Null-dispatch retry: if triage returned null, retry once then escalate.
if (!triageResult) {
  triageResult = await agent(
    [
      `You are running the TRIAGE phase for Forge bug ${bugId} (sprint bugs). This is a retry.`,
      `Phase: role="triage", command="triage", workflow="triage.md", iteration=1.`,
      '1. PRE-FLIGHT GATE. Call the MCP tool mcp__forge__preflight { "phase": "triage", "bug": "' + bugId + '" }.',
      '   If the result indicates failure: return gatePassed=false, escalated=true, verdict="none".',
      '2. RUN THE PHASE. Read `.forge/workflows/triage.md` and follow it for bug ' + bugId + '.',
      '   Write summaries.triage.route (field name is "route" NOT "path").',
      '   Do NOT write bug.status — the orchestrator owns the status writes.',
      '3. Emit start and complete events (action="start" then action="complete") to sprint bugs.',
      '4. Return verdict="none", taskStatus from bug record.',
    ].join('\n'),
    { label: `${bugId}:triage:1:retry`, phase: 'Triage', schema: TRIAGE_RESULT_SCHEMA, model: tierFor('triage') }
  )
}

if (!triageResult) {
  await escalateBug(bugId, 'triage dispatch returned null after retry')
  return { bugId, escalated: true, bugStatus: 'escalated', results: [] }
}

if (!triageResult.gatePassed || triageResult.escalated) {
  await escalateBug(bugId, triageResult.note || 'triage gate failed / self-escalated')
  return { bugId, escalated: true, bugStatus: 'escalated', results: [triageResult] }
}

// Orchestrator owns both status writes after triage returns (Iron Law 2).
// Two separate calls: reported→triaged, then triaged→in-progress.
await agent(
  [
    `Call mcp__forge__store { "command": "update-status", "args": ["bug", "${bugId}", "status", "triaged"] }.`,
    `Then call mcp__forge__store { "command": "update-status", "args": ["bug", "${bugId}", "status", "in-progress"] }.`,
    `Return taskStatus="in-progress", gatePassed=true, verdict="none", escalated=false, phase="status-write", role="orchestrator".`,
  ].join(' '),
  { label: `${bugId}:status-write`, phase: 'StatusWrite', schema: BUG_PHASE_RESULT_SCHEMA, model: 'haiku' }
)

// Read summaries.triage.route to select Path A or Path B.
const routeResult = await agent(
  [
    `Read the bug record for ${bugId}..`,
    `Call mcp__forge__store { "command": "read", "args": ["bug", "${bugId}", "--json"] }.`,
    `Return the value of summaries.triage.route (field name is "route" NOT "path").`,
    `Return as: { bugId: "${bugId}", bugStatus: "<current status>", note: "<route value A or B>" }.`,
  ].join(' '),
  { label: `${bugId}:read-route`, phase: 'ReadRoute', schema: BUG_RESOLVE_SCHEMA }
)

const route = routeResult?.note?.trim()
if (route !== 'A' && route !== 'B') {
  log(`⚠ ${bugId}  triage — route="${route}" is neither A nor B, escalating verdict_malformed`)
  await escalateBug(bugId, `verdict_malformed: summaries.triage.route="${route}" — expected "A" or "B"`)
  return { bugId, escalated: true, bugStatus: 'escalated', results: [triageResult] }
}

// Path selection — decided once, no switching (Iron Law 1 of fix_bug.md).
const phases = route === 'A' ? PHASES_A : PHASES_B
log(`Bug ${bugId} — route=${route}, pipeline: ${phases.map(p => p.role).join(' → ')}`)

// Step 2 — Drive the phase FSM. JS owns sequencing, counters, routing, escalation.
const iterationCounts = {}    // keyed by phase command
const results = [triageResult]
let i = 0
let escalated = false
let escalationReason = null

while (i < phases.length) {
  const p = phases[i]
  const iter = (iterationCounts[p.command] || 0) + 1
  log(`→ ${bugId}  ${p.role} [${tierFor(p.role)}]  (iter ${iter})`)

  // Dispatch with one retry on a null/skipped dispatch (escalate-don't-halt at phase grain).
  let r = await runBugPhase(bugId, p, iter)
  if (!r) r = await runBugPhase(bugId, p, iter)
  if (!r) {
    escalationReason = `phase ${p.role} dispatch returned null after retry`
    log(`✗ ${bugId}  ${p.role}  — dispatch failed twice, escalating`)
    escalated = true
    break
  }

  results.push(r)

  // Gate failure or subagent self-escalation (already wrote status=escalated).
  if (!r.gatePassed || r.escalated) {
    escalationReason = r.note || `${p.role} gate failed / self-escalated`
    log(`⚠ ${bugId}  ${p.role}  — escalated (${escalationReason})`)
    escalated = true
    break
  }

  // Review phases route on verdict; non-review phases advance on completion.
  if (REVIEW_ROLES.includes(p.role)) {
    if (r.verdict === 'approved') {
      log(`✓ ${bugId}  ${p.role}  — Approved`)
      i += 1
    } else if (r.verdict === 'revision') {
      iterationCounts[p.command] = (iterationCounts[p.command] || 0) + 1
      log(`↻ ${bugId}  ${p.role}  — Revision Required (iter ${iterationCounts[p.command]})`)
      if (iterationCounts[p.command] >= p.maxIterations) {
        escalationReason = `max iterations (${p.maxIterations}) reached at ${p.role}`
        escalated = true
        break
      }
      i = revisionTarget(phases, i)   // loop back to the producing phase
    } else {
      // 'malformed' (or unexpected 'none' from a review phase) — never guess.
      escalationReason = `verdict malformed at ${p.role}`
      escalated = true
      break
    }
  } else {
    log(`✓ ${bugId}  ${p.role}  — completed`)
    i += 1
  }
}

// If the JS driver decided to escalate (not the subagent), perform the status write.
const lastWroteEscalation = results.length && results[results.length - 1].escalated
if (escalated && !lastWroteEscalation) {
  await escalateBug(bugId, escalationReason)
}

if (escalated) {
  log(`⚠ Bug ${bugId} escalated: ${escalationReason}`)
  log(`   Resume with the failing phase command after addressing the issue, or re-run wfl:fix-bug.`)
  return {
    bugId,
    escalated: true,
    bugStatus: 'escalated',
    phasesRun: results.length,
    results,
    escalationReason,
  }
}

// Step 3 — Finalize (inline, not a subagent phase per fix_bug.md § Finalize).
// Run mcp__forge__collate (purgeEvents) then mcp__forge__preflight (phase=finalize, bug={bugId}).
// Gate failure escalates but PRESERVES bug.status=fixed (already set by commit phase).
phase('Finalize')
log(`→ ${bugId}  finalize [${tierFor('finalize')}]`)

const finalizeResult = await agent(
  [
    `Finalize Forge bug ${bugId}..`,
    `Step 1 (token accounting — MUST run BEFORE collate purges events): Run`,
    `   \`node .forge/tools/forge-usage-report.cjs --sprint bugs --apply\`.`,
    `   This deterministically reconciles per-phase token usage from THIS run's Workflow transcript`,
    `   onto the bug's COMPLETE events, so collate's cost section is accurate. It is a Bash tool`,
    `   (it reads the Workflow harness transcript — there is no MCP equivalent; documented boundary).`,
    `   BEST-EFFORT: if it prints "no workflow transcript dir" or errors, log and continue — do NOT`,
    `   escalate on this step. It MUST run before Step 2 because collate --purge-events deletes the events.`,
    `Step 2: Call mcp__forge__collate { "sprintId": "${bugId}", "purgeEvents": true }.`,
    `   This purges this bug's events from the shared bugs/ dir and embeds the cost section in INDEX.md.`,
    `   Do NOT run a separate cost aggregation — collate handles it automatically.`,
    `Step 3: Call mcp__forge__preflight { "phase": "finalize", "bug": "${bugId}" }.`,
    `   If the gate result indicates failure: emit one escalation event to sprint bugs with verdict="escalated"`,
    `   and notes="finalize gate failed". Do NOT call update-status bug — bug.status is already "fixed".`,
    `   Return escalated=true in that case.`,
    `Step 4: If collate and the gate succeeded, return gatePassed=true, escalated=false, verdict="none",`,
    `   taskStatus="fixed", phase="finalize", role="finalize".`,
    `   If the gate failed, return gatePassed=false, escalated=true, verdict="none", taskStatus="fixed".`,
  ].join('\n'),
  { label: `${bugId}:finalize:1`, phase: 'Finalize', schema: BUG_PHASE_RESULT_SCHEMA, model: tierFor('finalize') }
)

results.push(finalizeResult)

// Null-dispatch guard: agent() returns null if the finalize subagent is skipped
// or errors. Without this, `finalizeResult?.escalated` short-circuits to
// undefined and the driver would fall through and report bugStatus:'fixed',
// escalated:false — declaring the run clean when finalize (collate + gate)
// never executed. Every other dispatch in this driver has a null guard; this
// is the one that was missing. Preserve bugStatus:'fixed' (commit already wrote
// it) but mark the run escalated so it is not reported green.
if (!finalizeResult) {
  await escalateBug(bugId, 'finalize dispatch returned null (subagent skipped or errored)')
  return {
    bugId,
    escalated: true,
    bugStatus: 'fixed',
    phasesRun: results.length,
    results,
    escalationReason: 'finalize dispatch returned null',
  }
}

if (finalizeResult?.escalated) {
  log(`⚠ ${bugId}  finalize — gate failed; bug.status preserved as "fixed"`)
  log(`   Finalize escalation raised but bug commit is complete.`)
  return {
    bugId,
    escalated: true,
    bugStatus: 'fixed',   // commit already wrote fixed; finalize gate failure does not revert
    phasesRun: results.length,
    results,
    escalationReason: 'finalize gate failed',
  }
}

log(`✓ ${bugId}  finalize — complete`)

// Step 4 — Report terminal outcome.
phase('Report')
log(`🍂 Bug ${bugId} fixed — pipeline reached terminal (fixed).`)

return {
  bugId,
  escalated: false,
  bugStatus: 'fixed',
  phasesRun: results.length,
  results,
}
