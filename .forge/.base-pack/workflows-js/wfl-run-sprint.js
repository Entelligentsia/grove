export const meta = {
  name: 'wfl:run-sprint',
  description: 'Code-orchestrated port of /forge:run-sprint — load sprint, topo-sort into dependency waves, drive each task through the wfl:run-task sub-workflow, escalate-don\'t-halt, collate + report.',
  whenToUse: 'Run all tasks in a Forge sprint via a deterministic JS driver instead of the LLM orchestrator. Pass the sprint id as args, e.g. args: "FORGE-S27".',
  phases: [
    { title: 'Load',    detail: 'one agent reads the sprint + its tasks via store-cli, returns the task graph' },
    { title: 'Execute', detail: 'per dependency wave: invoke the wfl:run-task sub-workflow per task (parallel within a wave); escalate-don\'t-halt' },
    { title: 'Collate',  detail: 'run collate.cjs, summarise committed / escalated / carried-over' },
  ],
}

// ---------------------------------------------------------------------------
// wfl:run-sprint — a code-orchestrated port of .forge/workflows/run_sprint.md
//
// Why a script: run_sprint.md is a deterministic FSM (Kahn's-algorithm wave
// sort + per-task dispatch + escalate-don't-halt + collate). In the LLM
// orchestrator that loop is hand-run turn-by-turn. Here the JS holds the loop,
// branching, and intermediate results; agents only do I/O and run pipelines.
//
// Runtime constraint (per the Workflow tool): the SCRIPT has no filesystem or
// shell access — agents run all commands. So sprint/task loading and collation
// are agent() calls; wave computation + routing are JS.
//
// COMPOSITION: each task is driven by the wfl:run-task SUB-WORKFLOW (this file's
// sibling, .claude/workflows/wfl-run-task.js), invoked inline via the workflow()
// hook. That child ports orchestrate_task.md's phase FSM — it owns the per-phase
// loop, revision counters, verdict routing, and per-task escalation. This driver
// owns only the OUTER wave-sort FSM and escalate-don't-halt at the sprint grain.
// Nesting is one level: this sprint workflow -> wfl:run-task. wfl:run-task itself
// calls no further workflow(), so the one-level limit holds.
//
// Behaviour parity with run_sprint.md:
//   Step 1 Load        -> loadSprint()           (skip terminal; honour executionMode)
//   Step 2 Sort        -> computeWaves()          (Kahn's algorithm, cycle = halt)
//   Step 3 Execute     -> per-wave dispatch       (sequential | wave-parallel | full-parallel)
//                         via wfl:run-task + escalate-don't-halt
//   Step 4 Post-Sprint -> collate + report
//
// Invocation (Workflow tool):  { name: 'wfl:run-sprint', args: 'FORGE-S27' }
// args may also be an object: { sprintId: 'FORGE-S27', mode: 'sequential' }
// (mode override is optional; defaults to the sprint record's executionMode).
// ---------------------------------------------------------------------------

const TERMINAL = ['committed', 'abandoned']           // skip on load (already done)
const TERMINAL_OK = ['committed', 'abandoned', 'escalated']  // acceptable end-of-dispatch states
const TERMINAL_OK_SET = new Set(TERMINAL_OK)           // O(1) membership test for resume guard (#17)

// Phase banner map — visual phase identity for log lines (LOW #22).
// Maps the sprint orchestrator's phases to the persona label used in log output.
const BANNER_MAP = {
  'load':    'forge-orchestrator',
  'execute': 'forge-orchestrator',
  'collate': 'forge-collator',
}

const LOAD_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['sprintId', 'executionMode', 'tasks'],
  properties: {
    sprintId: { type: 'string' },
    executionMode: { type: 'string', enum: ['sequential', 'wave-parallel', 'full-parallel'] },
    tasks: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['taskId', 'status', 'dependencies'],
        properties: {
          taskId: { type: 'string' },
          status: { type: 'string' },
          dependencies: { type: 'array', items: { type: 'string' } },
        },
      },
    },
  },
}

