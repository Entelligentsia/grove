'use strict';

const fs = require('fs');
const path = require('path');
const { findProjectRoot } = require('./lib/project-root.cjs');
const { readJson, writeJson } = require('./lib/json-io.cjs');

/**
 * Store Facade for Forge
 * Provides a backend-agnostic interface for CRUD operations on core store entities.
 */
class Store {
  constructor(implementation) {
    this.impl = implementation;
  }

  // --- Sprints ---
  getSprint(id) { return this.impl.getSprint(id); }
  listSprints(filter) { return this.impl.listSprints(filter); }
  writeSprint(data) { return this.impl.writeSprint(data); }
  deleteSprint(id) { return this.impl.deleteSprint(id); }

  // --- Tasks ---
  getTask(id) { return this.impl.getTask(id); }
  listTasks(filter) { return this.impl.listTasks(filter); }
  writeTask(data) { return this.impl.writeTask(data); }
  deleteTask(id) { return this.impl.deleteTask(id); }

  // --- Bugs ---
  getBug(id) { return this.impl.getBug(id); }
  listBugs(filter) { return this.impl.listBugs(filter); }
  writeBug(data) { return this.impl.writeBug(data); }
  deleteBug(id) { return this.impl.deleteBug(id); }

  // --- Events ---
  getEvent(id, sprintId) { return this.impl.getEvent(id, sprintId); }
  listEvents(sprintId, filter) { return this.impl.listEvents(sprintId, filter); }
  writeEvent(sprintId, data) { return this.impl.writeEvent(sprintId, data); }
  deleteEvent(id, sprintId) { return this.impl.deleteEvent(id, sprintId); }
  renameEvent(sprintId, oldFilename, newEventId) { return this.impl.renameEvent(sprintId, oldFilename, newEventId); }

  // --- Features ---
  getFeature(id) { return this.impl.getFeature(id); }
  listFeatures(filter) { return this.impl.listFeatures(filter); }
  writeFeature(data) { return this.impl.writeFeature(data); }
  deleteFeature(id) { return this.impl.deleteFeature(id); }

  // --- Collation State ---
  writeCollationState(data) { return this.impl.writeCollationState(data); }
  readCollationState() { return this.impl.readCollationState(); }

  // --- Event Operations (extended) ---
  /**
   * Purge all event files for a sprint directory.
   * @param {string} sprintId
   * @param {{ dryRun?: boolean }} opts - dryRun: return file list without deleting
   * @returns {{ purged: boolean, fileCount: number, files: string[] }}
   */
  purgeEvents(sprintId, opts) { return this.impl.purgeEvents(sprintId, opts); }
  purgeBugEvents(bugId, opts) { return this.impl.purgeBugEvents(bugId, opts); }
  /**
   * List event filenames for a sprint directory.
   * @param {string} sprintId
   * @returns {{ filename: string, id: string }[]}
   */
  listEventFilenames(sprintId) { return this.impl.listEventFilenames(sprintId); }
}

/**
 * Filesystem Implementation of the Store facade.
 * Manages JSON flat-files in the .forge/store directory.
 */
class FSImpl {
  constructor(configPath = '.forge/config.json') {
    this.configPath = configPath;
    this.storeRoot = this._resolveStoreRoot();
  }

  _resolveStoreRoot() {
    const configPathIsAbsolute = path.isAbsolute(this.configPath);
    const projectRoot = configPathIsAbsolute ? null : findProjectRoot();
    try {
      const resolved = projectRoot ? path.join(projectRoot, this.configPath) : this.configPath;
      const config = JSON.parse(fs.readFileSync(resolved, 'utf8'));
      const storePath = config.paths.store;
      return path.isAbsolute(storePath) ? storePath
        : projectRoot ? path.join(projectRoot, storePath) : storePath;
    } catch (e) {
      // Fallback to default if config is missing or corrupt
      return projectRoot ? path.join(projectRoot, '.forge', 'store') : '.forge/store';
    }
  }

  _getPath(entity, id) {
    const entityMap = {
      sprint: 'sprints',
      task: 'tasks',
      bug: 'bugs',
      event: 'events',
      feature: 'features'
    };
    const dir = entityMap[entity];
    if (!dir) throw new Error(`Unknown entity type: ${entity}`);
    return path.join(this.storeRoot, dir, `${id}.json`);
  }

  _readJson(filePath) {
    return readJson(filePath);
  }

  _writeJson(filePath, data) {
    return writeJson(filePath, data);
  }

