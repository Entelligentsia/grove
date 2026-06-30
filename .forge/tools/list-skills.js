#!/usr/bin/env node
// Forge skill query helper
//
// Usage:
//   node list-skills.js                  — print all available skill names (one per line)
//   node list-skills.js <skill-name>     — exit 0 if available, exit 1 if not
//
// Sources checked:
//   ~/.claude/plugins/installed_plugins.json  — marketplace plugins
//     scope "user"  → always available
//     scope "local" → only if projectPath matches cwd
//   ~/.claude/skills/                          — personal skills (subdirs with SKILL.md)
//
// Uses only Node.js built-ins — no npm dependencies required.
// Works on Linux, macOS, and Windows wherever Claude Code runs.

'use strict';

// On unexpected failure, exit 0 so hook callers degrade gracefully
// (assume skill absent / no output) rather than propagating an error.
process.on('uncaughtException', () => process.exit(0));

const fs = require('fs');
const path = require('path');
const os = require('os');

const pluginsFile = process.env.CLAUDE_PLUGIN_DATA_ROOT
  ? path.join(process.env.CLAUDE_PLUGIN_DATA_ROOT, 'installed_plugins.json')
  : path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json');

const personalSkillsDir = process.env.CLAUDE_SKILLS_DIR
  || path.join(os.homedir(), '.claude', 'skills');

const cwd = process.cwd();
const skills = new Set();

// Source 1: marketplace plugins from installed_plugins.json
if (fs.existsSync(pluginsFile)) {
  try {
    const data = JSON.parse(fs.readFileSync(pluginsFile, 'utf8'));
    for (const [key, installs] of Object.entries(data.plugins || {})) {
      for (const install of installs) {
        const isUser = install.scope === 'user';
        const isLocal = install.scope === 'local' && install.projectPath === cwd;
        if (isUser || isLocal) {
          skills.add(key.split('@')[0]);
        }
      }
    }
  } catch { /* non-fatal */ }
}

// Source 2: personal skills — subdirectories containing a SKILL.md
if (fs.existsSync(personalSkillsDir)) {
  try {
    for (const entry of fs.readdirSync(personalSkillsDir, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const skillMd = path.join(personalSkillsDir, entry.name, 'SKILL.md');
      if (fs.existsSync(skillMd)) {
        skills.add(entry.name);
      }
    }
  } catch { /* non-fatal */ }
}

const sorted = [...skills].sort();
const query = process.argv[2];

if (!query) {
  // Print all available skill names
  if (sorted.length > 0) process.stdout.write(sorted.join('\n') + '\n');
  process.exit(0);
} else {
  // Exit 0 if skill is available, 1 if not
  process.exit(sorted.includes(query) ? 0 : 1);
}
