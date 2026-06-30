'use strict';
// store-facade.cjs — StoreFacade, extractExcerpt, loadForgeConfig
// Used by store-query.cjs. No dependency on store.cjs or schemas.

const fs = require('fs');
const path = require('path');
const { readJson } = require('./json-io.cjs');
const { deriveSlug } = require('./slug.cjs');

class StoreFacade {
  constructor(storeDir) {
    this.storeDir = storeDir;
  }

  _loadDir(dir) {
    const full = path.join(this.storeDir, dir);
    if (!fs.existsSync(full)) return [];
    return fs.readdirSync(full)
      .filter(f => f.endsWith('.json'))
      .map(f => {
        try { return readJson(path.join(full, f)); }
        catch { return null; }
      })
      .filter(Boolean);
  }

  listSprints(filter = {}) {
    return this._filterEntities(this._loadDir('sprints'), filter);
  }

  listTasks(filter = {}) {
    return this._filterEntities(
      this._loadDir('tasks').filter(e => e.taskId && !e.taskId.includes('BUG')),
      filter
    );
  }

  listBugs(filter = {}) {
    return this._filterEntities(this._loadDir('bugs'), filter);
  }

  listFeatures(filter = {}) {
    return this._filterEntities(this._loadDir('features'), filter);
  }

  getEntity(type, id) {
    const dir = { tasks: 'tasks', bugs: 'bugs', sprints: 'sprints', features: 'features' }[type];
    if (!dir) return null;
    const filePath = path.join(this.storeDir, dir, `${id}.json`);
    try { return readJson(filePath); }
    catch { return null; }
  }

  followFK(entity, fkField) {
    const val = entity[fkField];
    if (!val) return null;
    const fkMap = {
      sprintId:  'sprints',
      featureId: 'features',
      blockedBy: 'bugs',
      blocksTask: 'tasks',
      taskId:    'tasks',
      bugId:     'bugs',
    };
    const targetType = fkMap[fkField];
    if (!targetType) return null;
    if (Array.isArray(val)) {
      return val.map(v => this.getEntity(targetType, v)).filter(Boolean);
    }
    return this.getEntity(targetType, val);
  }

  _filterEntities(entities, filter) {
    return entities.filter(e => {
      for (const [key, val] of Object.entries(filter)) {
        if (Array.isArray(val)) {
          if (!val.includes(e[key])) return false;
        } else if (e[key] !== val) {
          return false;
        }
      }
      return true;
    });
  }
}

function extractExcerpt(indexPath, maxSentences = 4) {
  if (!indexPath || !fs.existsSync(indexPath)) return null;
  try {
    const content = fs.readFileSync(indexPath, 'utf8');
    const body = content.replace(/^---[\s\S]*?---\n*/, '');
    const lines = body.split('\n')
      .map(l => l.trim())
      .filter(l =>
        l &&
        !l.startsWith('#') &&
        !l.startsWith('<!--') &&
        !l.startsWith('|') &&
        !l.startsWith('---') &&
        !l.startsWith('**Status**') &&
        !l.startsWith('**Sprint**') &&
        !l.startsWith('[Back to')
      );
    const text = lines.join(' ');
    const sentences = text.match(/[^.!?]+[.!?]+/g) || [text];
    return sentences.slice(0, maxSentences).join(' ').trim() || null;
  } catch {
    return null;
  }
}

let _cfgCache = null;
function loadForgeConfig(cwd) {
  if (_cfgCache) return _cfgCache;
  const root = cwd || process.cwd();
  const configPath = path.join(root, '.forge', 'config.json');
  let cfg = {};
  if (fs.existsSync(configPath)) {
    try { cfg = readJson(configPath) || {}; } catch {}
  }
  const prefix = cfg.project?.prefix || 'WI';
  const kbRel = cfg.paths?.engineering || 'engineering';
  const storeRel = cfg.paths?.store || '.forge/store';
  _cfgCache = {
    prefix,
    kbPath: fs.existsSync(path.join(root, kbRel)) ? kbRel : (fs.existsSync(path.join(root, 'engineering')) ? 'engineering' : null),
    storePathRel: storeRel,
    storePathAbs: path.join(root, storeRel),
    projectName: cfg.project?.name || null,
  };
  return _cfgCache;
}

function resetConfigCache() {
  _cfgCache = null;
}

function findIndexPath(entity, kbPath) {
  if (entity.path) {
    const p = entity.path.replace(/\/$/, '');
    return `${p}/INDEX.md`;
  }
  if (!kbPath) return null;
  if (entity.sprintId && !entity.taskId && !entity.bugId) {
    return path.join(kbPath, 'sprints', entity.sprintId, 'INDEX.md');
  }
  if (entity.taskId) {
    const match = entity.taskId.match(/-S(\d+)-/);
    if (match) {
      const taskNum = entity.taskId.split('-').pop().replace('T', 'task_');
      return path.join(kbPath, 'sprints', `S${match[1]}`, 'tasks', taskNum, 'INDEX.md');
    }
  }
  if (entity.bugId) {
    const slug = entity.title
      ? deriveSlug(entity.title, { maxLen: 30 })
      : entity.bugId;
    return path.join(kbPath, 'bugs', `${entity.bugId}-${slug}`, 'INDEX.md');
  }
  return null;
}

module.exports = { StoreFacade, extractExcerpt, loadForgeConfig, resetConfigCache, findIndexPath };
