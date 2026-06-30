'use strict';

/**
 * paths.cjs — Shared path-computation helpers for Forge tools.
 *
 * Single source of truth for directory-name conventions that must be
 * consistent across substitute-placeholders.cjs, check-structure.cjs, etc.
 *
 * Exported API:
 *   getCommandsSubdir() — returns 'forge' (fixed namespace)
 */

/**
 * Commands subdirectory name under .claude/commands/.
 *
 * CLI-first redesign: the namespace is FIXED to 'forge'. Project-prefix
 * command namespaces (/acme:*, /hello:*) are retired — every project gets
 * the same /forge:* surface, matching what the 4ge bootstrap vendors into
 * .claude/commands/forge/. The prefix-derived namespace existed to avoid
 * collisions with the Forge *plugin's* own /forge:* commands; moot now
 * that the plugin mechanism is retired.
 *
 * @param {string} [_prefix] — vestigial, ignored (kept for caller compat)
 * @returns {string} always 'forge'
 */
function getCommandsSubdir(_prefix) {
  return 'forge';
}

module.exports = { getCommandsSubdir };