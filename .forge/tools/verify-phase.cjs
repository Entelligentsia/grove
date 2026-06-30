'use strict';

// verify-phase.cjs — shared init phase verification tool (FORGE-S26-T17)
//
// CLI interface:
//   node verify-phase.cjs --phase 1
//   node verify-phase.cjs --phase 1 --foundation-only
//   node verify-phase.cjs --phase 2 --kb-path <kbPath>
//   node verify-phase.cjs --phase 3
//
// Exit codes:
//   0 — all checks pass
//   1 — one or more checks failed; JSON written to stdout
//   2 — bad arguments; usage written to stderr
//
// Output JSON (exit 1) shape:
// {
//   "phase": <1|2|3>,
//   "ok": false,
//   "missing": ["<item>", ...],
//   "reason": "<optional human-readable reason>",
//   "checked": ["<field>", ...]
// }
//
// Uses process.cwd() as project root — consistent with all other Forge tools.

const fs = require('node:fs');
const path = require('node:path');

// ── Argument parsing ─────────────────────────────────────────────────────────

function parseArgs(argv) {
  const args = argv.slice(2);
  let phase = null;
  let foundationOnly = false;
  let kbPath = null;

  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === '--phase') {
      const val = args[++i];
      const n = parseInt(val, 10);
      if (!Number.isNaN(n)) phase = n;
    } else if (a === '--foundation-only') {
      foundationOnly = true;
    } else if (a === '--kb-path') {
      kbPath = args[++i] || null;
    }
  }

  return { phase, foundationOnly, kbPath };
}

function usage() {
  return [
    'Usage: node verify-phase.cjs --phase <N> [options]',
    '',
    '  --phase 1                 Verify Phase 1 deliverable (.forge/config.json)',
    '  --phase 1 --foundation-only  Verify project.name + project.prefix only',
    '  --phase 2 --kb-path <p>   Verify Phase 2 KB architecture docs',
    '  --phase 3                 Verify Phase 3 .forge/{workflows,personas,skills,templates}',
    '',
    '  Exit 0: pass | Exit 1: fail (JSON on stdout) | Exit 2: bad args (text on stderr)',
  ].join('\n');
}

// ── Phase 1: config.json verification ───────────────────────────────────────

const PHASE1_CHECKED = [
  'version',
  'project.name',
  'project.prefix',
  'stack',
  'commands',
  'paths.engineering',
  'paths.store',
  'paths.workflows',
];

function verifyPhase1(cwd) {
  const configPath = path.join(cwd, '.forge', 'config.json');

  if (!fs.existsSync(configPath)) {
    return {
      phase: 1,
      ok: false,
      missing: ['.forge/config.json'],
      reason: 'config file not written',
      checked: PHASE1_CHECKED,
    };
  }

  let cfg;
  try {
    cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  } catch (err) {
    return {
      phase: 1,
      ok: false,
      missing: ['.forge/config.json'],
      reason: `JSON parse failed: ${err.message ?? '?'}`,
      checked: PHASE1_CHECKED,
    };
  }

  const missing = [];
  if (!cfg.version) missing.push('version');
  const proj = cfg.project;
  if (!proj || !proj.name) missing.push('project.name');
  if (!proj || !proj.prefix) missing.push('project.prefix');
  if (!cfg.stack) missing.push('stack');
  if (!cfg.commands) missing.push('commands');
  const paths = cfg.paths;
  if (!paths || !paths.engineering) missing.push('paths.engineering');
  if (!paths || !paths.store) missing.push('paths.store');
  if (!paths || !paths.workflows) missing.push('paths.workflows');

  return {
    phase: 1,
    ok: missing.length === 0,
    missing,
    checked: PHASE1_CHECKED,
  };
}

// ── Phase 1 --foundation-only ────────────────────────────────────────────────

const PHASE1_FOUNDATION_CHECKED = ['project.name', 'project.prefix'];

