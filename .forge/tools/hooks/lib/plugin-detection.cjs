'use strict';
// hooks/lib/plugin-detection.cjs — extracted from check-update.js (H-2a, FORGE-S25-T14)
//
// Provides Forge plugin distribution detection and installation scanning.
// Extracted to improve testability — each function can be imported and
// tested independently without requiring the full check-update module.
//
// Uses only Node.js built-ins — no npm dependencies required.

const fs = require('fs');
const path = require('path');
const os = require('os');

// Distribution detection — derived from plugin path at runtime.
// The cache path encodes the marketplace name, making this more reliable than
// reading fields from plugin.json (which may be stale after a switch).
function detectDistribution(root) {
  return root.includes('/cache/skillforge/forge/') || root.includes('/marketplaces/skillforge/forge/')
    ? 'forge@skillforge' : 'forge@forge';
}

// Check if forge plugin is enabled in settings files.
// Returns true if no explicit disable found, false if disabled.
function isPluginEnabled(pluginPath, scope, homeDir, cwd) {
  try {
    // Check user settings: ~/.claude/settings.json
    const userSettingsPath = path.join(homeDir, '.claude', 'settings.json');
    if (fs.existsSync(userSettingsPath)) {
      const userSettings = JSON.parse(fs.readFileSync(userSettingsPath, 'utf8'));
      if (userSettings.disablePlugin === true) return false;
      // Check for per-plugin disable (if supported in future)
      if (userSettings.plugins && userSettings.plugins.forge === false) return false;
    }

    // Check project settings: ./.claude/settings.local.json
    const projectSettingsPath = path.join(cwd, '.claude', 'settings.local.json');
    if (fs.existsSync(projectSettingsPath)) {
      const projectSettings = JSON.parse(fs.readFileSync(projectSettingsPath, 'utf8'));
      if (projectSettings.disablePlugin === true) return false;
      if (projectSettings.plugins && projectSettings.plugins.forge === false) return false;
    }

    return true; // Default: enabled
  } catch (e) {
    return true; // Non-fatal — assume enabled if cannot read settings
  }
}

// Scans all known plugin locations to detect multiple Forge installations.
// Returns array of installation records with version, distribution, scope, enabled status.
// Optional parameters for dependency injection (testing).
function scanPluginInstallations(options) {
  const installations = [];
  const homeDir = (options && options.homeDir) || os.homedir();
  const cwd = (options && options.cwd) || process.cwd();

  // Candidate paths — user scope (global) and project scope (local)
  // Also scan skillforge subdirectory variant (skillforge/forge/forge)
  const basePaths = [
    path.join(homeDir, '.claude', 'plugins'),
    path.join(cwd, '.claude', 'plugins'),
  ];
  const variants = ['cache', 'marketplaces'];
  const pluginNames = ['forge/forge', 'skillforge/forge/forge'];

  const candidates = [];
  for (const basePath of basePaths) {
    for (const variant of variants) {
      for (const pluginName of pluginNames) {
        candidates.push(path.join(basePath, variant, pluginName));
      }
    }
  }

  for (const candidate of candidates) {
    try {
      const pluginJsonPath = path.join(candidate, '.claude-plugin', 'plugin.json');
      if (!fs.existsSync(pluginJsonPath)) continue;

      const manifest = JSON.parse(fs.readFileSync(pluginJsonPath, 'utf8'));
      // Determine scope: user-scope paths start with homeDir/.claude, project-scope start with cwd/.claude
      // Use cwd-relative check first to avoid false positives when cwd is subdir of homeDir
      const isProjectScope = candidate.startsWith(path.join(cwd, '.claude'));
      const isUserScope = candidate.startsWith(path.join(homeDir, '.claude'));
      const scope = isProjectScope ? 'project' : (isUserScope ? 'user' : 'unknown');
      const enabled = isPluginEnabled(candidate, scope, homeDir, cwd);

      // Avoid duplicates — skip if same path already recorded
      if (installations.some(i => i.path === candidate)) continue;

      installations.push({
        path: candidate,
        version: manifest.version || 'unknown',
        distribution: detectDistribution(candidate),
        scope: scope,
        enabled: enabled,
      });
    } catch (e) {
      // Non-fatal — skip broken installations silently
    }
  }

  return installations;
}

module.exports = { detectDistribution, isPluginEnabled, scanPluginInstallations };
