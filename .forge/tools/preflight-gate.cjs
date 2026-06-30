'use strict';

// preflight-gate.cjs — evaluates a phase's declared gates against the current
// task state, pre-spawn. Returns { ok, missing[] } so the orchestrator (or a
// manual command) can halt loudly before invoking a subagent on broken
// prerequisites.
//
// Pure function: only fs.existsSync / fs.statSync / fs.readFileSync. No writes,
// no network, no process spawns.

const fs = require('node:fs');
const path = require('node:path');
const { readVerdict } = require('./read-verdict.cjs');

function preflight({ phase, gates, state = {}, substitutions = {} }) {
  const spec = gates && gates[phase];
  if (!spec) {
    return { ok: false, missing: [`no gate definition registered for phase "${phase}" (unknown phase or missing gates block)`] };
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
      missing.push(`artifact missing: ${resolved}`);
    } else if (size < (art.minBytes || 0)) {
      missing.push(`artifact too small (stub): ${resolved} (${size} bytes, need >= ${art.minBytes})`);
    }
  }

  for (const pred of spec.require || []) {
    if (!evalPredicate(pred, state)) {
      missing.push(`require failed: ${describePredicate(pred)} (got ${JSON.stringify(readField(pred.field, state))})`);
    }
  }

  for (const pred of spec.forbid || []) {
    if (evalPredicate(pred, state)) {
      missing.push(`forbid triggered: ${describePredicate(pred)}`);
    }
  }

  // `after <phase> = approved` predicates read structured verdicts from the
  // store record (task.summaries.<canonical>.verdict, or task.status for the
  // approve phase). The markdown `**Verdict:**` line in review artifacts is
  // a human breadcrumb and no longer load-bearing.
  const record = state.task || state.bug || null;
  for (const after of spec.after || []) {
    if (!record) {
      missing.push(`predecessor verdict unreadable for phase "${after.phase}": no task/bug record in state`);
      continue;
    }
    const entityType = state.bug ? 'bug' : 'task';
    const { verdict, source, key } = readVerdict({ record, phase: after.phase, entityType });
    if (verdict === null) {
      const where = key ? `${source}["${key}"]` : source;
      missing.push(
        `predecessor verdict missing for phase "${after.phase}" ` +
        `(expected in ${where}). Subagent likely failed to call set-summary ` +
        `(or, for approve, did not transition task.status to "approved").`
      );
    } else if (verdict !== after.verdict) {
      missing.push(`predecessor "${after.phase}" verdict is "${verdict}", expected "${after.verdict}"`);
    }
  }

  return { ok: missing.length === 0, missing };
}

function applySubstitutions(template, subs) {
  return template.replace(/\{(\w+)\}/g, (full, key) => {
    if (Object.prototype.hasOwnProperty.call(subs, key)) return String(subs[key]);
    return full;
  });
}

function readField(dottedPath, state) {
  const parts = dottedPath.split('.');
  let cur = state;
  for (const p of parts) {
    if (cur === null || cur === undefined) return undefined;
    cur = cur[p];
  }
  return cur;
}

function evalPredicate(pred, state) {
  const actual = readField(pred.field, state);
  switch (pred.op) {
    case '==':
      return String(actual) === String(pred.value);
    case '!=':
      return String(actual) !== String(pred.value);
    case 'in':
      return pred.value.map(String).includes(String(actual));
    default:
      throw new Error(`preflight-gate: unknown predicate op "${pred.op}"`);
  }
}

function describePredicate(pred) {
  if (pred.op === 'in') return `${pred.field} in [${pred.value.join(', ')}]`;
  return `${pred.field} ${pred.op} ${pred.value}`;
}

module.exports = { preflight, resolveTaskArtifactDir, resolveSprintArtifactDir, deriveSprintTaskFromArtifactPath };

