'use strict';

// postflight-gate.cjs — evaluates a phase's declared outputs block against
// the filesystem / task state, post-subagent-return. Returns structured failure
// data so the orchestrator (run-task.ts) can halt before FSM advance when outputs
// are not satisfied.
//
// Pure function: only fs.existsSync / fs.statSync / fs.readFileSync. No writes,
// no network, no LLM, no process spawns.
//
// CLI shim: node postflight-gate.cjs --phase <name> --task <taskId>
// Exit codes: 0 (ok), 1 (guard failed), 2 (invalid args / parse error)
// Stdout on exit 1: single JSON line { phase, reasonCode, detail, remediation }
//
// New reasonCodes (FORGE-S26-T19):
//   output-missing  — artifact path does not exist
//   output-stub     — artifact exists but size < min= bytes
//   require-failed  — require predicate over state field failed
//   tool-error      — internal parse or store-read failure

const fs = require('node:fs');
const path = require('node:path');
const { deriveSprintTaskFromArtifactPath } = require('./preflight-gate.cjs');

/**
 * postflight({ phase, outputs, state, substitutions })
 *
 * @param {string} phase - Phase name (e.g. 'implement')
 * @param {object} outputs - Parsed outputs spec from parseOutputs() — { [phase]: { artifacts, require } }
 * @param {object} [state] - Task/bug state for require predicates (e.g. { task: {...} })
 * @param {object} [substitutions] - Template variable substitutions (e.g. { engineering, sprint, task })
 * @returns {{ ok: boolean, missing: string[], reasonCode: string, detail: string, remediation: string }}
 */
function postflight({ phase, outputs, state = {}, substitutions = {} }) {
  const spec = outputs && outputs[phase];
  if (!spec) {
    // No outputs block for this phase — pass through (no-op)
    return { ok: true, missing: [], reasonCode: null, detail: '', remediation: '' };
  }

  const missing = [];

  for (const art of spec.artifacts || []) {
    const resolved = applySubstitutions(art.path, substitutions);
    let exists = false;
    let size = 0;
    try {
      const st = fs.statSync(resolved);
      exists = st.isFile();
      size = st.size;
    } catch (_) {
      exists = false;
    }
    if (!exists) {
      missing.push(`output-missing: artifact absent: ${resolved}`);
    } else if (size < (art.minBytes || 0)) {
      missing.push(
        `output-stub: artifact too small: ${resolved} (${size} bytes, need >= ${art.minBytes})`,
      );
    }
  }

  for (const pred of spec.require || []) {
    if (!evalPredicate(pred, state)) {
      missing.push(
        `require-failed: ${describePredicate(pred)} (got ${JSON.stringify(readField(pred.field, state))})`,
      );
    }
  }

  if (missing.length === 0) {
    return { ok: true, missing: [], reasonCode: null, detail: '', remediation: '' };
  }

  const { reasonCode, detail, remediation } = buildStructuredFailure(phase, missing);
  return { ok: false, missing, reasonCode, detail, remediation };
}

function applySubstitutions(template, subs) {
  return template.replace(/\{(\w+)\}/g, (full, key) => {
    if (Object.prototype.hasOwnProperty.call(subs, key)) return String(subs[key]);
    return full;
  });
}

function walkPath(dottedPath, root) {
  const parts = dottedPath.split('.');
  let cur = root;
  for (const p of parts) {
    if (cur === null || cur === undefined) return undefined;
    cur = cur[p];
  }
  return cur;
}

function readField(dottedPath, state) {
  // Direct walk first (supports explicit `task.status` / `bug.status` paths).
  const direct = walkPath(dottedPath, state);
  if (direct !== undefined) return direct;
  // The materialized outputs blocks use BARE record paths
  // (`summaries.plan.verdict`) while the CLI wraps the record as
  // { task: record } — fall back to the entity record so those resolve.
  // (CART-S03-T01 false-halt: every bare require evaluated undefined.)
  if (state && state.task) {
    const viaTask = walkPath(dottedPath, state.task);
    if (viaTask !== undefined) return viaTask;
  }
  if (state && state.bug) {
    const viaBug = walkPath(dottedPath, state.bug);
    if (viaBug !== undefined) return viaBug;
  }
  return undefined;
}