const COLLATE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['committed', 'escalated', 'carriedOver'],
  properties: {
    committed: { type: 'integer' },
    escalated: { type: 'integer' },
    carriedOver: { type: 'integer' },
    summary: { type: 'string' },
  },
}

// --- Step 2: Kahn's-algorithm wave computation (port of run_sprint.md) -------
function computeWaves(tasks) {
  const graph = {}, inDegree = {}, ids = new Set(tasks.map(t => t.taskId))
  for (const t of tasks) { graph[t.taskId] = new Set(); inDegree[t.taskId] = 0 }
  for (const t of tasks) {
    for (const dep of t.dependencies || []) {
      if (ids.has(dep)) { graph[dep].add(t.taskId); inDegree[t.taskId] += 1 }
    }
  }
  const waves = []
  let queue = Object.keys(inDegree).filter(id => inDegree[id] === 0).sort()
  while (queue.length) {
    const wave = [...queue].sort()        // deterministic ordering within a wave
    waves.push(wave)
    const next = []
    for (const id of wave) {
      for (const succ of graph[id]) {
        inDegree[succ] -= 1
        if (inDegree[succ] === 0) next.push(succ)
      }
    }
    queue = next
  }
  const remaining = Object.keys(inDegree).filter(id => inDegree[id] > 0)
  if (remaining.length) throw new Error(`Dependency cycle detected among: ${remaining.join(', ')}`)
  return waves
}