// Derive {sprint} and {task} substitutions directly from a task's artifact path
// (task.path) when that path is a directory under `<engineeringRoot>/sprints/`.
//
// task.path is the authoritative artifact location — it is where the plan /
// implement phases actually write PLAN.md, REVIEW.md, etc. Splitting it tolerates
// ANY nesting depth and ANY on-disk naming, so a single rule covers both the
// modern 2-level layout (`sprints/<sprint>/<task>/`) and legacy 3-level layouts
// (`sprints/<sprint>/tasks/<task>/`) whose on-disk sprint dir name need not equal
// sprintId (e.g. sprintId "S19" → dir "sprint_19_platform_ux_improvements"). The
// directory-scan resolution (resolveSprintArtifactDir + resolveTaskArtifactDir)
// cannot express the `tasks/` nesting, which is what broke review-plan preflight
// for heterogeneous-layout projects (walkinto.in S19/S32/S33).
//
//   {task}   = the last path segment (the artifact directory name)
//   {sprint} = everything between `sprints/` and that last segment, joined by "/"
//
// File-vs-dir guard: if task.path ends in a filename (has an extension), the
// artifact directory is its PARENT — strip the filename first. This prevents the
// bogus `.../TASK_PROMPT.md/PLAN.md` (ENOTDIR) seen when a project stores
// task.path as a file inside the artifact dir.
//
// Returns null when the path is absent, or not under `<engineeringRoot>/sprints/`
// (e.g. a forge/ source-file path — see regression S12-T06), or resolves to
// fewer than 2 directory segments. In every null case the caller falls back to
// the directory-scan resolution, preserving prior behaviour.
function deriveSprintTaskFromArtifactPath(taskPath, engineeringRoot) {
  if (!taskPath) return null;
  let norm = String(taskPath).replace(/\\/g, '/').replace(/\/+$/, '');
  // File-vs-dir guard — drop a trailing filename so we split on the dir.
  if (/\.[a-zA-Z0-9]+$/.test(norm)) {
    norm = norm.replace(/\/[^/]*$/, '');
  }
  const er = String(engineeringRoot || 'engineering').replace(/\/+$/, '');
  const prefix = er + '/sprints/';
  if (!norm.startsWith(prefix)) return null;
  const segs = norm.slice(prefix.length).split('/').filter(Boolean);
  if (segs.length < 2) return null;
  const task = segs[segs.length - 1];
  const sprint = segs.slice(0, -1).join('/');
  return { sprint, task };
}

// CLI shim: `node preflight-gate.cjs --phase <name> --task <taskId> [--bug <bugId>] [--workflow <name>]`
// exit codes: 0 ok, 1 gate(s) failed, 2 invalid args / missing definitions

// Scan the sprints root for a directory matching sprintId exactly or as a prefix slug.
//
// Two patterns supported (in order of preference):
//   1. <sprintId>          — exact match (no slug)
//   2. <sprintId>-<slug>   — slug suffix (e.g. "FORGE-S25-foundation-refactor")
//
// Returns the directory name (e.g. "FORGE-S25-foundation-refactor") or null.
function resolveSprintArtifactDir(sprintId, engineeringRoot, cwd) {
  if (!sprintId) return null;
  const base = cwd || process.cwd();
  const sprintsRoot = path.resolve(base, engineeringRoot, 'sprints');
  let entries;
  try {
    entries = fs.readdirSync(sprintsRoot);
  } catch (_) {
    return null;
  }
  const isDir = (entry) => {
    try {
      return fs.statSync(path.join(sprintsRoot, entry)).isDirectory();
    } catch (_) {
      return false;
    }
  };
  // Pass 1: exact match
  for (const entry of entries) {
    if (!isDir(entry)) continue;
    if (entry === sprintId) return entry;
  }
  // Pass 2: slug match — dir starts with "<sprintId>-"
  for (const entry of entries) {
    if (!isDir(entry)) continue;
    if (entry.startsWith(sprintId + '-')) return entry;
  }
  return null;
}

