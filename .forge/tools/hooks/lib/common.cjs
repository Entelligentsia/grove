'use strict';

/**
 * hooks/lib/common.cjs — Shared primitives for Forge hooks
 *
 * Provides four canonical exports used across hook files:
 *   - resolveForgePaths()       (H-1a) — read .forge/config.json and derive key paths
 *   - readStdinJson(cb, stdin)  (H-1b) — async stdin → JSON parsing (callback pattern)
 *   - formatHookOutput(name, p) (H-1c) — build hookSpecificOutput JSON envelope
 *   - FORGE_COMMAND_PATTERNS    (H-1d) — canonical forge-command RegExp array
 *
 * IMPORTANT: This file is intentionally excluded from the forge-cli payload
 * (build-payload.cjs copies only hooks/*.js, not hooks/lib/). Therefore:
 *   - Only .cjs hooks (post-init.cjs, post-sprint.cjs) may require this file.
 *   - .js hooks (triage-error.js, forge-permissions.js) retain inline copies
 *     of the patterns they need. Those inline copies carry comments pointing
 *     here as the canonical source. Keep both in sync when patterns change.
 *
 * Closes findings: H-1a, H-1b, H-1c, H-1d (FORGE-S25-T08)
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

// ---------------------------------------------------------------------------
// H-1a: resolveForgePaths
// ---------------------------------------------------------------------------

/**
 * Read .forge/config.json from the current working directory and derive the
 * key filesystem paths used by hooks.
 *
 * Merged from the two identical implementations in post-init.cjs and
 * post-sprint.cjs. The post-init version included structureVersionsPath;
 * this function always returns all fields — callers that don't need a field
 * simply ignore it.
 *
 * @returns {{ forgeDir: string, eventsRoot: string, cacheDir: string,
 *             forgeRoot: string|null, structureVersionsPath: string } | null}
 *   Returns null if .forge/config.json is missing or unparseable.
 */
function resolveForgePaths() {
  const cfgPath = path.join(process.cwd(), '.forge', 'config.json');
  if (!fs.existsSync(cfgPath)) return null;
  let cfg;
  try { cfg = JSON.parse(fs.readFileSync(cfgPath, 'utf8')); } catch (_) { return null; }
  const forgeDir = path.dirname(cfgPath);
  return {
    forgeDir,
    eventsRoot: path.join(forgeDir, 'store', 'events'),
    structureVersionsPath: path.join(forgeDir, 'structure-versions.json'),
    cacheDir: path.join(forgeDir, 'cache'),
    forgeRoot: (cfg.paths && cfg.paths.forgeRoot) || null,
  };
}

// ---------------------------------------------------------------------------
// H-1b: readStdinJson
// ---------------------------------------------------------------------------

/**
 * Read all data from a readable stream (defaulting to process.stdin), parse
 * as JSON, and invoke the callback with the result.
 *
 * Merges the async stdin reading pattern used by triage-error.js and
 * forge-permissions.js. Note: .js hooks retain inline copies of this
 * pattern to avoid a module-scope require on hooks/lib/ (forge-cli bundle
 * gap — see file header). This function is provided for .cjs hook consumers
 * and for future use when the .js extension is migrated in T14.
 *
 * @param {function(object|null): void} callback
 *   Called with the parsed object, or null if input is empty or malformed.
 * @param {NodeJS.ReadableStream} [stdin]
 *   Readable stream to consume. Defaults to process.stdin.
 */
function readStdinJson(callback, stdin) {
  const stream = stdin || process.stdin;
  if (stream.setEncoding) stream.setEncoding('utf8');
  let raw = '';
  stream.on('data', chunk => { raw += chunk; });
  stream.on('end', () => {
    if (!raw) { callback(null); return; }
    try {
      callback(JSON.parse(raw));
    } catch (_) {
      callback(null);
    }
  });
}

// ---------------------------------------------------------------------------
// H-1c: formatHookOutput
// ---------------------------------------------------------------------------

/**
 * Build the Claude Code hook stdout envelope as a JSON string.
 *
 * Claude Code hook protocol: hooks write a JSON object to stdout where the
 * top-level key is `hookSpecificOutput` containing `hookEventName` plus any
 * additional fields from payload.
 *
 * @param {string} hookEventName - e.g. 'PostToolUse', 'SessionStart'
 * @param {object} payload - additional fields to include in hookSpecificOutput
 * @returns {string} JSON string ready for process.stdout.write
 */
function formatHookOutput(hookEventName, payload) {
  return JSON.stringify({
    hookSpecificOutput: {
      hookEventName,
      ...payload,
    },
  });
}