/**
 * Build template substitutions for an entity's artifact directory.
 * Canonical layouts:
 *   tasks: <engineering>/sprints/<sprintId>/<taskId>/
 *   bugs:  <engineering>/bugs/<bugId>/
 * {sprint} is defined as the path segment under <engineering> that contains
 * the entity dir — `sprints/<sprintId>` for tasks, `bugs` for bugs.
 *
 * PREFERRED resolution: derive {sprint}/{task} from the entity's stored
 * artifact path (rec.path) — the authoritative on-disk location written at
 * creation time and where the phases actually write PLAN.md / REVIEW.md / etc.
 * This is slug- and nesting-accurate, so it covers projects whose directory
 * names don't equal the bare IDs (sprintId "S39" → dir
 * "sprint_39_helpdesk_front_door", taskId "WI-S39-T01" → dir
 * "WI-S39-T01-inbound-email-threading"). Reusing the SAME helper the preflight
 * gate uses (deriveSprintTaskFromArtifactPath) keeps the two gates in agreement
 * about the artifact location for every phase.
 * (WI-S39-T01 false-halt: postflight reconstructed sprints/S39/WI-S39-T01 from
 * IDs and reported output-missing for a PLAN.md that existed under the slug dir.
 * CART-S03-T01 false-halt: {sprint} dropped the `sprints/` segment entirely.)
 *
 * FALLBACK (no usable rec.path — legacy records, or a non-`sprints/` path such
 * as a bug dir): reconstruct from IDs.
 */
function buildSubstitutions({ taskRecord, engineeringRoot, entityId }) {
  const rec = taskRecord || {};

  // deriveSprintTaskFromArtifactPath returns {sprint} as the segments BELOW
  // `sprints/` (preflight's template re-adds the literal `sprints/`). Postflight's
  // {sprint} convention INCLUDES the `sprints/` segment, so prepend it here.
  const derived = deriveSprintTaskFromArtifactPath(rec.path, engineeringRoot);
  if (derived) {
    return {
      engineering: engineeringRoot,
      sprint: `sprints/${derived.sprint}`,
      task: derived.task,
    };
  }

  const isBug = Boolean(rec.bugId) || /-BUG-/.test(String(entityId || ''));
  let sprint;
  if (isBug) {
    sprint = 'bugs';
  } else if (rec.sprintId) {
    sprint = `sprints/${rec.sprintId}`;
  } else {
    sprint = '{sprint}'; // unresolvable — leave the placeholder visible in the report
  }
  return {
    engineering: engineeringRoot,
    sprint,
    task: entityId,
  };
}

function evalPredicate(pred, state) {
  const actual = readField(pred.field, state);
  switch (pred.op) {
    case '==':
      return String(actual) === String(pred.value);
    case '!=':
      return String(actual) !== String(pred.value);
    case 'in':
      return (pred.value || []).map(String).includes(String(actual));
    default:
      throw new Error(`postflight-gate: unknown predicate op "${pred.op}"`);
  }
}

function describePredicate(pred) {
  if (pred.op === 'in') return `${pred.field} in [${(pred.value || []).join(', ')}]`;
  return `${pred.field} ${pred.op} ${pred.value}`;
}