// Scan the sprint directory for a subdirectory matching the task.
//
// Two patterns supported (in order of preference, to avoid false positives):
//   1. <taskId>-<slug>           — canonical (taskId already contains sprint prefix,
//                                  e.g., "FORGE-S21-T02-...").
//   2. <sprintId>-<taskId>-<slug>  — projects using bare task IDs (e.g., "T-C1-1")
//                                  whose sprint-scoped dir names carry the sprint
//                                  prefix explicitly (e.g., "S003-T-C1-1-...").
//
// Returns the directory name or null.
// The optional resolvedSprintDir parameter allows the caller to pass a pre-resolved
// sprint directory name (e.g. "FORGE-S25-foundation-refactor") so the task scan
// happens inside the correct on-disk directory rather than the bare sprintId dir.
function resolveTaskArtifactDir(taskRecord, engineeringRoot, cwd, resolvedSprintDir) {
  if (!taskRecord || !taskRecord.sprintId || !taskRecord.taskId) return null;
  const base = cwd || process.cwd();
  const sprintDirName = resolvedSprintDir || taskRecord.sprintId;
  const sprintDir = path.resolve(base, engineeringRoot, 'sprints', sprintDirName);
  let entries;
  try {
    entries = fs.readdirSync(sprintDir);
  } catch (_) {
    return null;
  }
  const taskId = taskRecord.taskId;
  const isDir = (entry) => {
    try {
      return fs.statSync(path.join(sprintDir, entry)).isDirectory();
    } catch (_) {
      return false;
    }
  };
  // Pass 1: canonical match — accept exact `<taskId>` (modern convention,
  // no slug) or `<taskId>-<slug>` (legacy convention).
  for (const entry of entries) {
    if (!isDir(entry)) continue;
    if (entry === taskId || entry.startsWith(taskId + '-')) return entry;
  }
  // Pass 2: sprint-prefixed match. Restrict to dirs that look like
  // "<sprintId>-<taskId>(-<slug>)?" to avoid matching unrelated names that
  // happen to contain the task ID as a substring.
  const sprintPrefix = taskRecord.sprintId + '-';
  for (const entry of entries) {
    if (!isDir(entry)) continue;
    if (!entry.startsWith(sprintPrefix)) continue;
    const rest = entry.slice(sprintPrefix.length);
    if (rest === taskId || rest.startsWith(taskId + '-')) return entry;
  }
  return null;
}

if (require.main === module) {
  const args = parseArgs(process.argv.slice(2));
  if (!args.phase || (!args.task && !args.bug)) {
    process.stderr.write('Usage: preflight-gate.cjs --phase <phaseName> --task <taskId> [--bug <bugId>]\n');
    process.exit(2);
  }

  const { parseGates } = require('./parse-gates.cjs');
  const store = require('./store.cjs');

  // Resolve store records and substitutions BEFORE calling loadWorkflowMarkdown
  // so the placeholder-key filter can use them to select the correct workflow file.
  // (Previously loadWorkflowMarkdown was called first, causing fix_bug.md to shadow
  // orchestrate_task.md for phases shared between the two workflows — forge#72.)
  const taskRecord = args.task ? safe(() => store.getTask(args.task)) : null;
  const bugRecord = args.bug ? safe(() => store.getBug(args.bug)) : null;
  const state = {};
  if (taskRecord) state.task = taskRecord;
  if (bugRecord) state.bug = bugRecord;

  // Resolve engineering root from config; path templates can reference {engineering}.
  let engineeringRoot = 'engineering';
  try {
    const cfg = JSON.parse(fs.readFileSync(path.resolve(process.cwd(), '.forge/config.json'), 'utf8'));
    if (cfg.paths && cfg.paths.engineering) engineeringRoot = cfg.paths.engineering;
  } catch (_) { /* fall back to default */ }

  // {task} / {bug} in path templates refer to the artifact directory suffix
  // (e.g., "FORGE-S12-T06-model-discovery", "BUG-007-broken-foo").
  // task.path is the primary source file in forge/, NOT the artifact directory.
  // Scan the sprint directory to find the correct artifact directory name.
  function lastSegment(p) {
    const parts = String(p || '').split('/').filter(Boolean);
    return parts[parts.length - 1] || '';
  }
  // Resolve the sprint directory name first (may have a slug suffix, e.g.
  // "FORGE-S25-foundation-refactor" for sprintId "FORGE-S25"). This resolved
  // name is used both as the {sprint} substitution and as the directory in
  // which resolveTaskArtifactDir scans for task subdirectories.
  const sprintId = taskRecord ? taskRecord.sprintId : undefined;
  // Primary resolution: derive {sprint}/{task} straight from task.path (the
  // authoritative artifact directory). This honours arbitrary layout nesting —
  // including legacy 3-level `sprints/<sprint>/tasks/<task>/` trees the
  // directory-scan resolution below cannot express. Falls back to the scan when
  // task.path is not an artifact dir under sprints/ (e.g. a forge/ source file).
  const derived = taskRecord ? deriveSprintTaskFromArtifactPath(taskRecord.path, engineeringRoot) : null;
  const resolvedSprintDir = derived
    ? derived.sprint
    : (sprintId ? (resolveSprintArtifactDir(sprintId, engineeringRoot) || sprintId) : undefined);
  const taskArtifactDir = derived
    ? derived.task
    : resolveTaskArtifactDir(taskRecord, engineeringRoot, undefined, resolvedSprintDir);
  // Fallback: if task.path points to a file (e.g., TASK_PROMPT.md) we want
  // the *directory* segment, not the filename. Without this, a project that
  // stores task.path as the prompt file would propagate "TASK_PROMPT.md" as
  // the {task} substitution, yielding bogus gate paths like ".../TASK_PROMPT.md/PLAN.md".
  function dirSegment(p) {
    const s = String(p || '');
    return /\.[a-zA-Z0-9]+$/.test(s) ? lastSegment(path.dirname(s)) : lastSegment(s);
  }
  const taskDir = taskArtifactDir
    || (taskRecord && taskRecord.path ? dirSegment(taskRecord.path) : args.task);
  const bugDir = bugRecord && bugRecord.path ? dirSegment(bugRecord.path) : args.bug;

  const substitutions = {
    engineering: engineeringRoot,
    sprint: resolvedSprintDir,
    task: taskDir,
    bug: bugDir,
  };

  // Now load the workflow, passing substitutions so the placeholder-key filter
  // can skip workflows whose gate block references keys not present in subs.
  const workflowMd = loadWorkflowMarkdown(args.phase, args.workflow, substitutions);
  if (!workflowMd) {
    // Some phases are gate-less by design (e.g. writeback/collator — a
    // deterministic regen with no predecessor verdict to check). Treat
    // "no workflow declares this phase" as a no-op pass-through rather
    // than a misconfiguration. Exit 2 stays reserved for real arg / parse
    // errors. Note on stderr so operators can see the skip.
    process.stderr.write(`preflight-gate: no preflight gates defined for phase "${args.phase}" — skipping\n`);
    process.exit(0);
  }
  let gates;
  try {
    gates = parseGates(workflowMd);
  } catch (err) {
    process.stderr.write(`preflight-gate: ${err.message}\n`);
    process.exit(2);
  }
  if (!gates[args.phase]) {
    // Workflow exists but declares no gate block for this phase — same
    // semantic as "no workflow" above: phase is gate-less, pass through.
    process.stderr.write(`preflight-gate: no gates block for phase "${args.phase}" — skipping\n`);
    process.exit(0);
  }

  const result = preflight({ phase: args.phase, gates, state, substitutions });
  if (result.ok) process.exit(0);
  process.stderr.write(`Gate failed for phase "${args.phase}":\n`);
  for (const m of result.missing) process.stderr.write(`  - ${m}\n`);
  // Emit structured JSON on stdout for orchestrators to parse and render.
  // Shape: { phase, reasonCode, detail, remediation }
  const structured = buildStructuredFailure(args.phase, result.missing);
  process.stdout.write(JSON.stringify(structured) + '\n');
  process.exit(1);
}

