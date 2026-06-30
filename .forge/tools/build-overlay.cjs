#!/usr/bin/env node
'use strict';

// Forge tool: build-overlay
// Materializes a per-spawn PROJECT_OVERLAY from task/bug record + MASTER_INDEX slice.
// Usage: node build-overlay.cjs --task <TASK_ID> [--bug <BUG_ID>] [--format json|md]

const fs = require('fs');
const path = require('path');

const PHASE_ORDER = ['plan', 'review_plan', 'implementation', 'code_review', 'validation'];

try {
  main();
} catch (err) {
  console.error('build-overlay error:', err.message);
  process.exit(1);
}

function main() {
  const args = process.argv.slice(2);

  if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
    console.error('Usage: node build-overlay.cjs --task <TASK_ID> [--bug <BUG_ID>] [--format json|md]');
    process.exit(1);
  }

  const taskId = argValue(args, '--task');
  const bugId  = argValue(args, '--bug');
  const format = argValue(args, '--format') || 'json';

  if (!taskId && !bugId) {
    console.error('Error: --task <TASK_ID> or --bug <BUG_ID> required');
    process.exit(1);
  }

  const config = loadConfig();
  const overlay = taskId
    ? buildTaskOverlay(taskId, config)
    : buildBugOverlay(bugId, config);

  validateOverlay(overlay);

  if (format === 'md') {
    process.stdout.write(renderMarkdown(overlay));
  } else {
    process.stdout.write(JSON.stringify(overlay, null, 2) + '\n');
  }
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

function loadConfig() {
  let configPath = path.join('.forge', 'config.json');
  if (!fs.existsSync(configPath)) {
    // Try parent directories (for development/nested invocation)
    const parentPath = path.join('..', '.forge', 'config.json');
    if (fs.existsSync(parentPath)) configPath = parentPath;
  }

  let config = {};
  try {
    config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  } catch (_) {}

  return {
    storeRoot: config.paths?.store || '.forge/store',
    engineeringRoot: config.paths?.engineering || 'engineering',
    commands: config.commands || {},
    projectPrefix: derivePrefix(config),
  };
}

function derivePrefix(config) {
  // Derive the project prefix from config or directory name
  const cwd = process.cwd();
  const dirname = path.basename(cwd).toUpperCase().replace(/[^A-Z0-9-]/g, '-');
  // Return first segment that looks like a prefix (upper-case short token)
  return dirname.split('-')[0] || 'PROJ';
}

// ---------------------------------------------------------------------------
// Task overlay builder
// ---------------------------------------------------------------------------

function buildTaskOverlay(taskId, config) {
  const taskPath = path.join(config.storeRoot, 'tasks', `${taskId}.json`);
  if (!fs.existsSync(taskPath)) {
    // FR-015: Exit 1 for "task not found" per CLI convention (non-zero = error).
    // This is intentional — the caller must know the task ID was invalid.
    throw new Error(`Task not found: ${taskId} (looked at ${taskPath})`);
  }

  const task = JSON.parse(fs.readFileSync(taskPath, 'utf8'));
  const indexSlice = extractIndexSlice(taskId, task.sprintId || '', config.storeRoot);
  const lastPhaseSummary = extractLastPhaseSummary(task);

  const overlay = {
    projectPrefix: config.projectPrefix,
    sprintId:      task.sprintId || '',
    sprintDir:     sprintDirFromId(task.sprintId || ''),
    taskId:        task.taskId,
    taskDir:       task.path || '',
    taskStatus:    task.status || '',
    storeRoot:     config.storeRoot,
    indexSlice,
    toolCommands:  config.commands,
  };

  if (lastPhaseSummary) {
    overlay.lastPhaseSummary = lastPhaseSummary;
  }

  return overlay;
}

// ---------------------------------------------------------------------------
// Bug overlay builder
// ---------------------------------------------------------------------------

function buildBugOverlay(bugId, config) {
  const bugPath = path.join(config.storeRoot, 'bugs', `${bugId}.json`);
  if (!fs.existsSync(bugPath)) {
    throw new Error(`Bug not found: ${bugId} (looked at ${bugPath})`);
  }

  const bug = JSON.parse(fs.readFileSync(bugPath, 'utf8'));
  const indexSlice = extractBugIndexSlice(bugId, config.storeRoot);
  const lastPhaseSummary = extractLastPhaseSummary(bug);

  const overlay = {
    projectPrefix: config.projectPrefix,
    bugId:         bug.bugId,
    bugDir:        bug.path || '',
    storeRoot:     config.storeRoot,
    indexSlice,
    toolCommands:  config.commands,
  };

  if (lastPhaseSummary) {
    overlay.lastPhaseSummary = lastPhaseSummary;
  }

  return overlay;
}

// ---------------------------------------------------------------------------
// Index slice extraction (queries the store directly — MASTER_INDEX.md is a
// downstream view and not the source of truth)
// ---------------------------------------------------------------------------

