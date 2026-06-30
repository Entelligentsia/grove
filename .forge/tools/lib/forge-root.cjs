'use strict';

// Forge shared helper: resolveForgeRoot
// FR-001: 3-tier priority resolution for FORGE_ROOT with actionable error.
//
// Resolution order:
//   1. FORGE_ROOT env var — accepted only if ${FORGE_ROOT}/.claude-plugin/plugin.json exists
//   2. __dirname/.. — the parent of this module's directory (tools/lib -> forge root)
//   3. Actionable error listing all checked paths
//
// This follows the same pattern as project-root.cjs (library module pattern):
//   - Export functions, no require.main === module CLI block
//   - Throw errors, don't call process.exit

const fs = require('fs');
const path = require('path');

/**
 * Resolve the Forge plugin root directory.
 *
 * @param {string} [envForgeRoot] - Value of FORGE_ROOT environment variable.
 *   If provided and points to a directory containing .claude-plugin/plugin.json,
 *   it is accepted. If provided but plugin.json is missing, resolution falls
 *   through to the __dirname fallback. If omitted/undefined, falls through.
 * @returns {string} Absolute path to the Forge plugin root.
 * @throws {Error} When no valid resolution path is found, with a message listing
 *   all paths that were checked.
 */
function resolveForgeRoot(envForgeRoot) {
  const checkedPaths = [];

  // Tier 1: Environment variable — only accepted if plugin.json exists
  if (envForgeRoot) {
    const pluginJsonPath = path.join(envForgeRoot, '.claude-plugin', 'plugin.json');
    checkedPaths.push(pluginJsonPath);
    if (fs.existsSync(pluginJsonPath)) {
      return envForgeRoot;
    }
    // Env var provided but doesn't point to a valid plugin root — fall through
  }

  // Tier 2: __dirname/../.. — the standard layout (forge/forge/tools/lib/ -> forge/forge/)
  // This module lives in tools/lib/, so going up two directories reaches the plugin root.
  const fallbackRoot = path.resolve(path.join(__dirname, '..', '..'));
  const fallbackPluginJson = path.join(fallbackRoot, '.claude-plugin', 'plugin.json');
  checkedPaths.push(fallbackPluginJson);
  if (fs.existsSync(fallbackPluginJson)) {
    return fallbackRoot;
  }

  // Tier 3: Actionable error
  throw new Error(
    `Cannot resolve Forge plugin root. Checked paths:\n` +
    checkedPaths.map(p => `  - ${p}`).join('\n') +
    '\nSet FORGE_ROOT to a directory containing .claude-plugin/plugin.json'
  );
}

module.exports = { resolveForgeRoot };