// Build a structured gate-failure object for orchestrators.
// Maps the human-readable `missing[]` strings to a typed { phase, reasonCode, detail, remediation }.
// reasonCode is derived from the dominant failure pattern:
//   artifact-missing            — artifact missing / too small
//   predecessor-verdict-missing — after-clause verdict absent or wrong
//   illegal-status              — require/forbid predicate fired
//   tool-error                  — internal error / unrecognised pattern
// When multiple failures exist, reasonCode reflects the first recognised pattern;
// detail combines all messages; only a single JSON object is ever emitted.
function buildStructuredFailure(phase, missing) {
  let reasonCode = 'tool-error';
  const detailParts = [];

  for (const m of missing) {
    detailParts.push(m);
    if (reasonCode === 'tool-error') {
      if (/^artifact (missing|too small)/i.test(m)) {
        reasonCode = 'artifact-missing';
      } else if (/^predecessor verdict (missing|unreadable)/i.test(m) || /verdict is "/i.test(m)) {
        reasonCode = 'predecessor-verdict-missing';
      } else if (/^(require failed|forbid triggered)/i.test(m)) {
        reasonCode = 'illegal-status';
      }
    }
  }

  const detail = detailParts.join('; ');

  const remediationMap = {
    'artifact-missing': `Re-run the phase that produces this artifact (e.g. /forge:plan or /forge:implement), then retry.`,
    'predecessor-verdict-missing': `Ensure the predecessor review phase completed and recorded a verdict via set-summary, then retry.`,
    'illegal-status': `Correct the task/bug status (use store-cli update-status) so it satisfies the gate predicate, then retry.`,
    'tool-error': `Check the gate configuration and store records for this task; run node .forge/tools/preflight-gate.cjs manually for diagnostics.`,
  };

  return {
    phase,
    reasonCode,
    detail,
    remediation: remediationMap[reasonCode],
  };
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--phase') out.phase = argv[++i];
    else if (a === '--task') out.task = argv[++i];
    else if (a === '--bug') out.bug = argv[++i];
    else if (a === '--workflow') out.workflow = argv[++i];
  }
  return out;
}