function extractIndexSlice(taskId, sprintId, storeRoot) {
  const tasksDir = path.join(storeRoot, 'tasks');
  if (!sprintId || !fs.existsSync(tasksDir)) return '';

  const siblings = [];
  for (const entry of fs.readdirSync(tasksDir)) {
    if (!entry.endsWith('.json')) continue;
    try {
      const rec = JSON.parse(fs.readFileSync(path.join(tasksDir, entry), 'utf8'));
      if (rec.sprintId === sprintId) siblings.push(rec);
    } catch (_) { /* skip unreadable */ }
  }
  if (!siblings.length) return '';

  siblings.sort((a, b) => (a.taskId || '').localeCompare(b.taskId || ''));

  const lines = [
    `**Sprint ${sprintId} tasks:**`,
    '| Task | Title | Status |',
    '|------|-------|--------|',
  ];
  for (const t of siblings) {
    const marker = t.taskId === taskId ? ' ← this' : '';
    const title = (t.title || '').replace(/\|/g, '\\|').slice(0, 60);
    lines.push(`| ${t.taskId}${marker} | ${title} | ${t.status || ''} |`);
  }

  return budget(lines.join('\n'), 800);
}

function extractBugIndexSlice(bugId, storeRoot) {
  const bugsDir = path.join(storeRoot, 'bugs');
  if (!fs.existsSync(bugsDir)) return '';

  const open = [];
  let target = null;
  for (const entry of fs.readdirSync(bugsDir)) {
    if (!entry.endsWith('.json')) continue;
    try {
      const rec = JSON.parse(fs.readFileSync(path.join(bugsDir, entry), 'utf8'));
      if (rec.bugId === bugId) target = rec;
      else if (rec.status && !['fixed', 'verified'].includes(rec.status)) open.push(rec);
    } catch (_) { /* skip */ }
  }

  const rows = [];
  if (target) rows.push(target);
  open.sort((a, b) => (a.bugId || '').localeCompare(b.bugId || ''));
  rows.push(...open);
  if (!rows.length) return '';

  const lines = [
    '**Active bugs:**',
    '| Bug | Title | Severity | Status |',
    '|-----|-------|----------|--------|',
  ];
  for (const b of rows) {
    const marker = b.bugId === bugId ? ' ← this' : '';
    const title = (b.title || '').replace(/\|/g, '\\|').slice(0, 50);
    lines.push(`| ${b.bugId}${marker} | ${title} | ${b.severity || ''} | ${b.status || ''} |`);
  }

  return budget(lines.join('\n'), 800);
}

function budget(text, max) {
  return text.length <= max ? text : text.slice(0, max - 3) + '...';
}

// ---------------------------------------------------------------------------
// Phase summary helpers
// ---------------------------------------------------------------------------

function extractLastPhaseSummary(record) {
  if (!record.summaries) return null;
  // Find the last phase with a summary in canonical order
  let last = null;
  let lastPhase = null;
  for (const phase of PHASE_ORDER) {
    if (record.summaries[phase]) {
      last = record.summaries[phase];
      lastPhase = phase;
    }
  }
  if (!last) return null;
  return { phase: lastPhase, ...last };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function argValue(args, flag) {
  const idx = args.indexOf(flag);
  if (idx === -1) return null;
  return args[idx + 1] || null;
}

function sprintDirFromId(sprintId) {
  if (!sprintId) return '';
  return sprintId;
}

// ---------------------------------------------------------------------------
// Schema validation
// ---------------------------------------------------------------------------

function validateOverlay(overlay) {
  const schemaPath = path.join(__dirname, '..', 'schemas', 'project-overlay.schema.json');
  if (!fs.existsSync(schemaPath)) return; // schema optional at dev time

  const schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8'));
  const errors = [];

  for (const req of (schema.required || [])) {
    if (overlay[req] === undefined) {
      errors.push(`missing required field: ${req}`);
    }
  }

  if (overlay.indexSlice && overlay.indexSlice.length > 800) {
    errors.push(`indexSlice exceeds 800 chars (${overlay.indexSlice.length})`);
  }

  if (errors.length > 0) {
    throw new Error(`Overlay schema validation failed:\n${errors.join('\n')}`);
  }
}

// ---------------------------------------------------------------------------
// Markdown rendering
// ---------------------------------------------------------------------------

function renderMarkdown(overlay) {
  const lines = ['### Project Overlay\n'];

  if (overlay.taskId) {
    lines.push(`**Task:** ${overlay.taskId} (${overlay.taskStatus || 'unknown'})`);
    lines.push(`**Sprint:** ${overlay.sprintId || 'unknown'}`);
    lines.push(`**Task dir:** ${overlay.taskDir || 'unknown'}`);
  } else if (overlay.bugId) {
    lines.push(`**Bug:** ${overlay.bugId}`);
    lines.push(`**Bug dir:** ${overlay.bugDir || 'unknown'}`);
  }

  lines.push(`**Store root:** ${overlay.storeRoot}`);

  if (overlay.indexSlice) {
    lines.push('\n**Sprint context (from index):**');
    lines.push('```');
    lines.push(overlay.indexSlice);
    lines.push('```');
  }

  if (overlay.lastPhaseSummary) {
    const s = overlay.lastPhaseSummary;
    lines.push(`\n**Last phase (${s.phase}):** ${s.objective || ''}`);
    if (s.key_changes && s.key_changes.length > 0) {
      lines.push('Key changes:');
      for (const c of s.key_changes.slice(0, 5)) {
        lines.push(`- ${c}`);
      }
    }
  }

  if (overlay.toolCommands) {
    const cmds = overlay.toolCommands;
    if (cmds.test) lines.push(`\n**Test command:** \`${cmds.test}\``);
    if (cmds.lint) lines.push(`**Lint command:** \`${cmds.lint}\``);
  }

  return lines.join('\n') + '\n';
}
