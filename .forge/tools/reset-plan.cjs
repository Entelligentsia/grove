#!/usr/bin/env node
// reset-plan.cjs — FEAT-009 T03: deterministic reset PLANNER + referential-
// integrity checker for /forge:reset.
//
// Given an entity (task | bug | sprint) and a target phase, this computes — but
// does NOT apply — the status transition(s) a reset would perform, plus the
// cross-entity integrity implications the operator must acknowledge before the
// LLM-backed /forge:reset command applies anything through store-cli.
//
// It NEVER mutates the store. Application is the command's job (store-cli
// update-status --force), gated on explicit confirmation of these warnings.
//
// Design: planReset(store, opts) is a pure function over the store facade so it
// is unit-testable with a mock store. The CLI wrapper loads the real store.

// Phase → required pre-status. Mirrors forge-cli's PHASE_PRE_STATUS /
// BUG_PHASE_PRE_STATUS (the two-layer boundary forbids a shared import); the
// reset-plan.test.cjs locks these against the documented workflow guards.
const TASK_PRE_STATUS = {
	plan: 'planned',
	'review-plan': 'planned',
	implement: 'plan-approved',
	'review-code': 'implemented',
	validate: 'review-approved',
	approve: 'review-approved',
	writeback: 'approved',
	commit: 'approved',
};
const BUG_PRE_STATUS = {
	triage: 'reported',
	'plan-fix': 'in-progress',
	'review-plan': 'in-progress',
	implement: 'in-progress',
	'review-code': 'in-progress',
	approve: 'in-progress',
	commit: 'in-progress',
};

// Lifecycle ordinals for rewind detection (higher = later).
const TASK_ORDER = {
	draft: 0,
	planned: 1,
	'plan-approved': 2,
	implementing: 3,
	implemented: 4,
	'review-approved': 5,
	approved: 6,
	committed: 7,
};
const BUG_ORDER = { reported: 0, triaged: 1, 'in-progress': 2, fixed: 3 };

function warn(code, msg, extra) {
	return Object.assign({ code, msg }, extra || {});
}

/** Tasks (and the sprint) that reference `id` in their dependencies. */
function findDependents(store, id) {
	const tasks = (store.listTasks() || []).filter(
		(t) => Array.isArray(t.dependencies) && t.dependencies.includes(id),
	);
	return tasks;
}

function planTask(store, id, to) {
	const task = store.getTask(id);
	if (!task) return { ok: false, error: `task ${id} not found` };
	const targetStatus = TASK_PRE_STATUS[to];
	if (!targetStatus) {
		return { ok: false, error: `unknown task phase '${to}'. Known: ${Object.keys(TASK_PRE_STATUS).join(', ')}` };
	}
	const from = task.status;
	const rewind = (TASK_ORDER[targetStatus] ?? 0) < (TASK_ORDER[from] ?? 0);
	const warnings = [];

	if (from === 'committed') {
		warnings.push(warn('committed-work', `Task ${id} is committed — its git commit is NOT reverted by this reset; the working tree may diverge from the rewound task state.`));
	}
	// Parent-sprint coherence.
	if (task.sprintId) {
		const sprint = store.getSprint(task.sprintId);
		if (sprint && ['completed', 'retrospective-done'].includes(sprint.status)) {
			warnings.push(warn('sprint-incoherent', `Parent sprint ${task.sprintId} is '${sprint.status}'; resetting a member task makes it inconsistent — reopen the sprint to 'active' as part of the reset.`, { sprintId: task.sprintId, sprintStatus: sprint.status }));
		}
	}
	// Dependents resting on rewound work.
	const dependents = findDependents(store, id).filter((d) => (TASK_ORDER[d.status] ?? 0) >= TASK_ORDER['plan-approved']);
	if (dependents.length > 0) {
		warnings.push(warn('dependents-affected', `${dependents.length} task(s) depend on ${id} and are already past planning; they rest on work being rewound and may need re-validation.`, { ids: dependents.map((d) => d.taskId) }));
	}

	return {
		ok: true,
		entity: { kind: 'task', id, currentStatus: from, targetStatus, targetPhase: to },
		rewind,
		forceRequired: rewind || from === 'blocked' || from === 'escalated',
		transitions: [{ id, kind: 'task', from, to: targetStatus }],
		warnings,
	};
}

