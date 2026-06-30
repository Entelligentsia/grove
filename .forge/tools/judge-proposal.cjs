'use strict';
// FORGE-S24-T03 — LLM-judge step in Phase 2 (Sonnet rubric, drop <3/5).
//
// Pure module consumed by meta-enhance.md Phase 2 (step 5c) after the
// recurrence annotator (step 5a) and delete-candidate detector (step 5b)
// and BEFORE the artifact write (step 6).
//
// Two roles:
//
//   1. scoreProposal(proposal) — deterministic per-axis scorer. The workflow
//      instructs Sonnet to apply the same rubric and emit scores; this
//      module is both the fallback when Sonnet is unavailable AND the
//      reference implementation tests pin so the rubric definition stays
//      single-sourced. The five axes (RUBRIC_AXES) each carry a 0..5 score.
//
//   2. decideJudgement({ axes }) — pure aggregation. average < 3 -> drop;
//      otherwise keep. Reason names the average so the rejection log is
//      self-describing for retro review (AC3).
//
// The judge runs against every proposal pre-presentation (AC1). Dropped
// proposals are logged with per-axis scores (AC3) by the workflow caller.
//
// Iron Law 2: tests for this module landed first (see judge-proposal.test.cjs).

const RUBRIC_AXES = Object.freeze([
  'specificity',
  'when_not_to_use',
  'no_trajectory_copy_paste',
  'body_under_2kb',
  'cites_friction',
]);

const DROP_THRESHOLD = 3;             // avg < 3 -> drop (AC: drop < 3/5)
const MAX_BODY_BYTES = 2048;          // 2KB cap (AC: <=2KB body)
const TRAJECTORY_RUN_BYTES = 400;     // suspicious verbatim run length

// --- Per-axis scorers -----------------------------------------------------

function scoreSpecificity(proposal) {
  // Specificity heuristic: a specific proposal names a concrete artifact
  // path beyond the generic top-level skills directory AND has a non-empty
  // rationale OR a recurrence trail.
  const t = String(proposal.target_path || '');
  const r = String(proposal.rationale || '');
  const seg = t.split('/').filter(Boolean);
  const deepPath = seg.length >= 3;        // forge/skills/<name>.md is the floor
  const namedSkill =
    seg.length >= 1 &&
    !/^(misc|tips|notes|general)\b/i.test(seg[seg.length - 1] || '');
  const hasRationale = r.trim().length >= 16;
  const recurrent = Number(proposal.recurrence_count || 1) > 1;

  let s = 0;
  if (deepPath)      s += 2;
  if (namedSkill)    s += 1;
  if (hasRationale)  s += 1;
  if (recurrent)     s += 1;
  return clamp05(s);
}

function scoreWhenNotToUse(proposal) {
  // AC: rubric axis "When NOT to use" present. Phrase-based check. We accept
  // the canonical phrase or any reasonable case variation.
  const body = String(proposal.diff_body || '');
  return /when\s+not\s+to\s+use/i.test(body) ? 5 : 0;
}

function scoreNoTrajectoryCopyPaste(proposal) {
  // Heuristic: long verbatim runs of the same character or massive
  // unbroken whitespace-free blocks suggest pasted trajectory output.
  const body = String(proposal.diff_body || '');
  // Penalise long runs of the same character (>= TRAJECTORY_RUN_BYTES).
  const longRun = new RegExp('(.)\\1{' + (TRAJECTORY_RUN_BYTES - 1) + ',}');
  if (longRun.test(body)) return 0;
  // Penalise any unbroken non-whitespace block >= TRAJECTORY_RUN_BYTES.
  if (/\S{400,}/.test(body)) return 0;
  // Penalise lines that look like raw log entries dominating the body.
  const lines = body.split(/\n/);
  const longLines = lines.filter((l) => l.length >= 300).length;
  if (longLines >= 3) return 1;
  return 5;
}

function scoreBodyUnder2KB(proposal) {
  const body = String(proposal.diff_body || '');
  const bytes = Buffer.byteLength(body, 'utf8');
  return bytes <= MAX_BODY_BYTES ? 5 : 0;
}

function scoreCitesFriction(proposal) {
  const ids = Array.isArray(proposal.sourceFrictionIds)
    ? proposal.sourceFrictionIds
    : [];
  if (ids.length === 0) return 0;
  if (ids.length >= 3)  return 5;
  if (ids.length === 2) return 4;
  // Single citation: boost when recurrence trail shows the friction is real.
  const recurrent =
    Number(proposal.recurrence_count || 1) > 1 &&
    Array.isArray(proposal.recurrence_task_ids) &&
    proposal.recurrence_task_ids.length > 1;
  return recurrent ? 3 : 1;
}

// --- Aggregators ----------------------------------------------------------

function scoreProposal(proposal) {
  if (!proposal || typeof proposal !== 'object') {
    throw new TypeError('proposal must be an object');
  }

  const axes = {
    specificity:              scoreSpecificity(proposal),
    when_not_to_use:          scoreWhenNotToUse(proposal),
    no_trajectory_copy_paste: scoreNoTrajectoryCopyPaste(proposal),
    body_under_2kb:           scoreBodyUnder2KB(proposal),
    cites_friction:           scoreCitesFriction(proposal),
  };

  validateAxes(axes);

  const average = averageOf(axes);
  return { axes, average };
}

function decideJudgement({ axes }) {
  if (!axes || typeof axes !== 'object') {
    throw new TypeError('axes must be an object');
  }
  validateAxes(axes);
  const average = averageOf(axes);
  const verdict = average < DROP_THRESHOLD ? 'drop' : 'keep';
  const reason = verdict === 'drop'
    ? `dropped: rubric average ${average.toFixed(1)} < ${DROP_THRESHOLD}/5 (axes: ${formatAxes(axes)})`
    : `kept: rubric average ${average.toFixed(1)} >= ${DROP_THRESHOLD}/5 (axes: ${formatAxes(axes)})`;
  return { verdict, average, axes: { ...axes }, reason };
}

// --- Internal helpers -----------------------------------------------------

function validateAxes(axes) {
  for (const axis of RUBRIC_AXES) {
    if (!(axis in axes)) {
      throw new RangeError(`missing axis: ${axis}`);
    }
    const v = axes[axis];
    if (!Number.isFinite(v) || v < 0 || v > 5) {
      throw new RangeError(`axis ${axis} out of range [0,5]: ${v}`);
    }
  }
}

function averageOf(axes) {
  let sum = 0;
  for (const axis of RUBRIC_AXES) sum += axes[axis];
  // Round to one decimal place; rubric resolution is intentionally coarse so
  // the rejection log reads cleanly in retros.
  return Math.round((sum / RUBRIC_AXES.length) * 10) / 10;
}

function formatAxes(axes) {
  return RUBRIC_AXES.map((a) => `${a}=${axes[a]}`).join(', ');
}

function clamp05(n) {
  if (n < 0) return 0;
  if (n > 5) return 5;
  return n;
}

module.exports = {
  RUBRIC_AXES,
  DROP_THRESHOLD,
  MAX_BODY_BYTES,
  scoreProposal,
  decideJudgement,
};
