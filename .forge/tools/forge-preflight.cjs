#!/usr/bin/env node
'use strict';

// forge-preflight.cjs — FORGE-S27-T01 (item A1).
//
// Bundles the deterministic pre-dispatch glue the LLM orchestrator currently
// hand-runs turn-by-turn — FORGE_ROOT resolution, config reconciliation,
// generation-manifest state, calibration-baseline freshness, MASTER_INDEX
// hashing, structure check, run timestamp — into ONE compact JSON blob,
// emitted once. The orchestrator reads this single blob instead of issuing
// ~20 separate round-trips before the first phase dispatch.
//
// Design: a PURE AGGREGATOR. It composes the existing deterministic tools
// (generation-manifest.cjs's hashContent, check-structure.cjs's
// checkNamespaces/validateManifest) and reads config — it does NOT reimplement
// their logic, so there is one source of truth per concern.
//
// Contract:
//   - Reads and checks only by default; no implicit side effects (idempotent).
//   - Fast-fail-safe: any probed-concern failure is captured into warnings[]
//     and sets ok:false; the tool NEVER throws uncaught and never half-writes
//     state. It always emits a parseable JSON blob on stdout.
//   - Output is keyed for a freshness/idempotency guard via masterIndexHash +
//     configMtime so callers can skip recompute when the blob is current.
//
// Usage:
//   node forge-preflight.cjs [--sprint <id>] [--task <id>] [--bug <id>]
//                            [--path <project-root>]

// Fast-fail-safe: an uncaught error must still surface as a blob, not a stack
// trace. We register a last-resort handler that emits ok:false and exits 0
// (the orchestrator branches on blob.ok, not on the exit code).
process.on('uncaughtException', (err) => {
  try {
    process.stdout.write(JSON.stringify({
      ok: false,
      forgeRoot: null,
      masterIndexHash: null,
      generatedAt: new Date().toISOString(),
      warnings: [`uncaught: ${err && err.message ? err.message : String(err)}`],
    }) + '\n');
  } catch (_) { /* nothing more we can do */ }
  process.exit(0);
});

const fs = require('fs');
const path = require('path');

// ---------------------------------------------------------------------------
// Arg parsing (tolerant — unknown flags are ignored, not fatal).
// ---------------------------------------------------------------------------
function parseArgs(argv) {
  const out = { projectRoot: process.cwd() };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--sprint' && argv[i + 1]) { out.sprintId = argv[++i]; }
    else if (a === '--task' && argv[i + 1]) { out.taskId = argv[++i]; }
    else if (a === '--bug' && argv[i + 1]) { out.bugId = argv[++i]; }
    else if (a === '--path' && argv[i + 1]) { out.projectRoot = path.resolve(argv[++i]); }
  }
  return out;
}

// ---------------------------------------------------------------------------
// Pure helpers. Each concern is wrapped so a single failure becomes a warning
// rather than aborting the whole preflight.
// ---------------------------------------------------------------------------

// Compose generation-manifest.cjs's hashing primitive — single source of truth
// for content hashing across the plugin.
function loadHashContent(forgeRoot) {
  try {
    const mod = require(path.join(forgeRoot, 'tools', 'generation-manifest.cjs'));
    if (mod && typeof mod.hashContent === 'function') return mod.hashContent;
  } catch (_) { /* fall through to local */ }
  // Fallback that matches generation-manifest's algorithm (sha256 hex) so the
  // blob is still useful if the module cannot be required.
  const crypto = require('crypto');
  return (content) => crypto.createHash('sha256').update(content, 'utf8').digest('hex');
}

function readConfig(projectRoot) {
  const configPath = path.join(projectRoot, '.forge', 'config.json');
  const stat = fs.statSync(configPath); // throws if missing -> caught by caller
  const raw = fs.readFileSync(configPath, 'utf8');
  const config = JSON.parse(raw); // throws on malformed -> caught by caller
  return { config, configMtime: stat.mtimeMs };
}

function hashMasterIndex(projectRoot, config, hashContent) {
  const engineering = (config.paths && config.paths.engineering) || 'engineering';
  const indexPath = path.join(projectRoot, engineering, 'MASTER_INDEX.md');
  if (!fs.existsSync(indexPath)) return null;
  return hashContent(fs.readFileSync(indexPath, 'utf8'));
}

// calibrationFresh carries ACTIONABLE detail, not a bare boolean: when stale,
// it names the remedy and the reason so the orchestrator can act on the blob
// without re-deriving the signal.
function assessCalibration(config, masterIndexHash) {
  const baseline = config.calibrationBaseline;
  if (!baseline) {
    return { fresh: false, suggest: '/forge:calibrate', reason: 'no calibrationBaseline in config' };
  }
  if (masterIndexHash && baseline.masterIndexHash && baseline.masterIndexHash !== masterIndexHash) {
    return { fresh: false, suggest: '/forge:calibrate', reason: 'masterIndexHash drift since last calibration' };
  }
  return { fresh: true, lastCalibrated: baseline.lastCalibrated || null };
}

// Compose check-structure.cjs's namespace check. Returns actionable detail.
function assessStructure(forgeRoot, projectRoot, warnings) {
  try {
    const cs = require(path.join(forgeRoot, 'tools', 'check-structure.cjs'));
    const manifestPath = path.join(forgeRoot, 'schemas', 'structure-manifest.json');
    if (typeof cs.checkNamespaces !== 'function' || !fs.existsSync(manifestPath)) {
      return { ok: null, reason: 'structure check unavailable' };
    }
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    const result = cs.checkNamespaces(manifest, projectRoot, {});
    // checkNamespaces returns a structured result; treat any missing-file
    // namespace as "not ok" but non-fatal (a warning, not a throw).
    const allOk = result && (result.ok === true || result.allPresent === true ||
      (Array.isArray(result.namespaces) && result.namespaces.every((n) => n.missing === 0 || (n.missing && n.missing.length === 0))));
    if (!allOk) warnings.push('structure check reported missing namespace files');
    return { ok: !!allOk };
  } catch (err) {
    warnings.push(`structure check error: ${err.message}`);
    return { ok: null, reason: err.message };
  }
}

