#!/usr/bin/env node
'use strict';

// Forge tool: seed-store
// Bootstrap the JSON store from an existing engineering/ directory structure.
// Supports slug-named directories (e.g., FORGE-S06-T07-slug-aware-seed-store/)
// as well as legacy bare-ID directories (e.g., S01/, T01/, B01/).
// Usage: seed-store [--dry-run]

const fs = require('fs');
const path = require('path');
const Store = require('./store.cjs');
const { deriveSlug } = require('./lib/slug.cjs');

const DRY_RUN = process.argv.includes('--dry-run');

// ── Exports for testing ────────────────────────────────────────────────────────

module.exports = { deriveSlug };

// ── CLI (only when run directly) ──────────────────────────────────────────────

if (require.main === module) {
const cwd = process.cwd();

function extractTitle(dir, fallback) {
  for (const file of ['PLAN.md', 'PROGRESS.md', 'INDEX.md', 'README.md']) {
    const p = path.join(dir, file);
    if (!fs.existsSync(p)) continue;
    const m = fs.readFileSync(p, 'utf8').match(/^#\s+(.+)/m);
    if (m) return m[1].trim();
  }
  return fallback;
}

function inferTaskStatus(taskDir) {
  const p = path.join(taskDir, 'PROGRESS.md');
  if (!fs.existsSync(p)) return 'planned';
  const content = fs.readFileSync(p, 'utf8').toLowerCase();
  if (content.includes('committed')) return 'committed';
  if (content.includes('approved')) return 'approved';
  if (content.includes('implemented')) return 'implemented';
  if (content.includes('implementing')) return 'implementing';
  return 'planned';
}

function inferSprintStatus(sprintPath, taskDirs) {
  if (taskDirs.length === 0) return 'planning';
  const allCommitted = taskDirs.every(t => inferTaskStatus(path.join(sprintPath, t)) === 'committed');
  return allCommitted ? 'completed' : 'active';
}

try {
  const config = JSON.parse(fs.readFileSync(path.join(cwd, '.forge', 'config.json'), 'utf8'));
  const prefix    = config.project?.prefix || 'PROJ';
  const engPath   = config.paths?.engineering || 'engineering';

  const sprintsDir = path.join(cwd, engPath, 'sprints');
  const bugsDir    = path.join(cwd, engPath, 'bugs');
  const featuresDir = path.join(cwd, engPath, 'features');

  let sprintCount = 0, taskCount = 0, bugCount = 0;

  // --- Scaffold features/ ---
  if (!DRY_RUN) {
    if (!fs.existsSync(featuresDir)) {
      fs.mkdirSync(featuresDir, { recursive: true });
    }
  } else {
    if (!fs.existsSync(featuresDir)) {
      console.log(`[dry-run] would scaffold directory: ${path.relative(cwd, featuresDir)}/`);
    }
  }

  // --- Sprints and Tasks ---
  if (fs.existsSync(sprintsDir)) {
    // Three-tier sprint discovery:
    // 1. {PREFIX}-S{NN}-*  (slug-named, e.g. FORGE-S06-post-07-feedback)
    // 2. {PREFIX}-S{NN}    (full ID, no slug, e.g. FORGE-S06)
    // 3. S{NN}             (bare legacy, e.g. S01)
    const prefixEscaped = prefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const slugSprintRe = new RegExp(`^${prefixEscaped}-(S\\d+)-.+$`, 'i');
    const fullSprintRe = new RegExp(`^${prefixEscaped}-(S\\d+)$`, 'i');
    const bareSprintRe  = /^S\d+$/i;

    const sprintDirs = fs.readdirSync(sprintsDir)
      .filter(e => fs.statSync(path.join(sprintsDir, e)).isDirectory())
      .map(e => {
        let match;
        if ((match = e.match(slugSprintRe))) {
          return { dirName: e, sprintNum: match[1], sortKey: parseInt(match[1].slice(1), 10) };
        }
        if ((match = e.match(fullSprintRe))) {
          return { dirName: e, sprintNum: match[1], sortKey: parseInt(match[1].slice(1), 10) };
        }
        if ((match = e.match(bareSprintRe))) {
          return { dirName: e, sprintNum: match[0].toUpperCase(), sortKey: parseInt(match[0].slice(1), 10) };
        }
        return null;
      })
      .filter(Boolean)
      .sort((a, b) => a.sortKey - b.sortKey);

    for (const { dirName: sprintDir, sprintNum } of sprintDirs) {
      const sprintFullPath = path.join(sprintsDir, sprintDir);
      const sprintId = `${prefix}-${sprintNum.toUpperCase()}`;

      // Three-tier task discovery:
      // 1. T{NN}-*                (bare task ID with slug, e.g. T01-fix-persona-lookup)
      // 2. {PREFIX}-S{NN}-T{NN}-* (full task ID with slug, e.g. FORGE-S06-T01-fix-persona)
      // 3. T{NN}                  (bare legacy, e.g. T01)
      const bareTaskSlugRe  = new RegExp(`^T(\\d+)-.+$`, 'i');
      const fullTaskSlugRe  = new RegExp(`^${prefixEscaped}-${sprintNum.toUpperCase()}-T(\\d+)-.+$`, 'i');
      const bareTaskRe      = /^T(\d+)$/i;

      const taskDirs = fs.readdirSync(sprintFullPath)
        .filter(e => fs.statSync(path.join(sprintFullPath, e)).isDirectory())
        .map(e => {
          let match;
          if ((match = e.match(fullTaskSlugRe))) {
            return { dirName: e, taskNum: match[1], sortKey: parseInt(match[1], 10) };
          }
          if ((match = e.match(bareTaskSlugRe))) {
            return { dirName: e, taskNum: match[1], sortKey: parseInt(match[1], 10) };
          }
          if ((match = e.match(bareTaskRe))) {
            return { dirName: e, taskNum: String(parseInt(match[1], 10)), sortKey: parseInt(match[1], 10) };
          }
          return null;
        })
        .filter(Boolean)
        .sort((a, b) => a.sortKey - b.sortKey);

      if (process.env.DEBUG_SEED) console.log(`DEBUG ${sprintId} taskDirs:`, JSON.stringify(taskDirs));
      const taskIds = taskDirs.map(t => `${prefix}-${sprintNum.toUpperCase()}-T${t.taskNum.padStart(2, '0')}`);

      if (DRY_RUN) {
        console.log(`[dry-run] would write sprint: ${sprintId}`);
      } else {
        Store.writeSprint({
          sprintId,
          title: extractTitle(sprintFullPath, `Sprint ${sprintNum}`),
          status: inferSprintStatus(sprintFullPath, taskDirs.map(t => t.dirName)),
          taskIds,
          createdAt: new Date().toISOString(),
          path: path.join(engPath, 'sprints', sprintDir),
        });
      }
      sprintCount++;

      for (const { dirName: taskDir, taskNum } of taskDirs) {
        const taskFullPath = path.join(sprintFullPath, taskDir);
        const taskId = `${prefix}-${sprintNum.toUpperCase()}-T${taskNum.padStart(2, '0')}`;
        if (DRY_RUN) {
          console.log(`[dry-run] would write task: ${taskId}`);
        } else {
          Store.writeTask({
            taskId,
            sprintId,
            title: extractTitle(taskFullPath, `Task T${taskNum}`),
            status: inferTaskStatus(taskFullPath),
            path: path.join(engPath, 'sprints', sprintDir, taskDir),
          });
        }
        taskCount++;
      }
    }
  }

  // --- Bugs ---
  if (fs.existsSync(bugsDir)) {
    // Three-tier bug discovery:
    // 1. {PREFIX}-BUG-{NNN}-*  (full slug, e.g. FORGE-BUG-001-sprint-runner-context)
    // 2. BUG-{NNN}-*           (partial slug, e.g. BUG-001-sprint-runner-context)
    // 3. B{NN}                 (bare legacy, e.g. B01)
    const prefixEscaped = prefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const fullSlugBugRe = new RegExp(`^${prefixEscaped}-BUG-(\\d+)-.+$`, 'i');
    const partialSlugBugRe = /^BUG-(\d+)-.+$/i;
    const bareBugRe = /^B(\d+)$/i;

    const bugDirs = fs.readdirSync(bugsDir)
      .filter(e => fs.statSync(path.join(bugsDir, e)).isDirectory())
      .map(e => {
        let match;
        if ((match = e.match(fullSlugBugRe))) {
          return { dirName: e, bugNum: match[1], sortKey: parseInt(match[1], 10) };
        }
        if ((match = e.match(partialSlugBugRe))) {
          return { dirName: e, bugNum: match[1], sortKey: parseInt(match[1], 10) };
        }
        if ((match = e.match(bareBugRe))) {
          return { dirName: e, bugNum: match[1], sortKey: parseInt(match[1], 10) };
        }
        return null;
      })
      .filter(Boolean)
      .sort((a, b) => a.sortKey - b.sortKey);

    for (const { dirName: bugDir, bugNum } of bugDirs) {
      const bugFullPath = path.join(bugsDir, bugDir);
      const bugId = `${prefix}-BUG-${bugNum.padStart(2, '0')}`;
      if (DRY_RUN) {
        console.log(`[dry-run] would write bug: ${bugId}`);
      } else {
        Store.writeBug({
          bugId,
          title: extractTitle(bugFullPath, `Bug ${bugNum}`),
          severity: 'minor',
          status: 'reported',
          path: path.join(engPath, 'bugs', bugDir),
          reportedAt: new Date().toISOString(),
        });
      }
      bugCount++;
    }
  }

  const prefix_ = DRY_RUN ? '[dry-run] ' : '';
  console.log(`${prefix_}Seeded: ${sprintCount} sprint(s), ${taskCount} task(s), ${bugCount} bug(s)`);
} catch (e) {
  console.error(`Error seeding store: ${e.message}`);
  process.exit(1);
}

} // end if (require.main === module)