  /**
   * Shared list helper: read all .json files in a store subdirectory and
   * apply an optional filter. Only top-level files are read (no recursion).
   *
   * NOTE: listEvents is intentionally excluded from this helper. It requires
   * an extra `sprintId` path segment and a different call shape, making it
   * impossible to express as a simple `_listEntities('events', filter)` call.
   * Do not attempt to unify listEvents here — the distinction is load-bearing.
   *
   * @param {string} subdir - subdirectory under storeRoot (e.g. 'sprints')
   * @param {object|null} filter - field equality filter, or null for all records
   * @returns {Array} matching records (null entries from parse failures included)
   */
  _listEntities(subdir, filter) {
    const dir = path.join(this.storeRoot, subdir);
    if (!fs.existsSync(dir)) return [];
    return fs.readdirSync(dir)
      .filter(f => f.endsWith('.json'))
      .map(f => this._readJson(path.join(dir, f)))
      .filter(r => !r || (filter ? this._matches(r, filter) : true));
  }

  // Sprints
  getSprint(id) { return this._readJson(this._getPath('sprint', id)); }
  listSprints(filter) { return this._listEntities('sprints', filter); }
  writeSprint(data) {
    return this._writeJson(this._getPath('sprint', data.sprintId), data);
  }
  deleteSprint(id) {
    const p = this._getPath('sprint', id);
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }

  // Tasks
  getTask(id) { return this._readJson(this._getPath('task', id)); }
  listTasks(filter) { return this._listEntities('tasks', filter); }
  writeTask(data) {
    return this._writeJson(this._getPath('task', data.taskId), data);
  }
  deleteTask(id) {
    const p = this._getPath('task', id);
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }

  // Bugs
  getBug(id) { return this._readJson(this._getPath('bug', id)); }
  listBugs(filter) { return this._listEntities('bugs', filter); }
  writeBug(data) {
    return this._writeJson(this._getPath('bug', data.bugId), data);
  }
  deleteBug(id) {
    const p = this._getPath('bug', id);
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }

  // Events
  getEvent(id, sprintId) {
    const p = path.join(this.storeRoot, 'events', sprintId, `${id}.json`);
    return this._readJson(p);
  }
  listEvents(sprintId, filter) {
    const dir = path.join(this.storeRoot, 'events', sprintId);
    if (!fs.existsSync(dir)) return [];
    return fs.readdirSync(dir)
      .filter(f => f.endsWith('.json'))
      .map(f => this._readJson(path.join(dir, f)))
      .filter(e => !e || (filter ? this._matches(e, filter) : true));
  }
  /**
   * Find an event file whose internal eventId matches the given eventId,
   * but whose filename does not. Returns the mismatched filename (without
   * .json extension), or null if none found. Skips _-prefixed (ephemeral) files.
   */
  _findEventFileByContentId(sprintId, eventId) {
    const dir = path.join(this.storeRoot, 'events', sprintId);
    if (!fs.existsSync(dir)) return null;
    const files = fs.readdirSync(dir).filter(f => f.endsWith('.json') && !f.startsWith('_'));
    for (const file of files) {
      const filename = file.slice(0, -5); // strip .json
      if (filename === eventId) continue; // already canonical
      const rec = this._readJson(path.join(dir, file));
      if (rec && rec.eventId === eventId) {
        return filename;
      }
    }
    return null;
  }
  /**
   * Rename an event file from oldFilename to match newEventId.
   * Throws if the target file already exists (collision).
   */
  renameEvent(sprintId, oldFilename, newEventId) {
    const dir = path.join(this.storeRoot, 'events', sprintId);
    const oldPath = path.join(dir, `${oldFilename}.json`);
    const newPath = path.join(dir, `${newEventId}.json`);
    if (oldPath === newPath) return; // no-op
    if (fs.existsSync(newPath)) {
      throw new Error(`Cannot rename event: target file already exists: ${newPath}`);
    }
    if (fs.existsSync(oldPath)) {
      fs.renameSync(oldPath, newPath);
    }
  }
  writeEvent(sprintId, data) {
    // Detect ghost file: an existing file whose content eventId matches but
    // whose filename does not. Rename it to the canonical name before writing.
    const ghostFilename = this._findEventFileByContentId(sprintId, data.eventId);
    if (ghostFilename !== null) {
      this.renameEvent(sprintId, ghostFilename, data.eventId);
    }
    const p = path.join(this.storeRoot, 'events', sprintId, `${data.eventId}.json`);
    return this._writeJson(p, data);
  }
  deleteEvent(id, sprintId) {
    const p = path.join(this.storeRoot, 'events', sprintId, `${id}.json`);
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }

  // Features
  getFeature(id) { return this._readJson(this._getPath('feature', id)); }
  listFeatures(filter) { return this._listEntities('features', filter); }
  writeFeature(data) {
    // FORGE-BUG-047: feature records key on `id` (matching the schema, the
    // store-cli template, and every FEAT-00x record) — not `feature_id`, which
    // was always undefined and misfiled writes to features/undefined.json.
    return this._writeJson(this._getPath('feature', data.id), data);
  }
  deleteFeature(id) {
    const p = this._getPath('feature', id);
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }

