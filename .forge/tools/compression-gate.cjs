'use strict';
// FORGE-S24-T06 — compression gate (reject >20% growth without 3+ frictions).
//
// Phase 2 of meta-enhance runs this gate BEFORE the LLM judge (T03). The gate
// is a cheap deterministic filter: an `update_skill` proposal that grows the
// target file by more than 20% (byte-wise) must be backed by at least 3
// supporting friction events. Insufficient support → reject; the judge never
// sees the proposal.
//
// Why a cheap pre-judge gate?
//   - Judging is expensive (LLM call).
//   - Unbounded growth of a skill body is the classic SkillOS failure mode —
//     adding pages of trajectory copy-paste to "patch" a friction. Cheap to
//     detect deterministically; wasteful to ask the judge to rule on.
//
// Op semantics:
//   - `insert_skill`: not gated — the file doesn't exist yet, "growth" is
//     undefined; the judge's body_under_2kb axis handles bloat instead.
//   - `delete_skill`: not gated — deletion always shrinks.
//   - `update_skill`: GATED. Growth is measured byte-wise (UTF-8) on the new
//     body that would land after applying the diff. The caller resolves the
//     "currentBody" and "newBody" by reading the file and applying the patch.
//
// Friction support:
//   - Default: `proposal.sourceFrictionIds.length`.
//   - Override: caller may supply `supportingFrictionCountFor(proposal) -> int`
//     when the policy is "count frictions citing the same skill across the
//     sprint" rather than "count citations on the proposal itself".
//
// Pure module: no fs access, no LLM call. Consumed by Phase 2 between
// step 5b (delete-candidate detection) and step 5c (LLM-judge gate).
//
// Exports:
//   GROWTH_THRESHOLD                       — 0.20 (strict >, ties admit).
//   MIN_SUPPORTING_FRICTIONS               — 3.
//   evaluateGrowth({ currentBody, newBody })
//     -> { currentBytes, newBytes, growthRatio }
//   evaluateProposal({ proposal, currentBody, newBody, supportingFrictionCount })
//     -> { admit, reason, growthRatio, currentBytes, newBytes,
//          supportingFrictionCount, threshold, minSupportingFrictions, op }
//   filterProposals({ proposals, currentBodyFor, newBodyFor,
//                     supportingFrictionCountFor })
//     -> { admitted: proposal[], rejected: { proposal, ...evaluation }[] }

const GROWTH_THRESHOLD         = 0.20;
const MIN_SUPPORTING_FRICTIONS = 3;

const VALID_OPS = new Set(['insert_skill', 'update_skill', 'delete_skill']);

function evaluateGrowth({ currentBody, newBody }) {
  if (typeof currentBody !== 'string') {
    throw new TypeError('currentBody must be a string');
  }
  if (typeof newBody !== 'string') {
    throw new TypeError('newBody must be a string');
  }
  const currentBytes = Buffer.byteLength(currentBody, 'utf8');
  const newBytes     = Buffer.byteLength(newBody,     'utf8');
  // When currentBytes === 0 the ratio is undefined for update; we return
  // Infinity and let the caller decide. evaluateProposal treats Infinity as
  // "over threshold" — an update on an empty file is, by definition,
  // unbounded growth — so the friction-support gate still applies.
  const growthRatio = currentBytes === 0
    ? (newBytes === 0 ? 0 : Infinity)
    : (newBytes - currentBytes) / currentBytes;
  return { currentBytes, newBytes, growthRatio };
}

function evaluateProposal({
  proposal,
  currentBody,
  newBody,
  supportingFrictionCount,
}) {
  if (!proposal || typeof proposal !== 'object') {
    throw new TypeError('proposal must be an object');
  }
  if (typeof proposal.op !== 'string' || !VALID_OPS.has(proposal.op)) {
    throw new TypeError(
      `proposal.op must be one of ${Array.from(VALID_OPS).join(', ')}; got ${JSON.stringify(proposal.op)}`,
    );
  }
  if (typeof currentBody !== 'string') {
    throw new TypeError('currentBody must be a string');
  }
  if (typeof newBody !== 'string') {
    throw new TypeError('newBody must be a string');
  }

  // Resolve supporting friction count — explicit > proposal.sourceFrictionIds.
  let frictionCount;
  if (supportingFrictionCount === undefined) {
    frictionCount = Array.isArray(proposal.sourceFrictionIds)
      ? proposal.sourceFrictionIds.length
      : 0;
  } else {
    if (!Number.isInteger(supportingFrictionCount) || supportingFrictionCount < 0) {
      throw new TypeError(
        'supportingFrictionCount must be a non-negative integer',
      );
    }
    frictionCount = supportingFrictionCount;
  }

  const { currentBytes, newBytes, growthRatio } = evaluateGrowth({
    currentBody, newBody,
  });

  const base = {
    growthRatio,
    currentBytes,
    newBytes,
    supportingFrictionCount: frictionCount,
    threshold:               GROWTH_THRESHOLD,
    minSupportingFrictions:  MIN_SUPPORTING_FRICTIONS,
    op:                      proposal.op,
  };

  // Non-update ops are not gated.
  if (proposal.op !== 'update_skill') {
    return { admit: true, reason: 'op_not_gated', ...base };
  }

  // Strict >: growth exactly at threshold admits.
  if (!(growthRatio > GROWTH_THRESHOLD)) {
    return { admit: true, reason: 'admitted_below_threshold', ...base };
  }

  // Over threshold — admit only if enough friction support.
  if (frictionCount >= MIN_SUPPORTING_FRICTIONS) {
    return { admit: true, reason: 'admitted_with_friction_support', ...base };
  }

  return { admit: false, reason: 'compression_gate_growth_unsupported', ...base };
}

function filterProposals({
  proposals,
  currentBodyFor,
  newBodyFor,
  supportingFrictionCountFor,
}) {
  if (!Array.isArray(proposals)) {
    throw new TypeError('proposals must be an array');
  }
  if (typeof currentBodyFor !== 'function') {
    throw new TypeError('currentBodyFor must be a function');
  }
  if (typeof newBodyFor !== 'function') {
    throw new TypeError('newBodyFor must be a function');
  }
  if (
    supportingFrictionCountFor !== undefined &&
    typeof supportingFrictionCountFor !== 'function'
  ) {
    throw new TypeError('supportingFrictionCountFor must be a function when provided');
  }

  const admitted = [];
  const rejected = [];

  for (const proposal of proposals) {
    const currentBody = currentBodyFor(proposal);
    const newBody     = newBodyFor(proposal);
    const supportingFrictionCount = supportingFrictionCountFor
      ? supportingFrictionCountFor(proposal)
      : undefined;

    const evaluation = evaluateProposal({
      proposal,
      currentBody,
      newBody,
      supportingFrictionCount,
    });

    if (evaluation.admit) {
      admitted.push(proposal);
    } else {
      rejected.push({ proposal, ...evaluation });
    }
  }

  return { admitted, rejected };
}

module.exports = {
  GROWTH_THRESHOLD,
  MIN_SUPPORTING_FRICTIONS,
  evaluateGrowth,
  evaluateProposal,
  filterProposals,
};
