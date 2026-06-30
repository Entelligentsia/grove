#!/usr/bin/env node
// Forge session-start hook — runs on SessionStart
// 1. Injects Forge-awareness context if this project has a .forge/ directory.
// 2. Checks once per day whether a newer version is available.
// 3. Detects distribution switches (forge@forge ↔ forge@skillforge) and
//    refreshes paths.forgeRoot in .forge/config.json so subagents always
//    reference the correct installed plugin path.
//
// Uses only Node.js built-ins — no npm dependencies required.
// Works on Linux, macOS, and Windows wherever Claude Code runs.

'use strict';

// This hook must never exit non-zero — a hook failure surfaces as noise to the
// user and blocks session start context. Any uncaught exception exits 0.
process.on('uncaughtException', () => process.exit(0));

const fs = require('fs');
const path = require('path');
const os = require('os');
const https = require('https');

// Extracted lib modules (H-2a, H-2b, H-2c — FORGE-S25-T14)
const { detectDistribution, scanPluginInstallations, isPluginEnabled } = require('./lib/plugin-detection.cjs');
const { FALLBACK_UPDATE_URL, ALLOWED_DOMAINS, validateUpdateUrl, getUpdateUrl } = require('./lib/update-url.cjs');
const { buildUpdateMsg, emit } = require('./lib/update-msg.cjs');

const pluginRoot = process.env.CLAUDE_PLUGIN_ROOT || '.';
const dataDir = process.env.CLAUDE_PLUGIN_DATA || path.join(os.tmpdir(), 'forge-plugin-data');
// Plugin-level cache: throttle only (lastCheck, remoteVersion) — shared across all projects.
const pluginCacheFile = path.join(dataDir, 'update-check-cache.json');
// Project-level cache: migration state (migratedFrom, localVersion, distribution, forgeRoot) — per project.
const forgeDir = '.forge';
const hasForge = fs.existsSync(forgeDir) && fs.existsSync(path.join(forgeDir, 'config.json'));
const projectCacheFile = path.join(forgeDir, 'update-check-cache.json');

const currentDistribution = detectDistribution(pluginRoot);

const remoteUrl = getUpdateUrl();
const checkInterval = 86400; // 24 hours in seconds

fs.mkdirSync(dataDir, { recursive: true });

// --- Forge-awareness context injection ---
let forgeContext = '';
if (fs.existsSync('.forge') && fs.existsSync(path.join('.forge', 'config.json'))) {
  forgeContext =
    'This project uses Forge AI-SDLC. Engineering knowledge base: engineering/. ' +
    'Generated workflows: .forge/workflows/. Sprint and task store: .forge/store/. ' +
    'Use the project slash commands (/plan, /implement, /sprint-plan) to drive development. ' +
    'Run /forge:health to check knowledge base currency.';
}

// --- Update check helpers ---
function localVersion() {
  try {
    const p = path.join(pluginRoot, '.claude-plugin', 'plugin.json');
    return JSON.parse(fs.readFileSync(p, 'utf8')).version || '0.0.0';
  } catch {
    return '0.0.0';
  }
}

function fetchRemoteVersion(cb) {
  https.get(remoteUrl, { timeout: 2000 }, (res) => {
    let body = '';
    res.on('data', chunk => { body += chunk; });
    res.on('end', () => {
      try { cb(JSON.parse(body).version || ''); } catch { cb(''); }
    });
  }).on('error', () => cb(''))
    .on('timeout', function() { this.destroy(); cb(''); });
}

