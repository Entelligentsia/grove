'use strict';
// hooks/lib/update-url.cjs — extracted from check-update.js (H-2b, FORGE-S25-T14)
//
// Provides update-check URL resolution and validation.
// Reads process.env.CLAUDE_PLUGIN_ROOT directly (option b from plan) so
// the module is self-contained — no pluginRoot argument needed at call sites.
//
// Uses only Node.js built-ins — no npm dependencies required.

const fs = require('fs');
const path = require('path');

// Determine the correct update-check URL for this distribution.
// Each distribution's plugin.json carries its own updateUrl pointing at the
// branch it was installed from (main for forge@forge, release for forge@skillforge),
// so we read it directly — no hardcoded per-distribution URLs needed.
const FALLBACK_UPDATE_URL = 'https://raw.githubusercontent.com/Entelligentsia/forge/main/forge/.claude-plugin/plugin.json';

const ALLOWED_DOMAINS = ['raw.githubusercontent.com'];

function validateUpdateUrl(url) {
  try {
    const parsed = new URL(url);
    const hostname = parsed.hostname.toLowerCase();
    if (!ALLOWED_DOMAINS.some(d => hostname === d || hostname.endsWith('.' + d))) {
      process.stderr.write(`forge-update: rejected update URL with disallowed domain '${hostname}', falling back\n`);
      return FALLBACK_UPDATE_URL;
    }
    return url;
  } catch {
    return FALLBACK_UPDATE_URL;
  }
}

// Reads updateUrl from the installed plugin.json.
// Uses process.env.CLAUDE_PLUGIN_ROOT (option b) with a fallback of '' so
// the function is self-contained. Falls back to FALLBACK_UPDATE_URL on any error.
function getUpdateUrl() {
  try {
    const pluginRoot = process.env.CLAUDE_PLUGIN_ROOT || '';
    const manifest = JSON.parse(fs.readFileSync(path.join(pluginRoot, '.claude-plugin', 'plugin.json'), 'utf8'));
    return validateUpdateUrl(manifest.updateUrl || FALLBACK_UPDATE_URL);
  } catch { return FALLBACK_UPDATE_URL; }
}

module.exports = { FALLBACK_UPDATE_URL, ALLOWED_DOMAINS, validateUpdateUrl, getUpdateUrl };
