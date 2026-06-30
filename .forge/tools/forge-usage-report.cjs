#!/usr/bin/env node
'use strict';
// forge-usage-report.cjs — deterministic token accounting for Claude Code
// orchestration runs (FORGE-S38).
//
// WHY: when a Forge orchestration runs as a code-orchestrated Workflow in Claude
// Code (wfl:run-task / wfl:run-sprint / wfl:fix-bug), each phase is a spawned
// subagent. The pi-runtime usage-hook that writes token sidecars does NOT exist
// in this path, so per-phase cost was never captured (the per-phase merge-sidecar
// dispatch found nothing and was removed). But the Workflow harness already
// persists ground-truth usage in its transcripts — so we reconcile it here, with
// NO LLM and NO extra agent dispatch.
//
// HOW: each agent-<id>.jsonl carries real provider usage per assistant turn, and
// the agent's own `action:"complete"` store-emit call names the exact eventId it
// wrote to disk. We sum usage per agent, join agent → eventId via that emit, and
// stamp the canonical token fields onto the matching COMPLETE event. Agents with
// usage but no complete-emit (escalate/skip/dispatch helpers) roll up to an
// `overhead` bucket. Idempotent; no-ops events that already carry a tokenSource
// (e.g. pi-populated), so a hybrid run is never double-counted.
//
// See doc/analysis/workflow-token-accounting.md for the full design.
//
// Usage:
//   node forge-usage-report.cjs --sprint <sprintId> [--workflow-dir <dir>]
//        [--project-dir <dir>] [--apply | --dry-run] [--json]
//
// --workflow-dir  the Workflow transcript dir (…/subagents/workflows/wf_*).
//                 If omitted, auto-discovers the most-recently-modified wf_* dir
//                 under $HOME/.claude/projects/**. Pass explicitly when multiple
//                 orchestrations run concurrently (auto-discovery is newest-wins).
//   --project-dir  the Forge project root holding .forge/store (default:
//                 CLAUDE_PROJECT_DIR or cwd).
//   --sprint      events/<sprintId>/ to reconcile ("bugs" for the bug pipeline).
//   --apply       write token fields onto events (default: dry-run report only).
//   --json        emit the machine-readable report on stdout.

const fs = require('fs');
const os = require('os');
const path = require('path');

const { loadSchemas } = require('./lib/schema-loader.cjs');
const { validateRecord } = require('./lib/validate.js');
let computeCost;
try { ({ computeCost } = require('./lib/pricing.cjs')); } catch { computeCost = null; }

// Canonical token fields we may write (subset of CANONICAL_TOKEN_FIELDS that this
// tool sources from the transcript). model/provider/timestamps already live on
// the COMPLETE event the subagent emitted — we add only the measured counts.
const TOKEN_FIELDS = ['inputTokens', 'outputTokens', 'cacheReadTokens', 'cacheWriteTokens'];

// ---------------------------------------------------------------------------
// arg parsing
// ---------------------------------------------------------------------------
function parseArgs(argv) {
  const a = { apply: false, json: false };
  for (let i = 0; i < argv.length; i++) {
    const t = argv[i];
    if (t === '--apply') a.apply = true;
    else if (t === '--dry-run') a.apply = false;
    else if (t === '--json') a.json = true;
    else if (t === '--workflow-dir') a.workflowDir = argv[++i];
    else if (t === '--project-dir') a.projectDir = argv[++i];
    else if (t === '--sprint') a.sprint = argv[++i];
    else if (t === '--help' || t === '-h') a.help = true;
  }
  return a;
}

// ---------------------------------------------------------------------------
// workflow-dir discovery
// ---------------------------------------------------------------------------
// Walk $HOME/.claude/projects/**/subagents/workflows/ and return the newest wf_*.
function autoDiscoverWorkflowDir() {
  const projectsRoot = path.join(os.homedir(), '.claude', 'projects');
  if (!fs.existsSync(projectsRoot)) return null;
  let best = null;
  let bestMtime = -1;
  // projects/<proj>/<session>/subagents/workflows/wf_*
  for (const proj of safeReaddir(projectsRoot)) {
    const projPath = path.join(projectsRoot, proj);
    for (const session of safeReaddir(projPath)) {
      const wfBase = path.join(projPath, session, 'subagents', 'workflows');
      for (const wf of safeReaddir(wfBase)) {
        if (!wf.startsWith('wf_')) continue;
        const full = path.join(wfBase, wf);
        let m;
        try { m = fs.statSync(full).mtimeMs; } catch { continue; }
        if (m > bestMtime) { bestMtime = m; best = full; }
      }
    }
  }
  return best;
}