function buildStructuredFailure(phase, missing) {
  let reasonCode = 'tool-error';
  const detailParts = [];

  for (const m of missing) {
    detailParts.push(m);
    if (reasonCode === 'tool-error') {
      if (/^output-missing/i.test(m)) {
        reasonCode = 'output-missing';
      } else if (/^output-stub/i.test(m)) {
        reasonCode = 'output-stub';
      } else if (/^require-failed/i.test(m)) {
        reasonCode = 'require-failed';
      }
    }
  }

  const detail = detailParts.join('; ');

  const remediationMap = {
    'output-missing': 'Re-run the phase that produces this artifact (e.g. /forge:implement), then retry.',
    'output-stub': 'The artifact was produced but appears incomplete (stub). Ensure the phase wrote a complete file.',
    'require-failed': 'Correct the task/bug state so it satisfies the postflight require predicate, then retry.',
    'tool-error': 'Check the outputs block configuration and store records; run node .forge/tools/postflight-gate.cjs manually for diagnostics.',
  };

  return {
    reasonCode,
    detail,
    remediation: remediationMap[reasonCode],
  };
}

module.exports = { postflight, buildSubstitutions };

// CLI shim — only runs when invoked directly
if (require.main === module) {
  const args = parseArgs(process.argv.slice(2));
  if (!args.phase || !args.task) {
    process.stderr.write('Usage: postflight-gate.cjs --phase <phaseName> --task <taskId>\n');
    process.exit(2);
  }

  const { parseOutputs } = require('./parse-gates.cjs');

  // Resolve config
  let engineeringRoot = 'engineering';
  try {
    const cfg = JSON.parse(fs.readFileSync(path.resolve(process.cwd(), '.forge/config.json'), 'utf8'));
    if (cfg.paths && cfg.paths.engineering) engineeringRoot = cfg.paths.engineering;
  } catch (_) { /* fall back to default */ }

  // Load task record
  let taskRecord = null;
  try {
    const store = require('./store.cjs');
    taskRecord = store.getTask(args.task);
  } catch (_) {
    // store.cjs not available or task not found — continue with substitutions from args
  }

  // Build substitutions (canonical entity-dir resolution — sprints/<id> or bugs)
  const substitutions = buildSubstitutions({
    taskRecord,
    engineeringRoot,
    entityId: args.task,
  });

  // Load workflow markdown (scan .forge/workflows/ for phase)
  const workflowMd = loadWorkflowMarkdown(args.phase);
  if (!workflowMd) {
    process.stderr.write(`postflight-gate: no outputs block defined for phase "${args.phase}" — skipping\n`);
    process.exit(0);
  }

  let outputs;
  try {
    outputs = parseOutputs(workflowMd);
  } catch (err) {
    process.stderr.write(`postflight-gate: ${err.message}\n`);
    process.exit(2);
  }

  if (!outputs[args.phase]) {
    process.stderr.write(`postflight-gate: no outputs block for phase "${args.phase}" — skipping\n`);
    process.exit(0);
  }

  // Build state from task record
  const state = {};
  if (taskRecord) state.task = taskRecord;

  const result = postflight({ phase: args.phase, outputs, state, substitutions });

  if (result.ok) process.exit(0);

  process.stderr.write(`Postflight guard failed for phase "${args.phase}":\n`);
  for (const m of result.missing) process.stderr.write(`  - ${m}\n`);

  // Emit structured JSON on stdout for orchestrators
  process.stdout.write(JSON.stringify({
    phase: args.phase,
    reasonCode: result.reasonCode,
    detail: result.detail,
    remediation: result.remediation,
  }) + '\n');
  process.exit(1);
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--phase') out.phase = argv[++i];
    else if (a === '--task') out.task = argv[++i];
  }
  return out;
}

function loadWorkflowMarkdown(phaseName) {
  const workflowsDir = path.resolve(process.cwd(), '.forge/workflows');
  let entries;
  try {
    entries = fs.readdirSync(workflowsDir).filter((f) => f.endsWith('.md'));
  } catch (_) {
    return null;
  }
  const fencePattern = new RegExp('^```outputs\\s+phase=' + escapeRegex(phaseName) + '\\s*$', 'm');
  for (const entry of entries) {
    const md = fs.readFileSync(path.join(workflowsDir, entry), 'utf8');
    if (fencePattern.test(md)) return md;
  }
  return null;
}

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