function planBug(store, id, to) {
	const bug = store.getBug(id);
	if (!bug) return { ok: false, error: `bug ${id} not found` };
	const targetStatus = BUG_PRE_STATUS[to];
	if (!targetStatus) {
		return { ok: false, error: `unknown bug phase '${to}'. Known: ${Object.keys(BUG_PRE_STATUS).join(', ')}` };
	}
	const from = bug.status;
	const rewind = (BUG_ORDER[targetStatus] ?? 0) < (BUG_ORDER[from] ?? 0);
	const warnings = [];
	if (from === 'fixed') {
		warnings.push(warn('committed-work', `Bug ${id} is fixed — its commit is NOT reverted by this reset.`));
	}
	const dependents = findDependents(store, id).filter((d) => (TASK_ORDER[d.status] ?? 0) >= TASK_ORDER['plan-approved']);
	if (dependents.length > 0) {
		warnings.push(warn('dependents-affected', `${dependents.length} task(s) depend on ${id} and are past planning; they rest on work being rewound.`, { ids: dependents.map((d) => d.taskId) }));
	}
	return {
		ok: true,
		entity: { kind: 'bug', id, currentStatus: from, targetStatus, targetPhase: to },
		rewind,
		forceRequired: rewind,
		transitions: [{ id, kind: 'bug', from, to: targetStatus }],
		warnings,
	};
}

/**
 * Sprint reset: reopen the sprint and cascade-reset a member task plus every
 * member task that (transitively) depends on it — they build on the rewound
 * task. `fromTask` is required (where to rewind from).
 */
function planSprint(store, id, fromTask) {
	const sprint = store.getSprint(id);
	if (!sprint) return { ok: false, error: `sprint ${id} not found` };
	if (!fromTask) return { ok: false, error: `sprint reset requires --from-task <taskId> (the member task to rewind from)` };
	const members = (sprint.taskIds || []).map((tid) => store.getTask(tid)).filter(Boolean);
	const memberIds = new Set(members.map((t) => t.taskId));
	if (!memberIds.has(fromTask)) {
		return { ok: false, error: `task ${fromTask} is not a member of sprint ${id}` };
	}

	// Transitive dependents of fromTask within the sprint.
	const affected = new Set([fromTask]);
	let grew = true;
	while (grew) {
		grew = false;
		for (const t of members) {
			if (affected.has(t.taskId)) continue;
			if ((t.dependencies || []).some((dep) => affected.has(dep))) {
				affected.add(t.taskId);
				grew = true;
			}
		}
	}

	const cascade = members
		.filter((t) => affected.has(t.taskId))
		.map((t) => ({ id: t.taskId, from: t.status, to: 'planned' }));
	const warnings = [];
	const committed = cascade.filter((c) => c.from === 'committed').map((c) => c.id);
	if (committed.length) {
		warnings.push(warn('committed-work', `${committed.length} cascade task(s) are committed; their commits are NOT reverted.`, { ids: committed }));
	}
	if (['completed', 'retrospective-done'].includes(sprint.status)) {
		warnings.push(warn('sprint-reopen', `Sprint ${id} is '${sprint.status}' and will be reopened to 'active'.`));
	}
	return {
		ok: true,
		entity: { kind: 'sprint', id, currentStatus: sprint.status, fromTask },
		transitions: [
			{ id, kind: 'sprint', from: sprint.status, to: 'active' },
			...cascade.map((c) => ({ id: c.id, kind: 'task', from: c.from, to: c.to })),
		],
		cascade: cascade.map((c) => c.id),
		warnings,
	};
}

/** Pure planner over the store facade. */
function planReset(store, opts) {
	const { entity, id, to, fromTask } = opts;
	if (entity === 'task') return planTask(store, id, to);
	if (entity === 'bug') return planBug(store, id, to);
	if (entity === 'sprint') return planSprint(store, id, fromTask);
	return { ok: false, error: `unknown entity '${entity}' (expected task | bug | sprint)` };
}

module.exports = { planReset, TASK_PRE_STATUS, BUG_PRE_STATUS };

// ── CLI ─────────────────────────────────────────────────────────────────────
if (require.main === module) {
	const argv = process.argv.slice(2);
	const get = (flag) => {
		const i = argv.indexOf(flag);
		return i >= 0 ? argv[i + 1] : undefined;
	};
	const entity = get('--entity');
	const id = get('--id');
	const to = get('--to');
	const fromTask = get('--from-task');
	if (!entity || !id) {
		process.stderr.write('Usage: reset-plan.cjs --entity task|bug|sprint --id <id> [--to <phase>] [--from-task <taskId>] [--json]\n');
		process.exit(2);
	}
	const store = require('./store.cjs');
	const plan = planReset(store, { entity, id, to, fromTask });
	process.stdout.write(JSON.stringify(plan, null, 2) + '\n');
	process.exit(plan.ok ? 0 : 1);
}
