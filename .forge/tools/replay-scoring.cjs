'use strict';
// FORGE-S24-T04 — cross-task replay scoring (recurrence boost).
//
// For each enhancement proposal synthesised from a friction event at task `t`,
// scan friction events in tasks `t+1..N` (within the same sprint) for the
// same `(subkind, evidence.skillId)` pair. Surface the count + task list on
// the proposal so the T03 judge sees "this friction happened 3 times" rather
// than "this friction happened once".
//
// Pure module; consumed by Phase 2 step 5/6 of meta-enhance.md after the
// dedup pass and before the proposal artifact is written.
//
// Exports:
//   computeRecurrence({ events, subkind, skillId, fromTaskId, taskOrder })
//     -> { recurrence_count, recurrence_task_ids }
//   annotateProposals(proposals, frictionEvents, taskOrder)
//     -> new array of proposals, each carrying recurrence_count +
//        recurrence_task_ids.

function computeRecurrence({ events, subkind, skillId, fromTaskId, taskOrder }) {
  if (!Array.isArray(events))    throw new TypeError('events must be an array');
  if (!Array.isArray(taskOrder)) throw new TypeError('taskOrder must be an array');
  if (typeof subkind    !== 'string' || subkind    === '') throw new TypeError('subkind required');
  if (typeof skillId    !== 'string' || skillId    === '') throw new TypeError('skillId required');
  if (typeof fromTaskId !== 'string' || fromTaskId === '') throw new TypeError('fromTaskId required');

  const originIdx = taskOrder.indexOf(fromTaskId);

  // Distinct taskIds with a matching friction event.
  const matchingTaskIds = new Set();

  for (const evt of events) {
    if (!evt || evt.type !== 'friction') continue;
    if (evt.subkind !== subkind) continue;
    const evtSkillId = evt.evidence && evt.evidence.skillId;
    if (evtSkillId !== skillId) continue;
    if (!evt.taskId) continue;

    if (originIdx === -1) {
      // fromTaskId not in taskOrder: only count the origin task itself.
      if (evt.taskId === fromTaskId) matchingTaskIds.add(evt.taskId);
      continue;
    }

    const evtIdx = taskOrder.indexOf(evt.taskId);
    // Forward-only: include origin task (evtIdx === originIdx) and later
    // (evtIdx > originIdx). Skip earlier and unknown tasks.
    if (evtIdx >= originIdx) matchingTaskIds.add(evt.taskId);
  }

  // Always include the origin task so recurrence_count >= 1.
  matchingTaskIds.add(fromTaskId);

  // Sort by taskOrder position; tasks not in taskOrder go last in insertion order.
  const ordered = [...matchingTaskIds].sort((a, b) => {
    const ai = taskOrder.indexOf(a);
    const bi = taskOrder.indexOf(b);
    if (ai === -1 && bi === -1) return 0;
    if (ai === -1) return 1;
    if (bi === -1) return -1;
    return ai - bi;
  });

  return {
    recurrence_count:    ordered.length,
    recurrence_task_ids: ordered,
  };
}

function annotateProposals(proposals, frictionEvents, taskOrder) {
  if (!Array.isArray(proposals))      throw new TypeError('proposals must be an array');
  if (!Array.isArray(frictionEvents)) throw new TypeError('frictionEvents must be an array');
  if (!Array.isArray(taskOrder))      throw new TypeError('taskOrder must be an array');

  // Index friction events by eventId for O(1) sourceFrictionIds resolution.
  const byEventId = new Map();
  for (const evt of frictionEvents) {
    if (evt && evt.eventId) byEventId.set(evt.eventId, evt);
  }

  return proposals.map((proposal) => {
    const sourceIds = Array.isArray(proposal.sourceFrictionIds)
      ? proposal.sourceFrictionIds
      : [];

    // Resolve the first sourceFrictionId that points at a known friction event
    // with subkind + evidence.skillId. The originating event determines
    // (subkind, skillId, fromTaskId) for the recurrence scan.
    let originEvent = null;
    for (const id of sourceIds) {
      const evt = byEventId.get(id);
      if (evt && evt.subkind && evt.evidence && evt.evidence.skillId && evt.taskId) {
        originEvent = evt;
        break;
      }
    }

    if (!originEvent) {
      // No resolvable provenance: neutral annotation. recurrence_count=1
      // keeps the schema invariant; recurrence_task_ids is empty so the
      // judge can distinguish "single observation" from "unresolved origin".
      return { ...proposal, recurrence_count: 1, recurrence_task_ids: [] };
    }

    const { recurrence_count, recurrence_task_ids } = computeRecurrence({
      events:     frictionEvents,
      subkind:    originEvent.subkind,
      skillId:    originEvent.evidence.skillId,
      fromTaskId: originEvent.taskId,
      taskOrder,
    });

    return { ...proposal, recurrence_count, recurrence_task_ids };
  });
}

module.exports = { computeRecurrence, annotateProposals };
