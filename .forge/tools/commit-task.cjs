'use strict';

// commit-task.cjs — deterministic commit choreography (forge-engineering#40).
//
// The commit phase was the most expensive phase of the SDLC pipeline
// (15–31% of run input tokens across instrumented runs) because an LLM
// re-derived deterministic choreography turn-by-turn: gate, state check,
// staging-set discovery, boundary verification, commit, terminal status
// transition. This tool owns the whole sequence; the agent supplies only the
// commit message (the single judgement-worthy step).
//
// Staging-set derivation (the commit boundary mirrors the task boundary):
//   1. record.path — the task/bug artifact directory (always staged)
//   2. record.summaries.implementation.files_changed — provenance recorded
//      by the implement phase (PHASE_SUMMARY_SCHEMA.files_changed)
//   3. --also <path> extras (each validated to stay inside the project root)
//
// Usage:
//   node commit-task.cjs --task <id> | --bug <id>
//        --message <msg>      commit subject/body (required unless --dry-run)
//        [--trailer <line>]   optional Co-authored-by trailer
//        [--also <path>]      extra path to stage (repeatable)
//        [--skip-gate]        skip preflight-gate (orchestrator already ran it)
//        [--force]            bypass the status precondition
//        [--dry-run]          print the staging plan as JSON; no writes
//
// Exit codes: 0 ok | 1 failure (message on stderr).

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { findProjectRoot } = require('./lib/project-root.cjs');

const ALLOWED_STATUS = { task: 'approved', bug: 'in-progress' };
const TERMINAL_STATUS = { task: 'committed', bug: 'fixed' };

function fail(msg) {
  console.error(`commit-task: ${msg}`);
  process.exit(1);
}

function parseArgs(argv) {
  const opts = { also: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case '--task': opts.entityKind = 'task'; opts.recordId = argv[++i]; break;
      case '--bug': opts.entityKind = 'bug'; opts.recordId = argv[++i]; break;
      case '--message': opts.message = argv[++i]; break;
      case '--trailer': opts.trailer = argv[++i]; break;
      case '--also': opts.also.push(argv[++i]); break;
      case '--skip-gate': opts.skipGate = true; break;
      case '--force': opts.force = true; break;
      case '--dry-run': opts.dryRun = true; break;
      default: fail(`unknown argument: ${a}`);
    }
  }
  if (!opts.entityKind || !opts.recordId) {
    fail('usage: commit-task.cjs --task <id> | --bug <id> --message <msg> [--trailer <line>] [--also <path>]... [--skip-gate] [--force] [--dry-run]');
  }
  if (!opts.dryRun && (!opts.message || !opts.message.trim())) {
    fail('--message is required (the commit message is the only input the agent supplies)');
  }
  return opts;
}

function git(root, args, { allowFail = false } = {}) {
  const r = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
  if (r.status !== 0 && !allowFail) {
    fail(`git ${args.join(' ')} failed: ${(r.stderr || r.stdout || '').trim()}`);
  }
  return r;
}

// Resolve a repo-relative path and reject anything escaping the project root.
function insideRoot(root, p) {
  const abs = path.resolve(root, p);
  const rel = path.relative(root, abs);
  if (rel.startsWith('..') || path.isAbsolute(rel)) return null;
  return rel;
}

