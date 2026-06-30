'use strict';

// read-verdict.cjs — canonical verdict resolver. Reads structured store
// summaries (and task.status for the approve phase) instead of regex-parsing
// markdown.
//
// History: previously verdicts were extracted from the literal `**Verdict:**`
// line in markdown review artifacts (PLAN_REVIEW.md, CODE_REVIEW.md, …) via
// parse-verdict.cjs. That layer was fragile: smaller-model architects wrote
// "Approval Status: APPROVED" instead, halting the pipeline at phase 8
// (commit) of /forge:run-task. The structured signal was already present
// — `set-summary` writes `summaries.<canonical>.verdict ∈ {approved, revision, n/a}`
// validated against PHASE_SUMMARY_SCHEMA, and the approve phase transitions
// task.status to "approved". This module reads that structured signal.
//
// The literal `**Verdict:**` markdown line remains as a human-debuggable
// breadcrumb; it is no longer load-bearing for gating.

// Map a phase.role (as used by the orchestrator and `after <phase>` preflight
// predicates) to the canonical summaries key the store actually carries.
// Mirrors VALID_SUMMARY_PHASES in store-cli.cjs.
//
// Phases that produce no summaries entry (e.g. `approve` transitions
// task.status directly) map to a sentinel marker so callers can branch.
const STATUS_SOURCE = Object.freeze({ source: 'task.status' });

const PHASE_VERDICT_SOURCE = Object.freeze({
  plan:          'plan',
  'review-plan': 'review_plan',
  implement:     'implementation',
  'review-code': 'code_review',
  validate:      'validation',
  approve:       STATUS_SOURCE,    // task: reads task.status; bug: falls through to BUG map below
});

// Bug-specific verdict source map. Overrides PHASE_VERDICT_SOURCE for bug entities.
// After Fix 2, the approve summary key exists in bug.schema.json;
// approve reads from bug.summaries.approve.verdict (not bug.status).
const BUG_PHASE_VERDICT_SOURCE = Object.freeze({
  'plan-fix':    'plan',
  'review-plan': 'review_plan',
  implement:     'implementation',
  'review-code': 'code_review',
  approve:       'approve',       // read from bug.summaries.approve (not STATUS_SOURCE)
  commit:        null,            // commit is terminal; no verdict needed
});

// Allowed verdict values as written by set-summary (mirrors PHASE_SUMMARY_SCHEMA).
const ALLOWED_VERDICTS = new Set(['approved', 'revision', 'n/a']);

/**
 * Read a phase verdict for a given record.
 *
 * @param {Object} args
 * @param {Object} args.record   - Task or bug record (must carry .summaries and .status).
 * @param {string} args.phase    - Phase role (e.g. "review-plan", "review-code", "validate", "approve").
 * @param {string} [args.entityType] - Entity kind: 'task' or 'bug'. Auto-detected from record if omitted.
 * @returns {{ verdict: 'approved'|'revision'|'n/a'|null, source: string, key: string|null }}
 *
 * `verdict` is null when no signal is present (predecessor never wrote a summary,
 * or approve phase has not transitioned status). Callers treat null as "missing".
 *
 * `source` and `key` are diagnostic — useful in error messages so operators
 * know where the gate looked.
 */
function readVerdict({ record, phase, entityType }) {
  if (!record || typeof record !== 'object') {
    return { verdict: null, source: 'no-record', key: null };
  }

  // Auto-detect entity type from record shape if not provided.
  const detected = entityType || (record.bugId ? 'bug' : record.taskId ? 'task' : null);
  const isBug = detected === 'bug';

  // Select the correct verdict source map based on entity type.
  const sourceMap = isBug ? BUG_PHASE_VERDICT_SOURCE : PHASE_VERDICT_SOURCE;
  const spec = sourceMap[phase];
  if (spec === undefined) {
    // Phase not defined in this entity's map — try the other entity's map.
    const altSpec = isBug ? PHASE_VERDICT_SOURCE[phase] : BUG_PHASE_VERDICT_SOURCE[phase];
    if (altSpec !== undefined) {
      if (altSpec === null) return { verdict: null, source: 'not-applicable', key: null };
      if (altSpec === STATUS_SOURCE) {
        return { verdict: record.status === 'approved' ? 'approved' : null, source: `${detected || 'entity'}.status`, key: null };
      }
      const v = record.summaries && record.summaries[altSpec] && record.summaries[altSpec].verdict;
      return { verdict: ALLOWED_VERDICTS.has(v) ? v : null, source: 'summaries (alt-map)', key: altSpec };
    }
    // Defensive defaults: hyphen → underscore fallback.
    const fallbackKey = String(phase).replace(/-/g, '_');
    const v = record.summaries && record.summaries[fallbackKey] && record.summaries[fallbackKey].verdict;
    return {
      verdict: ALLOWED_VERDICTS.has(v) ? v : null,
      source: 'summaries (fallback)',
      key: fallbackKey,
    };
  }
  if (spec === null) {
    // Phase has no verdict signal (e.g. commit for bugs).
    return { verdict: null, source: 'not-applicable', key: null };
  }
  if (spec === STATUS_SOURCE) {
    // Approve-style phase for tasks: verdict source is task.status.
    return {
      verdict: record.status === 'approved' ? 'approved' : null,
      source: 'task.status',
      key: null,
    };
  }
  const summaries = record.summaries || {};
  const v = summaries[spec] && summaries[spec].verdict;
  return {
    verdict: ALLOWED_VERDICTS.has(v) ? v : null,
    source: 'summaries',
    key: spec,
  };
}

module.exports = { readVerdict, PHASE_VERDICT_SOURCE, BUG_PHASE_VERDICT_SOURCE, STATUS_SOURCE, ALLOWED_VERDICTS };

// CLI shim: `node read-verdict.cjs --task <taskId> --phase <phase>`
// stdout: "approved" | "revision" | "n/a" | "unknown"
// exit codes: 0 approved, 1 revision/missing, 2 invalid args / record not found
if (require.main === module) {
  const args = {};
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--task')  args.task  = argv[++i];
    else if (a === '--bug')  args.bug   = argv[++i];
    else if (a === '--phase') args.phase = argv[++i];
  }
  if (!args.phase || (!args.task && !args.bug)) {
    process.stderr.write('Usage: read-verdict.cjs --phase <phase> (--task <id> | --bug <id>)\n');
    process.exit(2);
  }
  const store = require('./store.cjs');
  let record;
  try {
    record = args.task ? store.getTask(args.task) : store.getBug(args.bug);
  } catch (err) {
    process.stderr.write(`read-verdict: cannot read record: ${err.message}\n`);
    process.exit(2);
  }
  if (!record) {
    process.stderr.write(`read-verdict: record not found for ${args.task ? 'task' : 'bug'} "${args.task || args.bug}"\n`);
    process.exit(2);
  }
  const out = readVerdict({ record, phase: args.phase, entityType: args.bug ? 'bug' : 'task' });
  process.stdout.write(`${out.verdict || 'unknown'}\n`);
  if (out.verdict === 'approved') process.exit(0);
  if (out.verdict === 'revision') process.exit(1);
  process.exit(1);
}