// --- Step 3 helper: dispatch one task through its full pipeline -------------
// Delegates to the wfl:run-task SUB-WORKFLOW (.claude/workflows/wfl-run-task.js),
// which owns the per-phase FSM, revision loops, verdict routing, and per-task
// escalation. We normalise its return into the {taskId, status, terminal, note}
// shape the collation step expects, and apply escalate-don't-halt at the sprint
// grain: a task that escalates is logged and the sprint continues.
async function dispatchTask(sprintId, taskId, mode) {
  // Gap #4 (AC3): emit task-dispatch event before invoking wfl:run-task.
  await agent(
    [
      `Emit a task-dispatch event for task ${taskId} in sprint ${sprintId}.`,
      `Call mcp__forge__store { "command":"emit", "args":["${sprintId}",`,
      `'{"eventId":"<uuid-v4>","type":"task-dispatch","taskId":"${taskId}","sprintId":"${sprintId}",`,
      `"role":"orchestrator","action":"task-dispatch","phase":"dispatch","iteration":1,`,
      `"startTimestamp":"<ISO-now>","endTimestamp":"<ISO-now>","durationMinutes":0,`,
      `"model":"<your-model-id>","provider":"anthropic"}'] }`,
      'Replace <uuid-v4> with a UUID v4 string (e.g. crypto.randomUUID()), <ISO-now> with the',
      'current UTC ISO 8601 timestamp, and <your-model-id> with the actual model you are using.',
      'Do NOT modify any other store records.',
    ].join(' '),
    { label: `task-dispatch:${taskId}`, phase: 'Execute' }
  )

  // Gap #3 (AC1): worktree isolation — documented-latent stub.
  // Sequential mode: skip worktree ops (no parallel file-system contention).
  // wave-parallel / full-parallel: bracket dispatch with per-task worktree lifecycle.
  // DISPOSITION: implemented as documented-latent for FORGE-S28 (sequential mode only).
  // When parallel modes are exercised in a future sprint, remove the sequential guard
  // and promote these agent calls to full implementation with conflict-escalation logic.
  let worktreeActive = false
  if (mode !== 'sequential') {
    // Pre-dispatch: create an isolated worktree for this task.
    await agent(
      [
        `Create a git worktree for task ${taskId} to isolate parallel pipeline I/O.`,
        `git worktree add ../worktrees/${taskId} HEAD`,
        'Assert exit 0; if the command fails, log the error and escalate (do NOT halt the sprint).',
        'Return { ok: true } on success or { ok: false, error: "<msg>" } on failure.',
      ].join(' '),
      { label: `worktree-add:${taskId}`, phase: 'Execute' }
    )
    worktreeActive = true
  }

  let child
  try {
    // workflow() runs the child inline (shared concurrency cap, agent counter,
    // budget). It throws on unknown name / child error — catch so one bad task
    // never halts the whole sprint.
    child = await workflow('wfl:run-task', { taskId })
  } catch (err) {
    log(`⚠ ${taskId} — wfl:run-task threw (${err?.message || err}) — escalating, continuing sprint.`)
    if (worktreeActive) {
      // Teardown worktree even on error (try/finally equivalent).
      await agent(
        `Remove the git worktree for task ${taskId} after pipeline error. Run: git worktree remove --force ../worktrees/${taskId}`,
        { label: `worktree-remove:${taskId}`, phase: 'Execute' }
      )
    }
    return { taskId, status: 'escalated', terminal: false, note: `wfl:run-task threw: ${err?.message || err}` }
  }

  if (worktreeActive) {
    // Post-dispatch: rebase and remove worktree.
    await agent(
      [
        `Merge results from task ${taskId} worktree back to main and remove the worktree.`,
        `Run: git -C ../worktrees/${taskId} rebase origin/main`,
        '(If rebase conflicts: log and escalate — do NOT halt.)',
        `Then run: git worktree remove --force ../worktrees/${taskId}`,
      ].join(' '),
      { label: `worktree-remove:${taskId}`, phase: 'Execute' }
    )
  }

  if (!child) {
    return { taskId, status: 'unknown', terminal: false, note: 'wfl:run-task returned null (skipped/errored)' }
  }

  // wfl:run-task returns either a skipped result ({skipped, taskStatus}) or a
  // terminal result ({finalStatus, escalated, escalationReason}).
  const status = child.skipped ? child.taskStatus
    : child.escalated ? 'escalated'
    : (child.finalStatus || 'unknown')
  // A skipped child is terminal-acceptable regardless of taskStatus: the child
  // deliberately chose not to run the task (e.g. it is `blocked` and waiting on
  // a dependency, or already `committed`/`abandoned`). Re-dispatching + then
  // escalating a legitimately blocked task is wrong — leave it as-is to carry
  // over. Only a genuinely non-terminal *run* (status `unknown`/stalled) retries.
  const terminal = child.skipped || TERMINAL_OK_SET.has(status)
  const note = child.escalationReason
    || (child.skipped ? `skipped (status ${child.taskStatus})` : `pipeline ${status} after ${child.phasesRun ?? '?'} phase(s)`)

  // Gap #17 — Re-spawn guard: 2 attempts before escalating.
  // Mirrors run_sprint.md §Step 3 lines 105–124: if the first dispatch returns
  // a non-terminal status (context overflow, mid-pipeline stall), re-dispatch
  // once before escalating. wfl:run-task takes only a task id and resumes from
  // the task's store state (completed phases are skipped by their pre-flight
  // gates), so this is a clean re-dispatch — there is no resumeFrom argument.
  if (!terminal) {
    log(`⚠ ${taskId} did not reach terminal (status: ${status}) — re-dispatching once before escalating.`)
    let child2
    try {
      child2 = await workflow('wfl:run-task', { taskId })
    } catch (err2) {
      log(`⚠ ${taskId} — retry threw (${err2?.message || err2}) — escalating after 2 attempts.`)
      return { taskId, status: 'escalated', terminal: false, note: `respawn exhausted (attempt 2 threw): ${err2?.message || err2}` }
    }
    if (!child2) {
      log(`⚠ ${taskId} — retry returned null — escalating after 2 attempts.`)
      return { taskId, status: 'escalated', terminal: false, note: 'respawn exhausted (attempt 2 returned null)' }
    }
    const status2 = child2.skipped ? child2.taskStatus
      : child2.escalated ? 'escalated'
      : (child2.finalStatus || 'unknown')
    const terminal2 = TERMINAL_OK_SET.has(status2)
    if (!terminal2) {
      log(`⚠ ${taskId} did not reach terminal after 2 attempts (status: ${status2}) — escalating.`)
      return { taskId, status: 'escalated', terminal: false, note: `respawn-exhausted after 2 attempts (final status: ${status2})` }
    }
    return {
      taskId,
      status: status2,
      terminal: terminal2,
      note: child2.escalationReason || `resumed from ${status}, completed as ${status2}`,
    }
  }

  return { taskId, status, terminal, note }
}

