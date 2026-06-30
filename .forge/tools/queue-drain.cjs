'use strict';
// FORGE-S24-T07 — queue drain at sprint close (per-task curator → batched review).
//
// Paper §3.2.1 grouped reward: one batched review at sprint close, not one
// prompt per task. Per-task curators (T10) append proposals to a project-local
// queue as they encounter friction during the sprint; Phase 2 drains the queue,
// dedupes by {op, target_path, body-hash}, and feeds a single batch into the
// downstream pipeline (compression gate T06 → replay scoring T04 → judge T03).
//
// Queue layout (canonical):
//   .forge/enhancement-proposals/queue/<sprintId>/<taskId>-<ts>.json
//
//   - One file per per-task curator run.
//   - The `<ts>` suffix (ISO compact timestamp) means concurrent or repeated
//     curator runs on the same task never collide; each run produces a fresh
//     file. Files are append-only — once written, never overwritten. The drain
//     is a read-only pass that does NOT delete the queue (operators triage
//     queue files for retrospective debugging).
//
// Dedup key: `<op>|<target_path>|sha256(diff_body)`.
//   - `op` and `target_path` together identify "what artifact and how" (insert
//     vs update vs delete).
//   - `sha256(diff_body)` makes two proposals with byte-identical diff bodies
//     collapse to one even if their rationales or sourceFrictionIds differ.
//     Different bodies → different proposals (a smaller surgical patch and a
//     full-file rewrite are distinct decisions for the judge).
//
// Pure helpers (`bodyHash`, `dedupeKey`, `dedupeProposals`, `queuePathFor`) are
// fs-free. `appendToQueue` and `drainQueue` touch the filesystem.
//
// Exports:
//   bodyHash(body)                    → sha256 hex digest of body (UTF-8).
//   dedupeKey(proposal)               → composite key string.
//   dedupeProposals(proposals)        → first-seen-wins deduped array.
//   queuePathFor({ queueRoot, sprintId, taskId, ts })
//                                     → canonical file path (string).
//   appendToQueue({ queueRoot, sprintId, taskId, ts, proposals })
//                                     → absolute path written; throws if path
//                                       already exists (append-only invariant).
//   drainQueue({ queueRoot, sprintId })
//                                     → { proposals: [...deduped], files: [...],
//                                         errors: [...] } — fs read, never write.

const fs     = require('node:fs');
const path   = require('node:path');
const crypto = require('node:crypto');

function bodyHash(body) {
  if (typeof body !== 'string') {
    throw new TypeError('bodyHash: body must be a string');
  }
  return crypto.createHash('sha256').update(body, 'utf8').digest('hex');
}

function dedupeKey(proposal) {
  if (!proposal || typeof proposal !== 'object') {
    throw new TypeError('dedupeKey: proposal must be an object');
  }
  const op          = String(proposal.op          ?? '');
  const target_path = String(proposal.target_path ?? '');
  const diff_body   = String(proposal.diff_body   ?? '');
  return `${op}|${target_path}|${bodyHash(diff_body)}`;
}

function dedupeProposals(proposals) {
  if (!Array.isArray(proposals)) {
    throw new TypeError('dedupeProposals: proposals must be an array');
  }
  const seen = new Set();
  const out  = [];
  for (const p of proposals) {
    const k = dedupeKey(p);
    if (seen.has(k)) continue;
    seen.add(k);
    out.push(p);
  }
  return out;
}

function queuePathFor({ queueRoot, sprintId, taskId, ts } = {}) {
  if (typeof sprintId !== 'string' || !sprintId) {
    throw new TypeError('queuePathFor: sprintId is required');
  }
  if (typeof taskId !== 'string' || !taskId) {
    throw new TypeError('queuePathFor: taskId is required');
  }
  if (typeof ts !== 'string' || !ts) {
    throw new TypeError('queuePathFor: ts is required');
  }
  const root = queueRoot ?? '.forge/enhancement-proposals/queue';
  return path.join(root, sprintId, `${taskId}-${ts}.json`);
}

function appendToQueue({ queueRoot, sprintId, taskId, ts, proposals } = {}) {
  if (!Array.isArray(proposals)) {
    throw new TypeError('appendToQueue: proposals must be an array');
  }
  const target = queuePathFor({ queueRoot, sprintId, taskId, ts });
  if (fs.existsSync(target)) {
    throw new Error(
      `appendToQueue: queue file already exists at ${target} ` +
      `(queue is append-only; choose a fresh ts).`,
    );
  }
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, JSON.stringify(proposals, null, 2), 'utf8');
  return target;
}

function drainQueue({ queueRoot, sprintId } = {}) {
  if (typeof sprintId !== 'string' || !sprintId) {
    throw new TypeError('drainQueue: sprintId is required');
  }
  if (typeof queueRoot !== 'string' || !queueRoot) {
    throw new TypeError('drainQueue: queueRoot is required');
  }
  const sprintDir = path.join(queueRoot, sprintId);
  const result = { proposals: [], files: [], errors: [] };
  if (!fs.existsSync(sprintDir)) return result;

  const entries = fs.readdirSync(sprintDir)
    .filter((name) => name.endsWith('.json'))
    .sort(); // deterministic order: lexicographic ⇒ chronological because of ts suffix
  const merged = [];
  for (const name of entries) {
    const abs = path.join(sprintDir, name);
    result.files.push(abs);
    let parsed;
    try {
      parsed = JSON.parse(fs.readFileSync(abs, 'utf8'));
    } catch (err) {
      result.errors.push({ file: abs, error: err.message });
      continue;
    }
    if (!Array.isArray(parsed)) {
      result.errors.push({ file: abs, error: 'expected JSON array of proposals' });
      continue;
    }
    for (const p of parsed) merged.push(p);
  }
  result.proposals = dedupeProposals(merged);
  return result;
}

module.exports = {
  bodyHash,
  dedupeKey,
  dedupeProposals,
  queuePathFor,
  appendToQueue,
  drainQueue,
};
