#!/usr/bin/env node
'use strict';

// Forge hook: preflight-session (FORGE-S27-T01 / item A1).
//
// SessionStart hook that primes the orchestrator preflight cache blob
// (.forge/cache/preflight-status.json) by running forge-preflight.cjs once.
//
// SCOPING NOTE: SessionStart fires BEFORE any command is invoked and its
// envelope carries no per-command signal, so this hook is deliberately
// command-name-INDEPENDENT. Authoritative scoping to run-task / fix-bug /
// run-sprint lives in the orchestration command-preamble path (which reads the
// blob). This hook only ever:
//   - is a STRICT NO-OP when .forge/ is absent (never changes behavior for
//     non-Forge projects or unrelated commands);
//   - primes the blob when .forge/ is present;
//   - is FRESHNESS-GUARDED (idempotent): if an existing blob already matches
//     the current config mtime + MASTER_INDEX hash, it is left untouched;
//   - FAILS OPEN: any error -> stderr warning + exit 0. A hook failure must
//     never block session start.

process.on('uncaughtException', (err) => {
  try { process.stderr.write(`forge preflight-session: internal error (fail-open): ${err.message}\n`); } catch (_) {}
  process.exit(0);
});

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

// Read + discard the envelope on fd 0 (fail-open on any parse error).
try { fs.readFileSync(0, 'utf8'); } catch (_) { /* envelope optional */ }

// Strict no-op when .forge/ is absent.
const forgeDir = path.join(process.cwd(), '.forge');
const configPath = path.join(forgeDir, 'config.json');
if (!fs.existsSync(forgeDir) || !fs.existsSync(configPath)) {
  process.exit(0);
}

const cacheDir = path.join(forgeDir, 'cache');
const blobPath = path.join(cacheDir, 'preflight-status.json');

// Freshness guard (idempotency): if the existing blob was computed from the
// current config mtime, leave it untouched. forge-preflight records configMtime
// in the blob; MASTER_INDEX changes flow through masterIndexHash, which the
// blob also records, but the config mtime is the cheap primary key here.
function configMtimeMs() {
  try { return fs.statSync(configPath).mtimeMs; } catch (_) { return null; }
}

try {
  if (fs.existsSync(blobPath)) {
    const existing = JSON.parse(fs.readFileSync(blobPath, 'utf8'));
    const curMtime = configMtimeMs();
    if (existing && typeof existing.configMtime === 'number' &&
        curMtime !== null && existing.configMtime === curMtime) {
      // Blob is current — no rewrite, no duplicated side effect.
      process.exit(0);
    }
  }
} catch (_) { /* corrupt blob -> fall through and recompute */ }

// Resolve forge-preflight.cjs. Prefer CLAUDE_PLUGIN_ROOT (set by the runtime),
// fall back to config.paths.forgeRoot.
let forgeRoot = process.env.CLAUDE_PLUGIN_ROOT || null;
if (!forgeRoot) {
  try {
    const cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    forgeRoot = cfg && cfg.paths && cfg.paths.forgeRoot;
  } catch (_) { /* leave null */ }
}
if (!forgeRoot) process.exit(0); // cannot locate the tool — fail open

const tool = path.join(forgeRoot, 'tools', 'forge-preflight.cjs');
if (!fs.existsSync(tool)) process.exit(0);

// Run preflight and capture its single JSON blob.
const res = spawnSync('node', [tool, '--path', process.cwd()], {
  cwd: process.cwd(),
  encoding: 'utf8',
  timeout: 8000,
});
if (res.status !== 0 || !res.stdout) process.exit(0); // fail open

// Validate it parses before writing — never half-write a corrupt blob.
let blob;
try { blob = JSON.parse(res.stdout); } catch (_) { process.exit(0); }
if (!blob || typeof blob !== 'object') process.exit(0);

try {
  fs.mkdirSync(cacheDir, { recursive: true });
  // Atomic-ish write: tmp + rename so a crash never leaves a partial blob.
  const tmpPath = blobPath + '.tmp';
  fs.writeFileSync(tmpPath, JSON.stringify(blob, null, 2), 'utf8');
  fs.renameSync(tmpPath, blobPath);
} catch (_) { /* fail open */ }

process.exit(0);