// generation-manifest status — actionable manifest state.
function assessManifest(forgeRoot, projectRoot, warnings) {
  const manifestPath = path.join(projectRoot, '.forge', 'schemas', 'generation-manifest.json');
  // The manifest lives in the generated instance; absence is informational,
  // not an error (e.g. fresh project or fast-mode).
  if (!fs.existsSync(manifestPath)) {
    return { present: false, reason: 'no generation-manifest in instance (informational)' };
  }
  try {
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    const count = manifest.files ? Object.keys(manifest.files).length : 0;
    return { present: true, trackedFiles: count };
  } catch (err) {
    warnings.push(`generation-manifest parse error: ${err.message}`);
    return { present: true, error: err.message };
  }
}

// Resolve FORGE_ROOT via forgeRef-based cache scan — mirrors manage-config.cjs
// Priority 2 logic (FORGE-S29-T03). paths.forgeRoot is deprecated and no longer read.
function resolveForgeRootFromConfig(config) {
  const forgeRef = config.paths && config.paths.forgeRef;
  if (!forgeRef) return null;
  const homeDir = require('os').homedir();
  const candidates = [
    path.join(homeDir, '.claude', 'plugins', 'cache', 'forge', 'forge', forgeRef),
    path.join(homeDir, '.claude', 'plugins', 'marketplaces', 'skillforge', 'forge', 'forge', forgeRef),
  ];
  for (const c of candidates) {
    try {
      const pluginJsonPath = path.join(c, '.claude-plugin', 'plugin.json');
      if (fs.existsSync(pluginJsonPath)) {
        const manifest = JSON.parse(fs.readFileSync(pluginJsonPath, 'utf8'));
        if (manifest.version === forgeRef) return c;
      }
    } catch (_) { /* try next candidate */ }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Main aggregation.
// ---------------------------------------------------------------------------
function preflight(opts) {
  const warnings = [];
  const blob = {
    ok: true,
    forgeRoot: null,
    sprintId: opts.sprintId || null,
    configReconciled: false,
    manifestState: null,
    calibrationFresh: null,
    masterIndexHash: null,
    configMtime: null,
    structureOk: null,
    generatedAt: new Date().toISOString(),
    warnings,
  };

  // 1. Config — the load-bearing input. A failure here is fatal to ok.
  let config;
  try {
    const r = readConfig(opts.projectRoot);
    config = r.config;
    blob.configMtime = r.configMtime;
    blob.configReconciled = true;
  } catch (err) {
    blob.ok = false;
    warnings.push(`config unreadable: ${err.message}`);
    return blob; // fast-fail-safe: cannot proceed without config, but no throw
  }

  // 2. FORGE_ROOT resolution — mirrors manage-config.cjs Priority 2 (forgeRef cache scan).
  // paths.forgeRoot is deprecated (FORGE-S29-T03); we resolve via forgeRef only.
  // blob.forgeRoot is still emitted for backward-compatible telemetry.
  blob.forgeRoot = resolveForgeRootFromConfig(config);
  if (!blob.forgeRoot) {
    const forgeRef = (config.paths && config.paths.forgeRef) || null;
    const hint = forgeRef
      ? `Cannot resolve Forge plugin root from forgeRef "${forgeRef}" — run /forge:update to refresh.`
      : 'paths.forgeRef missing from config — cannot resolve Forge plugin root.';
    blob.ok = false;
    warnings.push(hint);
    return blob;
  }

  const hashContent = loadHashContent(blob.forgeRoot);

  // 3. MASTER_INDEX hash (freshness key + calibration input).
  try {
    blob.masterIndexHash = hashMasterIndex(opts.projectRoot, config, hashContent);
    if (!blob.masterIndexHash) warnings.push('MASTER_INDEX.md absent — hash null');
  } catch (err) {
    warnings.push(`master-index hash error: ${err.message}`);
  }

  // 4. Calibration freshness (actionable).
  try {
    blob.calibrationFresh = assessCalibration(config, blob.masterIndexHash);
  } catch (err) {
    warnings.push(`calibration assessment error: ${err.message}`);
  }

  // 5. Manifest state (actionable).
  try {
    blob.manifestState = assessManifest(blob.forgeRoot, opts.projectRoot, warnings);
  } catch (err) {
    warnings.push(`manifest assessment error: ${err.message}`);
  }

  // 6. Structure check.
  try {
    const s = assessStructure(blob.forgeRoot, opts.projectRoot, warnings);
    blob.structureOk = s.ok;
  } catch (err) {
    warnings.push(`structure assessment error: ${err.message}`);
  }

  // Aggregate verdict: ok stays true for soft/informational warnings (absent
  // master index, missing optional manifest); it is only flipped false above
  // for hard failures (no config / no forgeRoot). This keeps the orchestrator
  // running on a healthy-but-fresh project while still surfacing advisories.
  return blob;
}

// ---------------------------------------------------------------------------
// CLI entry.
// ---------------------------------------------------------------------------
if (require.main === module) {
  const opts = parseArgs(process.argv.slice(2));
  const blob = preflight(opts);
  process.stdout.write(JSON.stringify(blob) + '\n');
  process.exit(0); // exit code is always 0; callers branch on blob.ok
}

module.exports = { preflight, parseArgs, assessCalibration };
