'use strict';
// lib/schema-loader.cjs — memoized, multi-path schema search chain.
//
// Extracted from the _getSchemas() closure in tools/store-cli.cjs so both
// store-cli.cjs and hooks/validate-write.js (and any future consumer) can
// load schemas through one canonical place.
//
// Search order (first hit wins per schema file):
//   1. project-installed:  {cwd}/.forge/schemas/{file}
//   2. in-tree (dev):      {cwd}/forge/schemas/{file}
//   3. plugin-installed:   {pluginDir}/{file}   (default: __dirname/../schemas)
//   4. forge-cli bundled:  {bundledDir}/{file}  (default: __dirname/../.schemas)
//   5. fallback:           minimal required-fields stub + WARN on stderr

const fs = require('fs');
const path = require('path');

// schema-loader.cjs lives at tools/lib/schema-loader.cjs.
// Default schema dirs are computed relative to the tools/ parent directory
// (i.e., two levels up from __dirname) so they resolve correctly whether
// loaded from a dev tree or a forge-cli bundled payload copy.
//
//   __dirname          = …/tools/lib
//   TOOLS_DIR          = …/tools
//   DEFAULT_PLUGIN_DIR = …/schemas           (= $FORGE_ROOT/schemas/)
//   DEFAULT_BUNDLED_DIR= …/.schemas          (= forge-cli bundled .schemas/)
const TOOLS_DIR = path.join(__dirname, '..');

const ENTITY_TYPES = ['sprint', 'task', 'bug', 'event', 'feature'];

// Canonical minimal required-field set per entity type.
// Kept in one place here; store-cli.cjs and validate-write.js should import
// this instead of re-declaring their own copies.
const MINIMAL_REQUIRED = {
  sprint:  ['sprintId', 'title', 'status', 'taskIds', 'createdAt'],
  task:    ['taskId', 'sprintId', 'title', 'status', 'path'],
  bug:     ['bugId', 'title', 'severity', 'status', 'path', 'reportedAt'],
  event:   ['eventId', 'taskId', 'sprintId', 'role', 'action', 'phase', 'iteration',
            'startTimestamp', 'endTimestamp', 'durationMinutes', 'model'],
  feature: ['id', 'title', 'status', 'created_at'],
};

const AUX_SCHEMAS = {
  'event-sidecar':   'event-sidecar.schema.json',
  'progress-entry':  'progress-entry.schema.json',
  'collation-state': 'collation-state.schema.json',
};

// Module-level memoization cache — cleared by resetSchemaCache() for testing.
let _cache = null;

/**
 * Load the full set of Forge schemas, memoized per process.
 *
 * @param {object} [options]
 * @param {string} [options.cwd]       Override process.cwd() for path resolution (testing).
 * @param {string} [options.pluginDir] Override default plugin schema dir (testing).
 * @param {string} [options.bundledDir] Override default forge-cli bundled schema dir (testing).
 * @returns {{ [type: string]: object }} Map of entity/aux type → parsed JSON schema.
 */
function loadSchemas(options) {
  if (_cache) return _cache;

  const cwd       = (options && options.cwd)        || process.cwd();
  const pluginDir = (options && options.pluginDir)   || path.join(TOOLS_DIR, '..', 'schemas');
  const bundledDir = (options && options.bundledDir) || path.join(TOOLS_DIR, '..', '.schemas');

  const projectDir = path.join(cwd, '.forge', 'schemas');
  const inTreeDir  = path.join(cwd, 'forge', 'schemas');

  const allTypes = [...ENTITY_TYPES, ...Object.keys(AUX_SCHEMAS)];
  const schemas = {};

  for (const type of allTypes) {
    const schemaFile = AUX_SCHEMAS[type] || `${type}.schema.json`;
    let schema = null;

    // 1. project-installed
    const projectPath = path.join(projectDir, schemaFile);
    try {
      if (fs.existsSync(projectPath)) {
        schema = JSON.parse(fs.readFileSync(projectPath, 'utf8'));
      }
    } catch (_) {}

    // 2. in-tree (dev mode)
    if (!schema) {
      const inTreePath = path.join(inTreeDir, schemaFile);
      try {
        if (fs.existsSync(inTreePath)) {
          schema = JSON.parse(fs.readFileSync(inTreePath, 'utf8'));
        }
      } catch (_) {}
    }

    // 3. plugin-installed  (store-cli.cjs lives at $FORGE_ROOT/tools/, so
    //    default pluginDir = __dirname/../schemas = $FORGE_ROOT/schemas/)
    if (!schema) {
      const pluginPath = path.join(pluginDir, schemaFile);
      try {
        if (fs.existsSync(pluginPath)) {
          schema = JSON.parse(fs.readFileSync(pluginPath, 'utf8'));
        }
      } catch (_) {}
    }

    // 4. forge-cli bundled payload (build-payload.cjs writes schemas to .schemas/)
    if (!schema) {
      const bundledPath = path.join(bundledDir, schemaFile);
      try {
        if (fs.existsSync(bundledPath)) {
          schema = JSON.parse(fs.readFileSync(bundledPath, 'utf8'));
        }
      } catch (_) {}
    }

    if (schema) {
      schemas[type] = schema;
    } else {
      // eslint-disable-next-line no-console
      process.stderr.write(`WARN: schema file ${schemaFile} not found, using minimal fallback\n`);
      schemas[type] = { type: 'object', required: MINIMAL_REQUIRED[type] || [], properties: {} };
    }
  }

  _cache = schemas;
  return _cache;
}

/**
 * Clear the memoization cache.
 * FOR TESTING ONLY — not part of the production API.
 * Do not call this from production code.
 */
function resetSchemaCache() {
  _cache = null;
}

module.exports = { loadSchemas, resetSchemaCache, ENTITY_TYPES, MINIMAL_REQUIRED, AUX_SCHEMAS };