function safeReaddir(dir) {
  try {
    return fs.readdirSync(dir, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name);
  } catch { return []; }
}

// Collect agent-*.jsonl recursively (nested sub-workflows live under the tree).
function collectAgentFiles(dir) {
  const out = [];
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return out; }
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...collectAgentFiles(full));
    else if (e.isFile() && /^agent-.*\.jsonl$/.test(e.name)) out.push(full);
  }
  return out;
}

// ---------------------------------------------------------------------------
// transcript parsing
// ---------------------------------------------------------------------------
function parseJsonl(file) {
  let raw;
  try { raw = fs.readFileSync(file, 'utf8'); } catch { return []; }
  const recs = [];
  for (const line of raw.split('\n')) {
    if (!line.trim()) continue;
    try { recs.push(JSON.parse(line)); } catch { /* skip malformed line */ }
  }
  return recs;
}

// Sum provider usage across an agent's assistant turns.
function sumUsage(records) {
  const acc = { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0 };
  for (const r of records) {
    const u = r && r.message && r.message.usage;
    if (!u) continue;
    acc.inputTokens += u.input_tokens || 0;
    acc.outputTokens += u.output_tokens || 0;
    acc.cacheReadTokens += u.cache_read_input_tokens || 0;
    acc.cacheWriteTokens += u.cache_creation_input_tokens || 0;
  }
  return acc;
}

// Find the eventId this agent stamped on its own COMPLETE event, by scanning its
// store-emit tool calls (mcp__forge__store emit, or a Bash store-cli emit). The
// join is exact: the emit payload's eventId IS the on-disk event filename.
function findCompleteEventId(records) {
  for (const r of records) {
    const content = r && r.message && r.message.content;
    if (!Array.isArray(content)) continue;
    for (const b of content) {
      if (!b || b.type !== 'tool_use') continue;
      const candidates = [];
      // mcp__forge__store { command:"emit", args:[sprintId, "<json>"] }
      if (b.name === 'mcp__forge__store' && b.input && b.input.command === 'emit' && Array.isArray(b.input.args)) {
        candidates.push(b.input.args.find((x) => typeof x === 'string' && x.includes('"action"')));
      }
      // Bash store-cli.cjs emit <sprintId> '<json>' — pull the embedded JSON.
      if (b.name === 'Bash' && b.input && typeof b.input.command === 'string') {
        const m = b.input.command.match(/\{.*"action".*\}/s);
        if (m) candidates.push(m[0]);
      }
      for (const c of candidates) {
        const ev = tryParseEvent(c);
        if (ev && ev.action === 'complete' && ev.eventId) {
          return { eventId: ev.eventId, sprintId: ev.sprintId };
        }
      }
    }
  }
  return null;
}

function tryParseEvent(s) {
  if (typeof s !== 'string') return null;
  try { return JSON.parse(s); } catch { return null; }
}

function hasUsage(u) {
  return u.inputTokens || u.outputTokens || u.cacheReadTokens || u.cacheWriteTokens;
}