// ---------------------------------------------------------------------------
// H-1d: FORGE_COMMAND_PATTERNS
// ---------------------------------------------------------------------------

/**
 * Canonical array of RegExp patterns that identify forge-related commands.
 *
 * @catalogSync forge/schemas/enum-catalog.json#commandNames (FORGE-S25-T26)
 *
 * Build-time drift detection: forge/tools/__tests__/build-enum-catalog.test.cjs
 * verifies that every `forge:*` entry in enum-catalog.json commandNames has at
 * least one matching regex here. Run `node --test forge/tools/__tests__/*.test.cjs`
 * to check. Drift will cause the drift-detection test to fail.
 *
 * Runtime: this array is the single source of truth for forge command recognition.
 * hooks/triage-error.cjs and hooks/forge-permissions.cjs maintain inline copies
 * of subsets; those files cannot require() this module due to the forge-cli bundle
 * gap (build-payload.cjs bundles hooks/*.cjs but excludes hooks/lib/).
 *
 * When adding a new forge command: update this array, the inline copies in
 * triage-error.cjs and forge-permissions.cjs, AND build-enum-catalog.cjs COMMAND_NAMES.
 */
const FORGE_COMMAND_PATTERNS = [
  /manage-config/,
  /\.forge\//,
  /CLAUDE_PLUGIN_ROOT/,
  /FORGE_ROOT/,
  /MANAGE_CONFIG/,
  /engineering\/tools\//,
  /forge:init/,
  /forge:health/,
  /forge:rebuild/,
  /forge:update/,
  /forge:add-pipeline/,
  /forge:add-task/,
  /forge:plan/,
  /forge:implement/,
  /forge:approve/,
  /forge:commit/,
  /forge:review/,
  /forge:new-sprint/,
  /forge:plan-sprint/,
  /forge:run-task/,
  /forge:run-sprint/,
  /forge:fix-bug/,
  /forge:retro/,
  /forge:check-agent/,
  /forge:report-bug/,
  // forge:enhance removed in v1.0 (T03) — absorbed into forge:rebuild --enrich
  // forge:collate removed from user-facing surface in v1.0 (T03) — internal tool only
  /forge:validate/,
  // forge:calibrate removed in v1.0 (T03) — absorbed into forge:health --fix (T04)
  // forge:materialize removed in v1.0 (T03) — fast-mode eliminated in T01
  /forge:remove/,
  /forge:search/,
  /forge:repair/,
  /forge:store-custodian/,
  /forge:config/,
  /forge:ask/,
  /forge:refresh-kb-links/,
  /store-cli\.cjs/,
  /validate-store\.cjs/,
];

// ---------------------------------------------------------------------------
// H-5b: logSwallowedError
// ---------------------------------------------------------------------------

/**
 * Append a diagnostic line to the swallowed-error log at
 * `$dataDir/logs/forge-hooks.log`. If `dataDir` is falsy, falls back to
 * `os.tmpdir()/forge-plugin-data/logs/forge-hooks.log`.
 *
 * Format per line:
 *   <ISO-timestamp> [<tag>] <err.message>
 *
 * Invariants:
 *   - Append-only. No log rotation. Users can `truncate -s 0` or `rm` the
 *     file; it will be recreated on the next swallowed error.
 *   - Hook code NEVER reads this log.
 *   - Fully fail-open: if the log write itself fails, we exit silently.
 *
 * Closes finding: H-5b (FORGE-S25-T15)
 *
 * @param {string} tag - Short hook identifier (e.g. 'post-init', 'post-sprint').
 * @param {Error|*} err - The caught error. Uses err.message if available.
 * @param {string|null|undefined} dataDir - CLAUDE_PLUGIN_DATA directory.
 */
function logSwallowedError(tag, err, dataDir) {
  try {
    const baseDir = dataDir || path.join(os.tmpdir(), 'forge-plugin-data');
    const logsDir = path.join(baseDir, 'logs');
    fs.mkdirSync(logsDir, { recursive: true });
    const logPath = path.join(logsDir, 'forge-hooks.log');
    const msg = (err && err.message) ? err.message : String(err);
    const line = `${new Date().toISOString()} [${tag}] ${msg}\n`;
    fs.appendFileSync(logPath, line, 'utf8');
  } catch (_) {
    // Fully fail-open — never re-throw
  }
}

// ---------------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------------

module.exports = {
  resolveForgePaths,
  readStdinJson,
  formatHookOutput,
  FORGE_COMMAND_PATTERNS,
  logSwallowedError,
};
