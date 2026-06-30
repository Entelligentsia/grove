'use strict';

// ── Canonical artifact-kind registry ─────────────────────────────────────────
//
// Single source of truth for artifact filenames, bug-mode filename overrides,
// and the phase→kind mapping. Extracted from artifact.cjs per ADR
// `doc/decisions/artifact-resolution-abstraction.md` (Phase 1) so that
// artifact.cjs, store-cli.cjs (set-summary / set-bug-summary), and the
// forge-cli `forge_artifact` surface all consume ONE catalog instead of
// maintaining parallel copies.
//
// Renaming an artifact file is a one-line edit here.

const ARTIFACT_CATALOG = {
  'plan':                   { filename: 'PLAN.md',                    type: 'md' },
  'plan-review':            { filename: 'PLAN_REVIEW.md',             type: 'md' },
  'progress':               { filename: 'PROGRESS.md',                type: 'md' },
  'code-review':            { filename: 'CODE_REVIEW.md',             type: 'md' },
  'validation-report':      { filename: 'VALIDATION_REPORT.md',       type: 'md' },
  'architect-approval':     { filename: 'ARCHITECT_APPROVAL.md',      type: 'md' },
  'triage':                 { filename: 'TRIAGE.md',                  type: 'md' },
  'bug-report':             { filename: 'BUG_REPORT.md',              type: 'md' },
  'index':                  { filename: 'INDEX.md',                   type: 'md' },
  'task-prompt':            { filename: 'TASK_PROMPT.md',             type: 'md' },
  'sprint-requirements':    { filename: 'SPRINT_REQUIREMENTS.md',     type: 'md' },
  'sprint-completion-review': { filename: 'SPRINT_COMPLETION_REVIEW.md', type: 'md' },
  'cost-report':            { filename: 'COST_REPORT.md',             type: 'md' },
  'timesheet':              { filename: 'TIMESHEET.md',               type: 'md' },
  'plan-summary':           { filename: 'PLAN-SUMMARY.json',          type: 'json' },
  'review-plan-summary':    { filename: 'REVIEW-PLAN-SUMMARY.json',   type: 'json' },
  'implementation-summary': { filename: 'IMPLEMENTATION-SUMMARY.json', type: 'json' },
  'review-code-summary':    { filename: 'REVIEW-CODE-SUMMARY.json',   type: 'json' },
  'review-impl-summary':    { filename: 'REVIEW-IMPL-SUMMARY.json',   type: 'json' },
  'validation-summary':     { filename: 'VALIDATION-SUMMARY.json',    type: 'json' },
  'approve-summary':        { filename: 'APPROVE-SUMMARY.json',       type: 'json' },
  'commit-summary':         { filename: 'COMMIT-SUMMARY.json',        type: 'json' },
  'triage-summary':         { filename: 'TRIAGE-SUMMARY.json',        type: 'json' },
  'writeback-summary':      { filename: 'WRITEBACK-SUMMARY.json',     type: 'json' },
  'collation-summary':      { filename: 'COLLATION-SUMMARY.json',     type: 'json' },
};

// Per-entity filename overrides. Bug-mode plans and plan-summaries use the
// BUG_FIX_PLAN prefix to match the long-standing forge convention and the
// preflight-gate.cjs expectations for review-plan in bug mode. Without this
// override, plan-fix (routed via plan_task.md post FORGE-BUG-040) writes
// PLAN.md and review-plan preflight then fails "artifact missing:
// BUG_FIX_PLAN.md" — see FORGE-BUG-041.
const ARTIFACT_FILENAME_OVERRIDES = {
  bug: {
    'plan':         'BUG_FIX_PLAN.md',
    'plan-summary': 'BUG-FIX-PLAN-SUMMARY.json',
  },
};

// Phase → artifact-kind. Phases use underscore spellings (mirror of
// store-helpers VALID_SUMMARY_PHASES); kinds use the hyphenated catalog keys.
// This is the mapping set-summary / set-bug-summary use to self-resolve the
// sidecar JSON file from a record's path, so callers never hand-build paths.
const PHASE_TO_KIND = {
  'plan':           'plan-summary',
  'review_plan':    'review-plan-summary',
  'implementation': 'implementation-summary',
  'code_review':    'review-code-summary',
  'validation':     'validation-summary',
  'triage':         'triage-summary',
  'approve':        'approve-summary',
};

function resolveArtifactFilename(entity, artifactName) {
  const override = ARTIFACT_FILENAME_OVERRIDES[entity];
  if (override && override[artifactName]) return override[artifactName];
  const entry = ARTIFACT_CATALOG[artifactName];
  if (!entry) throw new Error(`unknown artifact kind: ${artifactName}`);
  return entry.filename;
}

// Resolve the phase-summary sidecar filename for an entity. Applies the
// phase→kind map, then the per-entity filename override (so a bug's "plan"
// summary becomes BUG-FIX-PLAN-SUMMARY.json).
function resolveSummaryFilename(entity, phase) {
  const kind = PHASE_TO_KIND[phase];
  if (!kind) throw new Error(`unknown phase: ${phase}`);
  return resolveArtifactFilename(entity, kind);
}

const ARTIFACT_NAMES = Object.keys(ARTIFACT_CATALOG).sort();

module.exports = {
  ARTIFACT_CATALOG,
  ARTIFACT_FILENAME_OVERRIDES,
  ARTIFACT_NAMES,
  PHASE_TO_KIND,
  resolveArtifactFilename,
  resolveSummaryFilename,
};