function verifyPhase1Foundation(cwd) {
  const configPath = path.join(cwd, '.forge', 'config.json');

  if (!fs.existsSync(configPath)) {
    return {
      phase: 1,
      ok: false,
      missing: ['.forge/config.json'],
      reason: 'config file not written',
      checked: PHASE1_FOUNDATION_CHECKED,
    };
  }

  let cfg;
  try {
    cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  } catch (err) {
    return {
      phase: 1,
      ok: false,
      missing: ['.forge/config.json'],
      reason: `JSON parse failed: ${err.message ?? '?'}`,
      checked: PHASE1_FOUNDATION_CHECKED,
    };
  }

  const missing = [];
  const proj = cfg.project;
  if (!proj || !proj.name) missing.push('project.name');
  if (!proj || !proj.prefix) missing.push('project.prefix');

  return {
    phase: 1,
    ok: missing.length === 0,
    missing,
    checked: PHASE1_FOUNDATION_CHECKED,
  };
}

// ── Phase 2: KB architecture docs verification ────────────────────────────────

const ARCH_DOCS = ['stack', 'processes', 'database', 'routing', 'deployment', 'entity-model', 'stack-checklist'];

function verifyPhase2(cwd, kbPath) {
  const missing = [];
  const checked = [];

  for (const doc of ARCH_DOCS) {
    const rel = path.join(kbPath, 'architecture', `${doc}.md`);
    checked.push(rel);
    if (!fs.existsSync(path.join(cwd, rel))) {
      missing.push(rel);
    }
  }

  return {
    phase: 2,
    ok: missing.length === 0,
    missing,
    checked,
  };
}

// ── Phase 3: .forge subdirectories verification ───────────────────────────────

const PHASE3_DIRS = ['workflows', 'personas', 'skills', 'templates'];

// JS workflow files that substitute-placeholders.cjs emits from
// base-pack/workflows-js/ into .claude/workflows/ (FORGE-S28-T01).
// These are checked in addition to the .forge/ directory checks.
const PHASE3_JS_FILES = [
  path.join('.claude', 'workflows', 'wfl-run-task.js'),
  path.join('.claude', 'workflows', 'wfl-run-sprint.js'),
];

function verifyPhase3(cwd) {
  const missing = [];
  const checked = [];

  for (const d of PHASE3_DIRS) {
    const dir = path.join(cwd, '.forge', d);
    const rel = `.forge/${d}/`;
    checked.push(rel);
    let count = 0;
    try {
      count = fs.readdirSync(dir).filter(f => f.endsWith('.md') || f.endsWith('.json')).length;
    } catch {
      count = 0;
    }
    if (count === 0) {
      missing.push(`${rel} (empty)`);
    }
  }

  // Assert generated JS workflow files are present (FORGE-S28-T01)
  for (const relFile of PHASE3_JS_FILES) {
    checked.push(relFile);
    const absFile = path.join(cwd, relFile);
    if (!fs.existsSync(absFile)) {
      missing.push(relFile);
    }
  }

  return {
    phase: 3,
    ok: missing.length === 0,
    missing,
    checked,
  };
}

// ── Main ─────────────────────────────────────────────────────────────────────

function main() {
  const { phase, foundationOnly, kbPath } = parseArgs(process.argv);

  // Validate args
  if (phase === null || phase < 1 || phase > 3) {
    process.stderr.write(`Error: --phase must be 1, 2, or 3\n\n${usage()}\n`);
    process.exit(2);
  }

  if (phase === 2 && !kbPath) {
    process.stderr.write(`Error: --phase 2 requires --kb-path <path>\n\n${usage()}\n`);
    process.exit(2);
  }

  const cwd = process.cwd();

  let result;
  if (phase === 1) {
    result = foundationOnly ? verifyPhase1Foundation(cwd) : verifyPhase1(cwd);
  } else if (phase === 2) {
    result = verifyPhase2(cwd, kbPath);
  } else {
    result = verifyPhase3(cwd);
  }

  if (result.ok) {
    process.exit(0);
  } else {
    process.stdout.write(JSON.stringify(result, null, 2) + '\n');
    process.exit(1);
  }
}

main();