function safe(fn) {
  try { return fn(); } catch (_) { return null; }
}

// Extract the gate block body for a given phase from a workflow markdown string.
// Returns the text between the opening and closing fence for that phase, or null.
function extractGateBlockBody(md, phaseName) {
  const openPattern = new RegExp('^```gates\\s+phase=' + escapeRegex(phaseName) + '\\s*$', 'm');
  const openMatch = openPattern.exec(md);
  if (!openMatch) return null;
  const afterOpen = md.slice(openMatch.index + openMatch[0].length);
  const closeIdx = afterOpen.indexOf('\n```');
  if (closeIdx === -1) return afterOpen; // unterminated fence — return what we have
  return afterOpen.slice(0, closeIdx);
}

// Check whether all {placeholder} keys in a gate block body are satisfied by
// the given substitutions map. Returns true if all placeholders are satisfied
// (or there are none), false if any placeholder key is missing from subs.
function gatePlaceholdersSatisfied(gateBody, subs) {
  const tokenRe = /\{(\w+)\}/g;
  let match;
  while ((match = tokenRe.exec(gateBody)) !== null) {
    const key = match[1];
    if (!Object.prototype.hasOwnProperty.call(subs, key) || subs[key] === undefined) {
      return false;
    }
  }
  return true;
}

function loadWorkflowMarkdown(phaseName, workflowName, substitutions) {
  const workflowsDir = path.resolve(process.cwd(), '.forge/workflows');
  let entries;
  try {
    entries = fs.readdirSync(workflowsDir).filter((f) => f.endsWith('.md'));
  } catch (_) {
    return null;
  }
  const fencePattern = new RegExp('^```gates\\s+phase=' + escapeRegex(phaseName) + '\\s*$', 'm');

  // If a specific workflow file was requested, try it first before scanning all files.
  // This prevents alphabetically-earlier files from shadowing the caller's workflow.
  if (workflowName) {
    const normalised = workflowName.endsWith('.md') ? workflowName : workflowName + '.md';
    if (entries.includes(normalised)) {
      const md = fs.readFileSync(path.join(workflowsDir, normalised), 'utf8');
      if (fencePattern.test(md)) return md;
    }
  }

  // Placeholder-key filter: when substitutions are provided, skip any workflow
  // whose gate block for this phase references a {key} not present in subs.
  // This prevents fix_bug.md (which uses {bug}) from shadowing orchestrate_task.md
  // (which uses {task}) when only --task is supplied and --bug is absent.
  // Fall-back: if NO workflow passes the filter, return the first match regardless
  // (preserves existing behaviour for malformed/unknown invocations).
  const subs = substitutions || {};
  const hasSubstitutions = Object.keys(subs).some(k => subs[k] !== undefined);

  let firstMatch = null; // fallback candidate
  for (const entry of entries) {
    const md = fs.readFileSync(path.join(workflowsDir, entry), 'utf8');
    if (!fencePattern.test(md)) continue;

    if (firstMatch === null) firstMatch = md; // remember first match for fallback

    if (!hasSubstitutions) {
      // No substitutions provided — use original first-match behaviour
      return md;
    }

    const gateBody = extractGateBlockBody(md, phaseName);
    if (gateBody !== null && gatePlaceholdersSatisfied(gateBody, subs)) {
      return md;
    }
    // Placeholder(s) unsatisfied — log at warn level and continue scanning
    process.stderr.write(
      `preflight-gate: skipping ${entry} for phase "${phaseName}" — gate block contains ` +
      `placeholder key(s) not present in supplied substitutions\n`
    );
  }

  // Fallback: no workflow passed the placeholder filter — return first match to
  // avoid total breakage for malformed invocations (preserves existing behaviour)
  if (firstMatch !== null) {
    process.stderr.write(
      `preflight-gate: placeholder filter matched no workflow for phase "${phaseName}" — ` +
      `falling back to first match\n`
    );
    return firstMatch;
  }
  return null;
}

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