  // Collation State
  writeCollationState(data) {
    const filePath = path.join(this.storeRoot, 'COLLATION_STATE.json');
    return this._writeJson(filePath, data);
  }
  readCollationState() {
    const filePath = path.join(this.storeRoot, 'COLLATION_STATE.json');
    return this._readJson(filePath);
  }

  // Event Operations (extended)
  /**
   * Purge the events directory for a given sprint.
   * Includes a path-traversal guard: the resolved directory must remain
   * within the events base directory. Throws on escape attempt.
   * Note: fileCount reflects .json files only, but the directory is removed
   * entirely (including any non-.json files).
   */
  purgeEvents(sprintId, { dryRun = false } = {}) {
    const eventsBase = path.resolve(this.storeRoot, 'events');
    const eventsDir = path.resolve(eventsBase, sprintId);
    // Guard: resolved path must stay within the events base directory.
    if (!eventsDir.startsWith(eventsBase + path.sep) && eventsDir !== eventsBase) {
      throw new Error(`Resolved events path '${eventsDir}' escapes store root — aborting purge`);
    }
    if (!fs.existsSync(eventsDir)) {
      return { purged: false, fileCount: 0, files: [] };
    }
    const files = fs.readdirSync(eventsDir).filter(f => f.endsWith('.json'));
    if (dryRun) {
      return { purged: false, fileCount: files.length, files };
    }
    fs.rmSync(eventsDir, { recursive: true, force: true });
    return { purged: true, fileCount: files.length, files };
  }

  /**
   * Purge only the events belonging to a specific bug from the shared
   * `.forge/store/events/bugs/` virtual sprint dir.
   *
   * Bugs share a single events directory (`events/bugs/`) — see
   * `meta-fix-bug.md § Event Emission` and `validate-store.spec.md`. Purging
   * by bug therefore must filter primary events by `event.bugId === bugId`
   * and sweep sidecars whose filename pattern `_{eventId}_usage.json`
   * matches a purged primary. The `events/bugs/` directory itself is
   * never removed — other bugs' events remain.
   *
   * Returns `{ purged, fileCount, files }` matching `purgeEvents`.
   */
  purgeBugEvents(bugId, { dryRun = false } = {}) {
    const eventsBugsDir = path.join(this.storeRoot, 'events', 'bugs');
    if (!fs.existsSync(eventsBugsDir)) {
      return { purged: false, fileCount: 0, files: [] };
    }

    const all = fs.readdirSync(eventsBugsDir).filter(f => f.endsWith('.json'));
    const primaries = all.filter(f => !f.startsWith('_'));
    const sidecars  = all.filter(f =>  f.startsWith('_') && f.endsWith('_usage.json'));

    // Identify primaries whose payload.bugId matches the requested bug.
    const matchedPrimaries = [];
    const matchedEventIds = new Set();
    for (const filename of primaries) {
      const filePath = path.join(eventsBugsDir, filename);
      let payload;
      try {
        payload = JSON.parse(fs.readFileSync(filePath, 'utf8'));
      } catch (_) {
        continue; // malformed file — skip
      }
      if (payload && payload.bugId === bugId) {
        matchedPrimaries.push(filename);
        if (payload.eventId) matchedEventIds.add(payload.eventId);
      }
    }

    // Match sidecars by filename pattern: `_{eventId}_usage.json`.
    const matchedSidecars = sidecars.filter(filename => {
      const m = filename.match(/^_(.+)_usage\.json$/);
      return m && matchedEventIds.has(m[1]);
    });

    const allMatches = [...matchedPrimaries, ...matchedSidecars];
    if (dryRun) {
      return { purged: false, fileCount: allMatches.length, files: allMatches };
    }
    for (const filename of allMatches) {
      fs.unlinkSync(path.join(eventsBugsDir, filename));
    }
    return {
      purged: allMatches.length > 0,
      fileCount: allMatches.length,
      files: allMatches,
    };
  }

  /**
   * List all event filenames for a sprint directory.
   * Returns { filename, id } objects for ALL .json files including
   * _-prefixed ephemeral sidecars. Callers filter internally.
   */
  listEventFilenames(sprintId) {
    const dir = path.join(this.storeRoot, 'events', sprintId);
    if (!fs.existsSync(dir)) return [];
    return fs.readdirSync(dir)
      .filter(f => f.endsWith('.json'))
      .map(f => ({
        filename: f,
        id: f.slice(0, -5) // strip .json extension
      }));
  }

  _matches(record, filter) {
    if (!filter) return true;
    return Object.entries(filter).every(([key, value]) => record[key] === value);
  }
}

// Export a singleton instance for the plugin, plus classes for testing
module.exports = new Store(new FSImpl());
module.exports.Store = Store;
module.exports.FSImpl = FSImpl;