function addInto(target, u) {
  for (const f of TOKEN_FIELDS) target[f] += u[f];
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write('Usage: forge-usage-report.cjs --sprint <id> [--workflow-dir <dir>] [--project-dir <dir>] [--apply|--dry-run] [--json]\n');
    return 0;
  }
  if (!args.sprint) {
    process.stderr.write('forge-usage-report: --sprint <sprintId> is required\n');
    return 2;
  }

  const projectDir = path.resolve(args.projectDir || process.env.CLAUDE_PROJECT_DIR || process.cwd());
  const eventsDir = path.join(projectDir, '.forge', 'store', 'events', args.sprint);

  const wfDir = args.workflowDir || autoDiscoverWorkflowDir();
  if (!wfDir || !fs.existsSync(wfDir)) {
    process.stderr.write(`forge-usage-report: no workflow transcript dir (looked for ${wfDir || 'auto-discover'}). Skipping.\n`);
    // Best-effort: not an error for the finalize caller — nothing to reconcile.
    if (args.json) process.stdout.write(JSON.stringify({ skipped: true, reason: 'no-workflow-dir', attributed: 0, overhead: zeroBucket() }) + '\n');
    return 0;
  }

  const schemas = loadSchemas();
  const agentFiles = collectAgentFiles(wfDir);

  const overhead = zeroBucket();
  const perEvent = new Map();   // eventId -> usage bucket
  let unattributed = 0;

  for (const file of agentFiles) {
    const recs = parseJsonl(file);
    const usage = sumUsage(recs);
    if (!hasUsage(usage)) continue;
    const hit = findCompleteEventId(recs);
    if (!hit) { addInto(overhead, usage); continue; }   // helper / overhead agent
    if (!perEvent.has(hit.eventId)) perEvent.set(hit.eventId, zeroBucket());
    addInto(perEvent.get(hit.eventId), usage);
  }

  const applied = [];
  const skippedExisting = [];
  for (const [eventId, usage] of perEvent) {
    const evPath = path.join(eventsDir, `${eventId}.json`);
    if (!fs.existsSync(evPath)) { unattributed++; continue; }   // event not on disk
    let event;
    try { event = JSON.parse(fs.readFileSync(evPath, 'utf8')); } catch { unattributed++; continue; }

    // No-op when already populated (e.g. pi's usage-hook got there first).
    if (event.tokenSource) { skippedExisting.push(eventId); continue; }

    const merged = { ...event };
    for (const f of TOKEN_FIELDS) merged[f] = usage[f];
    merged.tokenSource = 'reported';

    const errs = validateRecord(merged, schemas.event, { entity: 'event' });
    if (errs.length) {
      process.stderr.write(`forge-usage-report: event ${eventId} would fail validation, skipping: ${errs.join('; ')}\n`);
      unattributed++;
      continue;
    }
    if (args.apply) fs.writeFileSync(evPath, JSON.stringify(merged, null, 2) + '\n');
    applied.push({ eventId, ...usage, cost: cost(usage, event.model) });
  }

  const report = {
    workflowDir: wfDir,
    sprint: args.sprint,
    applied: args.apply,
    attributed: applied.length,
    skippedExisting: skippedExisting.length,
    unattributed,
    events: applied,
    overhead: { ...overhead, cost: cost(overhead, null) },
    totals: rollup(applied, overhead),
  };

  if (args.json) {
    process.stdout.write(JSON.stringify(report) + '\n');
  } else {
    printHuman(report);
  }
  return 0;
}

function zeroBucket() {
  return { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0 };
}

function cost(usage, model) {
  if (!computeCost) return null;
  try { return computeCost({ ...usage, model: model || 'claude-sonnet-4-6' }); } catch { return null; }
}

function rollup(events, overhead) {
  const t = zeroBucket();
  for (const e of events) addInto(t, e);
  addInto(t, overhead);
  return { ...t, cost: cost(t, null) };
}

function printHuman(r) {
  const lines = [];
  lines.push(`Token usage — sprint ${r.sprint}  (${r.applied ? 'applied' : 'dry-run'})`);
  lines.push(`  workflow: ${r.workflowDir}`);
  for (const e of r.events) {
    lines.push(`  ${e.eventId.padEnd(48)} in=${e.inputTokens} out=${e.outputTokens} cr=${e.cacheReadTokens} cw=${e.cacheWriteTokens}`);
  }
  lines.push(`  ${'overhead (unattributed helpers)'.padEnd(48)} in=${r.overhead.inputTokens} out=${r.overhead.outputTokens}`);
  lines.push(`  attributed=${r.attributed} skippedExisting=${r.skippedExisting} unattributed=${r.unattributed}`);
  const tc = r.totals.cost;
  lines.push(`  TOTAL  in=${r.totals.inputTokens} out=${r.totals.outputTokens} cr=${r.totals.cacheReadTokens} cw=${r.totals.cacheWriteTokens}${tc != null ? `  ~$${tc.toFixed ? tc.toFixed(4) : tc}` : ''}`);
  process.stdout.write(lines.join('\n') + '\n');
}

if (require.main === module) {
  process.exit(main());
}

module.exports = { sumUsage, findCompleteEventId, collectAgentFiles, autoDiscoverWorkflowDir };