// --- Main -------------------------------------------------------------------
const sprintId = (typeof args === 'string' ? args : args?.sprintId)
if (!sprintId) throw new Error('wfl:run-sprint requires a sprint id — pass args: "FORGE-S27"')
const modeOverride = (typeof args === 'object' && args?.mode) || null

// Step 1 — Load sprint + tasks (agent does the store I/O; script has none).
phase('Load')
const loaded = await agent(
  [
    `Load Forge sprint ${sprintId}.`,
    `Call mcp__forge__store { "command":"read", "args":["sprint","${sprintId}","--json"] } and read every task in`,
    `.forge/store/tasks/ whose sprintId === ${sprintId}.`,
    'Return: sprintId, executionMode (the sprint record\'s mode; default "sequential" if absent),',
    'and tasks[] each with taskId, status, and dependencies[].',
    'Do NOT modify anything. Read-only.',
  ].join(' '),
  { label: `load:${sprintId}`, phase: 'Load', schema: LOAD_SCHEMA }
)
if (!loaded) throw new Error(`Could not load sprint ${sprintId}`)

const mode = modeOverride || loaded.executionMode || 'sequential'
// Skip already-terminal tasks (committed/abandoned) — run_sprint.md Step 1.
const active = loaded.tasks.filter(t => !TERMINAL.includes(t.status))
log(`Sprint ${sprintId}: ${loaded.tasks.length} task(s), ${active.length} to run, mode=${mode}`)
if (!active.length) {
  log('No non-terminal tasks — nothing to run.')
  return { sprintId, mode, dispatched: 0, results: [] }
}

// Step 2 — Sort into dependency waves.
const allWaves = computeWaves(active)
// full-parallel collapses to one wave; sequential expands to one task per step.
let waves
if (mode === 'full-parallel') waves = [active.map(t => t.taskId)]
else if (mode === 'sequential') waves = allWaves.flat().map(id => [id])
else waves = allWaves   // wave-parallel
log(`Dependency plan: ${waves.length} step(s) — ${waves.map(w => `[${w.join(',')}]`).join(' → ')}`)

// Gap #4 (AC2): emit sprint-start event before the wave loop begins.
await agent(
  [
    `Emit a sprint-start event for sprint ${sprintId}.`,
    'Call mcp__forge__store { "command":"emit", "args":[',
    `"${sprintId}",`,
    `'{"eventId":"<uuid-v4>","type":"sprint-start","sprintId":"${sprintId}",`,
    `"role":"orchestrator","action":"sprint-start",`,
    `"startTimestamp":"<ISO-now>","endTimestamp":"<ISO-now>","durationMinutes":0,`,
    `"model":"<your-model-id>","provider":"anthropic","taskCount":${active.length}}'] }`,
    'Replace <uuid-v4> with a UUID v4 string, <ISO-now> with the current UTC ISO 8601 timestamp,',
    'and <your-model-id> with the actual model you are using.',
    'Do NOT modify any other store records.',
  ].join(' '),
  { label: `sprint-start:${sprintId}`, phase: 'Execute' }
)