function main() {
  const opts = parseArgs(process.argv.slice(2));
  const root = findProjectRoot();
  if (!root) fail('no .forge/config.json found above the current directory');

  // 1. Record read (store facade — never raw .forge/store writes).
  const store = require('./store.cjs');
  const record = opts.entityKind === 'task' ? store.getTask(opts.recordId) : store.getBug(opts.recordId);
  if (!record) fail(`${opts.entityKind} ${opts.recordId} not found in the store`);

  // 2. Status precondition (deterministic version of the Pipeline Step Guard).
  const allowed = ALLOWED_STATUS[opts.entityKind];
  if (record.status !== allowed && !opts.force) {
    const hint = opts.entityKind === 'task' ? "; /forge:approve must complete first" : '';
    fail(`× ${opts.recordId} is in state '${record.status}' — commit requires '${allowed}'${hint}. Use --force to bypass (operator-gated, user-invoked re-runs only).`);
  }

  // 3. Preflight gate (skippable when the orchestrator already ran it).
  if (!opts.skipGate) {
    const gate = spawnSync(process.execPath,
      [path.join(__dirname, 'preflight-gate.cjs'), '--phase', 'commit', `--${opts.entityKind}`, opts.recordId],
      { cwd: root, encoding: 'utf8' });
    if (gate.status !== 0) {
      fail(`preflight gate failed (exit ${gate.status}):\n${(gate.stderr || gate.stdout || '').trim()}`);
    }
  }

  // 4. Staging set: artifact dir + implementation provenance + --also extras.
  const stage = [];
  const warnings = [];
  if (record.path) {
    const rel = insideRoot(root, record.path);
    if (rel && fs.existsSync(path.join(root, rel))) stage.push(rel);
    else warnings.push(`artifact dir missing on disk: ${record.path}`);
  }
  const provenance = record.summaries?.implementation?.files_changed;
  if (Array.isArray(provenance) && provenance.length > 0) {
    for (const p of provenance) {
      const rel = insideRoot(root, p);
      if (rel === null) { warnings.push(`provenance path outside the project root, skipped: ${p}`); continue; }
      if (!fs.existsSync(path.join(root, rel))) {
        // Deleted files are legitimate changes — git add stages deletions of
        // tracked paths; only warn when git knows nothing about the path.
        const known = git(root, ['ls-files', '--error-unmatch', '--', rel], { allowFail: true });
        if (known.status !== 0) { warnings.push(`provenance path not found (skipped): ${p}`); continue; }
      }
      stage.push(rel);
    }
  } else {
    warnings.push('no files_changed provenance in summaries.implementation — staging the artifact dir only. Pass source files via --also if the task changed code.');
  }
  for (const p of opts.also) {
    const rel = insideRoot(root, p);
    if (rel === null) fail(`--also path outside the project root: ${p}`);
    stage.push(rel);
  }
  let stageSet = [...new Set(stage)];
  if (stageSet.length === 0) fail('nothing to stage — no artifact dir, no provenance, no --also paths');

  // Pre-filter gitignored paths (live-run finding, forge-engineering#40):
  // `git add` of the whole set is all-or-nothing — one ignored path aborts
  // everything. check-ignore each path; warn-skip the ignored ones.
  const ignored = [];
  for (const rel of stageSet) {
    const ci = git(root, ['check-ignore', '-q', '--', rel], { allowFail: true });
    if (ci.status === 0) ignored.push(rel);
  }
  if (ignored.length > 0) {
    warnings.push(`gitignored path(s) skipped from the staging set: ${ignored.join(', ')} — track them or stage explicitly with git add -f outside this tool.`);
    stageSet = stageSet.filter((p) => !ignored.includes(p));
  }

  for (const w of warnings) console.error(`commit-task: warning: ${w}`);

  if (opts.dryRun) {
    process.stdout.write(JSON.stringify({ dryRun: true, entityKind: opts.entityKind, recordId: opts.recordId, stage: stageSet }, null, 2) + '\n');
    return;
  }

  // 5. Commit-boundary guard: a pre-populated index means someone else's
  //    changes would be swept into this commit. Abort loudly (Iron Law:
  //    the commit boundary mirrors the task boundary).
  const preStaged = git(root, ['diff', '--cached', '--name-only']).stdout.trim();
  if (preStaged) {
    fail(`index already has staged changes — refusing to sweep them into the ${opts.recordId} commit:\n${preStaged}\nUnstage them (git reset) or commit them separately, then re-run.`);
  }

  // 6. Terminal status transition helper — through store-cli (single source
  //    of truth for transition legality — never reimplemented here).
  const target = TERMINAL_STATUS[opts.entityKind];
  const transition = (context) => {
    const updArgs = [path.join(__dirname, 'store-cli.cjs'), 'update-status', opts.entityKind, opts.recordId, 'status', target];
    if (opts.force) updArgs.push('--force'); // --force bypasses the transition map too (user-invoked re-runs)
    const upd = spawnSync(process.execPath, updArgs, { cwd: root, encoding: 'utf8' });
    if (upd.status !== 0) {
      fail(`${context} but status transition to '${target}' failed:\n${(upd.stderr || upd.stdout || '').trim()}`);
    }
  };

  // 7. Stage exactly the derived set. A clean staging set is a legitimate
  //    terminal state (e.g. a bug whose fix is already at HEAD): no commit,
  //    but the record is still sealed (live-run finding, forge-engineering#40).
  const noop = (why) => {
    console.error(`commit-task: ${why} — nothing to commit; sealing the record without a commit.`);
    transition('no-op commit');
    process.stdout.write(JSON.stringify(
      { ok: true, committed: false, reason: 'nothing-to-commit', skippedIgnored: ignored, status: target }, null, 2) + '\n');
  };
  if (stageSet.length === 0) {
    noop('entire staging set is gitignored');
    return;
  }
  git(root, ['add', '--', ...stageSet]);
  const staged = git(root, ['diff', '--cached', '--name-only']).stdout.trim().split('\n').filter(Boolean);
  if (staged.length === 0) {
    noop('working tree already clean for the staging set');
    return;
  }

  // 8. Commit. Message comes from the agent; the optional trailer is appended
  //    after a blank line per git convention.
  const message = opts.trailer ? `${opts.message.trim()}\n\n${opts.trailer.trim()}\n` : `${opts.message.trim()}\n`;
  git(root, ['commit', '-m', message]);
  const sha = git(root, ['rev-parse', 'HEAD']).stdout.trim();

  transition(`commit ${sha} created`);

  process.stdout.write(JSON.stringify({ ok: true, committed: true, sha, staged, status: target }, null, 2) + '\n');
}

try {
  main();
} catch (err) {
  console.error(`commit-task: ${err.message}`);
  process.exit(1);
}
