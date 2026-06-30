'use strict';
// FORGE-S24-T05 — delete-candidate detection (3-sprint zero-use).
//
// Scan `skill_usage` events across the trailing `windowSize` sprints of
// `sprintOrder`. Any skill that has at least one observation inside the
// window AND zero retrievals AND zero invocations across every in-window
// observation is a delete candidate. The detector emits one `delete_skill`
// proposal per qualifying skill.
//
// Pure module; consumed by Phase 2 of meta-enhance.md (between the
// recurrence-annotation step and the proposal-artifact write step).
//
// Carry-over caveat (documented in the workflow): the window only becomes
// meaningful once `windowSize` sprints have actually elapsed since
// skill_usage emission landed in T01. Until then, the function still runs
// over the available sprints but the signal is noisier.
//
// We deliberately do NOT propose deletion for skills with zero observations
// in the window — that case is indistinguishable from a brand-new skill
// that simply hasn't been loaded yet. The detector only deletes what it
// has actually seen go cold.
//
// Exports:
//   scanZeroUse({ events, sprintOrder, windowSize = 3 })
//     -> [{ skillId, observedSprintIds }]
//   buildDeleteProposals({ events, sprintOrder, windowSize = 3, targetPathFor })
//     -> [proposal] — proposal shape conforming to proposal.schema.json.

const DEFAULT_WINDOW_SIZE = 3;

function scanZeroUse({ events, sprintOrder, windowSize = DEFAULT_WINDOW_SIZE }) {
  if (!Array.isArray(events))      throw new TypeError('events must be an array');
  if (!Array.isArray(sprintOrder)) throw new TypeError('sprintOrder must be an array');
  if (!Number.isInteger(windowSize) || windowSize < 1) {
    throw new TypeError('windowSize must be a positive integer');
  }

  // Trailing N sprints. If sprintOrder is shorter than the window, use all of it.
  const window = sprintOrder.slice(-windowSize);
  const windowSet = new Set(window);

  // skillId -> { observedSprintIds: Set, anyRetrieved: bool, anyUsed: bool }
  const bySkill = new Map();

  for (const evt of events) {
    if (!evt || evt.type !== 'skill_usage') continue;
    if (typeof evt.skillId !== 'string' || evt.skillId === '') continue;
    if (typeof evt.sprintId !== 'string' || evt.sprintId === '') continue;
    if (!windowSet.has(evt.sprintId)) continue;

    let agg = bySkill.get(evt.skillId);
    if (!agg) {
      agg = { observedSprintIds: new Set(), anyRetrieved: false, anyUsed: false };
      bySkill.set(evt.skillId, agg);
    }
    agg.observedSprintIds.add(evt.sprintId);
    if (evt.retrieved === true) agg.anyRetrieved = true;
    if (evt.used      === true) agg.anyUsed      = true;
  }

  const results = [];
  for (const [skillId, agg] of bySkill) {
    if (agg.anyRetrieved || agg.anyUsed) continue;
    if (agg.observedSprintIds.size === 0)   continue;

    // Sort observedSprintIds by window order so output is deterministic and
    // mirrors sprintOrder.
    const ordered = window.filter(s => agg.observedSprintIds.has(s));
    results.push({ skillId, observedSprintIds: ordered });
  }

  // Deterministic skill ordering.
  results.sort((a, b) => a.skillId.localeCompare(b.skillId));
  return results;
}

function buildDeleteProposals({
  events,
  sprintOrder,
  windowSize = DEFAULT_WINDOW_SIZE,
  targetPathFor,
}) {
  if (typeof targetPathFor !== 'function') {
    throw new TypeError('targetPathFor must be a function');
  }

  const candidates = scanZeroUse({ events, sprintOrder, windowSize });
  const window = sprintOrder.slice(-windowSize);

  return candidates.map(({ skillId, observedSprintIds }) => ({
    op:          'delete_skill',
    target_path: targetPathFor(skillId),
    diff_body:
      `- entire skill removed: ${skillId}\n` +
      `- reason: zero retrieval AND zero invocation across ` +
      `${observedSprintIds.length} observed sprint(s) within trailing ` +
      `${windowSize}-sprint window (${window.join(', ')})`,
    rationale:
      `Skill "${skillId}" has no retrieval and no invocation in any of the ` +
      `trailing ${windowSize} sprints it was observed in (${observedSprintIds.join(', ')}). ` +
      `Delete-candidate per FORGE-S24-T05 detector.`,
    sourceFrictionIds:   [],
    window_size:         windowSize,
    window_sprint_ids:   observedSprintIds.slice(),
    recurrence_count:    1,
    recurrence_task_ids: [],
  }));
}

module.exports = {
  scanZeroUse,
  buildDeleteProposals,
  DEFAULT_WINDOW_SIZE,
};