// LOW #23: transition sprint status → active before the wave loop begins.
// Mirrors run_sprint.md Step 1: the prose orchestrator calls
// `store-cli update-status sprint <id> active` right after loading the sprint
// and before processing any wave. Without this the sprint stays in `planned`
// status during execution, which misrepresents state in /forge:status output.
await agent(
  [
    `Transition sprint ${sprintId} to active status.`,
    `Call mcp__forge__store { "command":"update-status", "args":["sprint","${sprintId}","active"] }`,
    'If the sprint is already active or completed, the command is a no-op — that is fine.',
    'Return "ok".',
  ].join(' '),
  { label: `sprint-active:${sprintId}`, phase: 'Execute' }
)

// Step 3 — Execute. Parallel within a wave; escalate-don't-halt.
phase('Execute')

// Gap #15 — Clear the sprint progress log before dispatching any task.
// Mirrors run_sprint.md §Step 3 "Clear Progress Log at Sprint Start" (lines 73–82).
// store-cli exits 0 for a missing progress log — idempotent, fire-and-continue.
await agent(
  [
    `Clear the sprint progress log for ${sprintId} before dispatching any task.`,
    `Call mcp__forge__store { "command":"progress-clear", "args":["${sprintId}"] }`,
    'Exit 0 for a missing log is expected and fine. Do NOT modify any other store records.',
  ].join(' '),
  { label: `progress-clear:${sprintId}`, phase: 'Execute' }
)

const results = []
for (let i = 0; i < waves.length; i++) {
  const wave = waves[i]
  log(`▶ wave ${i + 1}/${waves.length}: ${wave.join(', ')}`)
  const waveResults = await parallel(wave.map(taskId => () => dispatchTask(sprintId, taskId, mode)))
  results.push(...waveResults.filter(Boolean))
}

// Gap #18-sprint — Sprint-side friction emission after wave loop, before collation.
// Mirrors run_sprint.md §Iron Laws: "Every failure must produce a visible signal and a
// structured event." Drains FRICTION-*.jsonl written by sub-agents and emits a
// type:friction event for any task that never reached terminal after 2 attempts.
// Fire-and-continue: skip silently if no escalations and no FRICTION-*.jsonl files.
await agent(
  [
    `Sprint ${sprintId} wave loop complete. Drain any queued friction records and emit sprint-level friction events.`,
    `Escalated task outcomes: ${JSON.stringify(results.filter(r => !r.terminal || r.status === 'escalated').map(r => ({ id: r.taskId, status: r.status, note: r.note })))}.`,
    '',
    'Step 1 — For each escalated/non-terminal task listed above, emit a type:friction event:',
    `  Call mcp__forge__store { "command":"emit", "args":["${sprintId}", '{"eventId":"<uuid-v4>","type":"friction","sprintId":"${sprintId}","workflow":"wfl:run-sprint","persona":"orchestrator","issue":"respawn-exhausted","taskId":"<task-id>","startTimestamp":"<ISO-now>","endTimestamp":"<ISO-now>","durationMinutes":0,"model":"<your-model-id>","provider":"anthropic"}'] }`,
    '  Replace <uuid-v4>, <task-id>, <ISO-now>, <your-model-id> with actual values.',
    '',
    'Step 2 — Drain any .forge/cache/FRICTION-*.jsonl files:',
    '  For each line in each FRICTION-*.jsonl file, emit the record as a type:friction event via mcp__forge__store.',
    '  After emitting all records from a file, delete the file.',
    '',
    'If no escalations occurred AND no FRICTION-*.jsonl files exist, do nothing (skip silently).',
    'Do NOT modify task or sprint store records.',
  ].join(' '),
  { label: `friction-drain:${sprintId}`, phase: 'Execute' }
)