// --- Main logic (only runs when executed as script, not when required as module) ---
if (require.main === module) {
  const local = localVersion();
  const now = Math.floor(Date.now() / 1000);

  // Scan for all plugin installations — builds inventory for multi-plugin awareness.
  const allInstallations = scanPluginInstallations();

// Plugin-level cache: throttle (lastCheck, remoteVersion) — shared, not migration state.
let pluginCache = null;
if (fs.existsSync(pluginCacheFile)) {
  try { pluginCache = JSON.parse(fs.readFileSync(pluginCacheFile, 'utf8')); } catch { pluginCache = null; }
}

// Project-level cache: migration state — per project.
let projectCache = null;
if (hasForge && fs.existsSync(projectCacheFile)) {
  try { projectCache = JSON.parse(fs.readFileSync(projectCacheFile, 'utf8')); } catch { projectCache = null; }
}

// FR-010: Backfill forgeRef from localVersion if missing in existing cache.
if (projectCache && !projectCache.forgeRef && projectCache.localVersion) {
  projectCache.forgeRef = projectCache.localVersion;
}

// --- Distribution + forgeRoot/forgeRef sync (always runs before update-check logic) ---
// Refreshes paths.forgeRoot and paths.forgeRef in config.json and the distribution/
// forgeRoot/forgeRef fields in the project cache. Handles distribution switches
// transparently — the user gets a clear message and all path references are
// corrected before any command runs.
let distributionSwitchMsg = '';
if (hasForge && pluginRoot !== '.') {
  // Keep paths.forgeRoot and paths.forgeRef in .forge/config.json in sync.
  // Generated workflows read forgeRoot to invoke tools without needing CLAUDE_PLUGIN_ROOT.
  // forgeRef is the version-based portable field (FR-010).
  try {
    const configPath = path.join(forgeDir, 'config.json');
    const cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    if (!cfg.paths) cfg.paths = {};
    let configChanged = false;
    if (cfg.paths.forgeRoot !== pluginRoot) {
      cfg.paths.forgeRoot = pluginRoot;
      configChanged = true;
    }
    // FR-010: Write forgeRef from the local plugin version.
    if (!cfg.paths.forgeRef || cfg.paths.forgeRef !== local) {
      cfg.paths.forgeRef = local;
      configChanged = true;
    }
    if (configChanged) {
      fs.writeFileSync(configPath, JSON.stringify(cfg, null, 2) + '\n');
    }
  } catch { /* non-fatal */ }

  if (projectCache) {
    const storedRoot = projectCache.forgeRoot;
    const storedDist = projectCache.distribution;
    const switched = storedRoot && storedRoot !== pluginRoot;

    // Build distribution switch message when the active plugin path changed and
    // the distribution name is different (e.g. forge@skillforge → forge@forge).
    if (switched && storedDist && storedDist !== currentDistribution) {
      const versionNote = projectCache.localVersion && projectCache.localVersion !== local
        ? ` Version: ${projectCache.localVersion} → ${local}.`
        : '';
      distributionSwitchMsg =
        `Plugin distribution switched from ${storedDist} to ${currentDistribution}.${versionNote}` +
        ` paths.forgeRoot updated. Run /forge:update to verify migration state.`;
    }

    // Sync distribution + forgeRoot + forgeRef into the project cache whenever they drift.
    // FR-002: Preserve updateStatus, pendingReason, pendingMigrations if present.
    if (storedRoot !== pluginRoot || storedDist !== currentDistribution) {
      try {
        const updated = { ...projectCache, distribution: currentDistribution, forgeRoot: pluginRoot, forgeRef: local };
        fs.writeFileSync(projectCacheFile, JSON.stringify(updated, null, 2) + '\n');
        projectCache = updated; // keep in-memory copy consistent
      } catch { /* non-fatal */ }
    }
  }
}

// FR-002: Surface pending-state message to the user if update is incomplete.
let pendingStateMsg = '';
if (hasForge && projectCache && projectCache.updateStatus === 'pending') {
  const pendingMigrations = Array.isArray(projectCache.pendingMigrations)
    ? projectCache.pendingMigrations.join(', ')
    : '(unknown)';
  pendingStateMsg =
    `Forge update is incomplete — pending migration(s): ${pendingMigrations}.` +
    ` Run /forge:update to continue or /forge:init --migrate to complete.`;
}

const elapsed = pluginCache ? now - (pluginCache.lastCheck || 0) : Infinity;

// Build multi-plugin awareness message if multiple installations found.
let multiPluginMsg = '';
if (allInstallations.length >= 2) {
  const activeInst = allInstallations.find(i => i.path === pluginRoot) || allInstallations[0];
  const otherInsts = allInstallations.filter(i => i.path !== activeInst.path);
  if (otherInsts.length > 0) {
    const otherDesc = otherInsts.map(i => `${i.version} (${i.distribution}, ${i.scope})`).join(', ');
    multiPluginMsg = `Also installed: ${otherDesc}. `;
  }
}

if (elapsed < checkInterval) {
  // Plugin cache still fresh — use stored remote version.
  // Detect post-install: if the project's recorded localVersion differs from
  // the running plugin version, the plugin was updated since last migration.
  let postInstallMsg = '';
  if (hasForge && projectCache && projectCache.localVersion && projectCache.localVersion !== local) {
    // Record the pre-install version as baseline, update localVersion.
    // FR-010: Include forgeRef. FR-002: updateStatus/pendingReason/pendingMigrations
    // are preserved via spread.
    const updated = {
      ...projectCache,
      migratedFrom: projectCache.localVersion,
      localVersion: local,
      distribution: currentDistribution,
      forgeRoot: pluginRoot,
      forgeRef: local,
    };
    try { fs.writeFileSync(projectCacheFile, JSON.stringify(updated, null, 2) + '\n'); } catch { /* non-fatal */ }
    // Reset plugin cache lastCheck so we fetch a fresh remote version next session.
    try { fs.writeFileSync(pluginCacheFile, JSON.stringify({ ...pluginCache, lastCheck: 0 }, null, 2) + '\n'); } catch { /* non-fatal */ }
    // Suppress post-install message when a distribution switch message already covers the event.
    if (!distributionSwitchMsg) {
      postInstallMsg = `Forge was updated to ${local} (was ${projectCache.localVersion}). Run /forge:update to review changes and update.`;
    }
  }
  const baseMsg = distributionSwitchMsg || postInstallMsg || buildUpdateMsg((pluginCache && pluginCache.remoteVersion) || '', local);
  // FR-002: Append pending-state message if present.
  const updateMsg = baseMsg ? multiPluginMsg + baseMsg : baseMsg;
  const pendingMsg = pendingStateMsg ? ' ' + pendingStateMsg : '';
  emit(forgeContext, (updateMsg + pendingMsg).trim());
} else {
  // Plugin cache expired or missing — fetch fresh remote version.
  fetchRemoteVersion((remoteVersion) => {
    if (remoteVersion) {
      // Update plugin-level throttle cache.
      try { fs.writeFileSync(pluginCacheFile, JSON.stringify({ lastCheck: now, remoteVersion }, null, 2) + '\n'); } catch { /* non-fatal */ }
      // Seed project-level cache on first run if not yet present.
      // FR-010: Include forgeRef alongside forgeRoot.
      if (hasForge && !projectCache) {
        try {
          fs.writeFileSync(projectCacheFile, JSON.stringify({
            migratedFrom: local, localVersion: local,
            distribution: currentDistribution, forgeRoot: pluginRoot,
            forgeRef: local,
            updateStatus: 'complete', pendingReason: null, pendingMigrations: [],
          }, null, 2) + '\n');
        } catch { /* non-fatal */ }
      } else if (hasForge && projectCache && !projectCache.localVersion) {
        // Backfill localVersion (and distribution/forgeRoot/forgeRef) if missing.
        // FR-002: Preserve updateStatus/pendingReason/pendingMigrations via spread.
        try {
          fs.writeFileSync(projectCacheFile, JSON.stringify({
            ...projectCache, localVersion: local,
            distribution: currentDistribution, forgeRoot: pluginRoot,
            forgeRef: local,
          }, null, 2) + '\n');
        } catch { /* non-fatal */ }
      }
    }
    const baseMsg = distributionSwitchMsg || buildUpdateMsg(remoteVersion, local);
    // FR-002: Append pending-state message if present.
    const updateMsg = baseMsg ? multiPluginMsg + baseMsg : baseMsg;
    const pendingMsg = pendingStateMsg ? ' ' + pendingStateMsg : '';
    emit(forgeContext, (updateMsg + pendingMsg).trim());
  });
}
}

// Re-export from lib modules for backward compatibility with existing tests
// that import these functions from check-update directly.
// Canonical sources: hooks/lib/plugin-detection.cjs and hooks/lib/update-url.cjs
module.exports = {
  ...require('./lib/plugin-detection.cjs'),  // detectDistribution, scanPluginInstallations, isPluginEnabled
  ...require('./lib/update-url.cjs'),         // validateUpdateUrl, getUpdateUrl, FALLBACK_UPDATE_URL, ALLOWED_DOMAINS
};