// Step 4 — Post-sprint collation + report (agent does the I/O + status writes).
phase('Collate')
const committed = results.filter(r => r.status === 'committed').length
const escalated = results.filter(r => r.status === 'escalated' || !r.terminal).length
const carriedOver = results.filter(r => r.status === 'abandoned').length
const committedIds = results.filter(r => r.status === 'committed').map(r => r.taskId)
const report = await agent(
  [
    `All tasks for ${sprintId} have reached a terminal state.`,
    `STEP 0 (token accounting — run BEFORE collate): Run \`node .forge/tools/forge-usage-report.cjs --sprint ${sprintId} --apply\`.`,
    `   This deterministically reconciles per-phase token usage from this run's Workflow transcript (including every`,
    `   task sub-workflow) onto the sprint's COMPLETE events, so collate's cost section is accurate. It is a Bash tool`,
    `   (reads the Workflow harness transcript — no MCP equivalent; documented boundary). BEST-EFFORT: if it prints`,
    `   "no workflow transcript dir" or errors, log and continue — do NOT abort collation.`,
    'Then call mcp__forge__collate { "sprintId":"' + sprintId + '" }.',
    `Then set the sprint status: "completed" if all tasks committed, otherwise "partially-completed", via mcp__forge__store { "command":"update-status", "args":["sprint","${sprintId}","<status>"] }.`,
    `Per-task outcomes: ${JSON.stringify(results.map(r => ({ id: r.taskId, status: r.status })))}.`,
    // Gap #4 (AC4): emit sprint-complete event with outcome counts.
    `Then emit a sprint-complete event via:`,
    `mcp__forge__store { "command":"emit", "args":["${sprintId}",`,
    `'{"eventId":"<uuid-v4>","type":"sprint-complete","sprintId":"${sprintId}",`,
    `"role":"orchestrator","action":"sprint-complete",`,
    `"startTimestamp":"<sprint-start-ISO>","endTimestamp":"<ISO-now>","durationMinutes":<elapsed>,`,
    `"model":"<your-model-id>","provider":"anthropic",`,
    `"taskCount":${results.length},"completedTaskIds":${JSON.stringify(committedIds)},`,
    `"verdict":"${committed === results.length ? 'complete' : 'partial'}",`,
    `"waveCount":${waves.length},"maxConcurrency":${mode === 'sequential' ? 1 : waves.reduce((m, w) => Math.max(m, w.length), 1)}}'] }`,
    'Replace placeholders: <uuid-v4>=UUID v4, <sprint-start-ISO>=sprint start timestamp,',
    '<ISO-now>=current UTC ISO 8601, <elapsed>=minutes elapsed since sprint-start, <your-model-id>=actual model.',
    // Gap #16 (AC2): rebuild context pack — mirrors collator_agent.md §Algorithm §3.
    // On exit 1 (architecture dir absent), skip silently.
    `Then rebuild the context pack (no MCP tool — Bash boundary): node .forge/tools/build-context-pack.cjs --arch-dir engineering/architecture --out-md .forge/cache/context-pack.md --out-json .forge/cache/context-pack.json`,
    '(If build-context-pack.cjs exits 1 because the architecture dir is absent, skip silently and continue.)',
    // Gap #16 (AC3): write WRITEBACK-SUMMARY.json to sprint artifact path.
    // Use sprint.path from the store read (not a reconstructed template).
    `Then read the sprint record: mcp__forge__store { "command":"read", "args":["sprint","${sprintId}","--json"] }`,
    'Extract sprint.path. Write WRITEBACK-SUMMARY.json to that path with this shape:',
    `{ "objective": "Sprint ${sprintId} collation complete", "key_changes": [<list of committed task ids>], "verdict": "<complete|partial>", "written_at": "<ISO-now>" }`,
    // Gap #16 (AC4): invoke forge:refresh-kb-links via Skill tool.
    `Then invoke the forge:refresh-kb-links skill via the Skill tool to refresh KB and workflow links in agent instruction files.`,
    `Return committed/escalated/carriedOver counts and a one-line summary.`,
  ].join(' '),
  { label: `collate:${sprintId}`, phase: 'Collate', schema: COLLATE_SCHEMA }
)

log(`🌊 Sprint ${sprintId} complete — 〇 committed:${committed}  △ escalated:${escalated}  ── carried/abandoned:${carriedOver}`)
log(`Next: /forge:retrospective ${sprintId}`)

return {
  sprintId,
  mode,
  waves,
  results,
  counts: report || { committed, escalated, carriedOver },
}
