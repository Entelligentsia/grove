#!/usr/bin/env node
'use strict';

// Forge tool: validate-store
// Check store integrity: required fields, types, enums, and referential integrity.
// Usage: validate-store [--dry-run] [--fix] [--json]

let _store;
function _getStore() { return _store || (_store = require('./store.cjs')); }

let _schemas;
function _getSchemas() {
  if (_schemas) return _schemas;
  const fs = require('fs');
  const path = require('path');

  const ENTITY_TYPES = ['sprint', 'task', 'bug', 'event', 'feature'];

  const MINIMAL_REQUIRED = {
    sprint:  ['sprintId', 'title', 'status', 'taskIds', 'createdAt'],
    task:    ['taskId', 'sprintId', 'title', 'status', 'path'],
    bug:     ['bugId', 'title', 'severity', 'status', 'path', 'reportedAt'],
    event:   ['eventId', 'taskId', 'sprintId', 'role', 'action', 'phase', 'iteration', 'startTimestamp', 'endTimestamp', 'durationMinutes', 'model'],
    feature: ['id', 'title', 'status', 'created_at']
  };

  const schemas = {};
  const projectDir   = path.join('.forge', 'schemas');
  const inTreeDir    = path.join('forge', 'schemas');
  const pluginDir    = path.join(__dirname, '..', 'schemas');

  for (const type of ENTITY_TYPES) {
    const schemaFile = `${type}.schema.json`;
    let schema = null;

    // 1. Try project-installed schemas first
    const projectPath = path.join(projectDir, schemaFile);
    try {
      if (fs.existsSync(projectPath)) {
        schema = JSON.parse(fs.readFileSync(projectPath, 'utf8'));
      }
    } catch (e) {
      console.error(`WARN: schema file ${projectPath} exists but could not be parsed: ${e.message}`);
    }

    // 2. Fall back to in-tree source schemas (development mode)
    if (!schema) {
      const inTreePath = path.join(inTreeDir, schemaFile);
      try {
        if (fs.existsSync(inTreePath)) {
          schema = JSON.parse(fs.readFileSync(inTreePath, 'utf8'));
        }
      } catch (e) {
        console.error(`WARN: schema file ${inTreePath} exists but could not be parsed: ${e.message}`);
      }
    }

    // 3. Fall back to plugin-installed schemas (production mode)
    //    validate-store.cjs lives at $FORGE_ROOT/tools/, so __dirname/../schemas/
    //    resolves to $FORGE_ROOT/schemas/ — always correct for installed plugins.
    if (!schema) {
      const pluginPath = path.join(pluginDir, schemaFile);
      try {
        if (fs.existsSync(pluginPath)) {
          schema = JSON.parse(fs.readFileSync(pluginPath, 'utf8'));
        }
      } catch (e) {
        console.error(`WARN: schema file ${pluginPath} exists but could not be parsed: ${e.message}`);
      }
    }

    if (schema) {
      schemas[type] = schema;
    } else {
      console.error(`WARN: schema file ${schemaFile} not found in ${projectDir}, ${inTreeDir}, or ${pluginDir}, using minimal fallback`);
      schemas[type] = { type: 'object', required: MINIMAL_REQUIRED[type] || [], properties: {} };
    }
  }

  _schemas = schemas;
  return _schemas;
}

// ---------------------------------------------------------------------------
// Constants (exported for testing)
// ---------------------------------------------------------------------------

const ENTITY_TYPES = ['sprint', 'task', 'bug', 'event', 'feature'];

// Non-entity schemas: referenced and parsed for validity but not used for store record validation.
const ANCILLARY_SCHEMAS = ['project-overlay', 'project-context', 'structure-versions'];

const MINIMAL_REQUIRED = {
  sprint:  ['sprintId', 'title', 'status', 'taskIds', 'createdAt'],
  task:    ['taskId', 'sprintId', 'title', 'status', 'path'],
  bug:     ['bugId', 'title', 'severity', 'status', 'path', 'reportedAt'],
  event:   ['eventId', 'taskId', 'sprintId', 'role', 'action', 'phase', 'iteration', 'startTimestamp', 'endTimestamp', 'durationMinutes', 'model'],
  feature: ['id', 'title', 'status', 'created_at']
};

// Fields that are legitimately null:
//   sprintId / taskId  — optional FK (e.g. standalone bug fix has no sprint)
//   endTimestamp / durationMinutes — not recorded on "start" events (phase opened but never closed)
const NULLABLE_FK = new Set(['sprintId', 'taskId', 'endTimestamp', 'durationMinutes']);

// --- Validation ---
function validateRecord(record, schema) {
  const errors = [];
  const required = schema.required || [];

  for (const field of required) {
    if (record[field] === undefined || record[field] === '') {
      errors.push({ category: 'missing-required', field, message: `missing required field: "${field}"`, value: record[field], expected: null });
    } else if (record[field] === null && !NULLABLE_FK.has(field)) {
      errors.push({ category: 'missing-required', field, message: `missing required field: "${field}"`, value: null, expected: null });
    }
  }

  for (const [field, def] of Object.entries(schema.properties || {})) {
    const val = record[field];
    if (val === undefined) continue;
    if (val === null) continue;

    if (def.type) {
      const typeMatches = (expected, actualVal) => {
        return expected === 'integer' ? Number.isInteger(actualVal)
             : expected === 'number'  ? typeof actualVal === 'number'
             : expected === 'array'   ? Array.isArray(actualVal)
             : typeof actualVal === expected;
      };
      const ok = Array.isArray(def.type)
        ? def.type.some(t => typeMatches(t, val))
        : typeMatches(def.type, val);
      if (!ok) errors.push({ category: 'type-mismatch', field, message: `field "${field}": expected ${def.type}, got ${Array.isArray(val) ? 'array' : typeof val}`, value: val, expected: String(def.type) });
    }
    if (def.enum && !def.enum.includes(val)) {
      errors.push({ category: 'invalid-enum', field, message: `field "${field}": value "${val}" not in [${def.enum.join(', ')}]`, value: String(val), expected: def.enum });
    }
    if (def.minimum !== undefined && typeof val === 'number' && val < def.minimum) {
      errors.push({ category: 'minimum-violation', field, message: `field "${field}": value ${val} is below minimum ${def.minimum}`, value: val, expected: String(def.minimum) });
    }
    // FORGE-S20-T01 — minimal `pattern` interpreter for string fields. Strictly
    // additive: schemas without `pattern` see no behavior change. Used by the
    // friction `subkind` slot to encode "frozen enum OR ^x_[a-z_]+$" as a
    // single combined regex (neither validator supports `anyOf`).
    if (def.pattern && typeof val === 'string') {
      let re;
      try { re = new RegExp(def.pattern); }
      catch (_) {
        errors.push({
          category: 'pattern-invalid',
          field,
          message:  `field "${field}": schema pattern "${def.pattern}" is not a valid regex`,
          value:    String(val),
          expected: String(def.pattern),
        });
        re = null;
      }
      if (re && !re.test(val)) {
        errors.push({
          category: 'pattern-mismatch',
          field,
          message:  `field "${field}": value "${val}" does not match pattern ${def.pattern}`,
          value:    String(val),
          expected: String(def.pattern),
        });
      }
    }
  }

  // Check for undeclared fields when additionalProperties is false
  if (schema.additionalProperties === false) {
    const allowed = new Set([...required, ...Object.keys(schema.properties || {})]);
    for (const key of Object.keys(record)) {
      if (!allowed.has(key)) {
        errors.push({ category: 'undeclared-field', field: key, message: `undeclared field: "${key}"`, value: record[key], expected: null });
      }
    }
  }

  // Conditional required via JSON-Schema `allOf` with `if/then/required`.
  // FORGE-S20-T00 — minimal interpreter: each clause may carry an `if` whose
  // `properties.<field>.const` must equal `record[field]`, AND every field in
  // `if.required` must be present on the record. When the clause matches,
  // every name in `then.required` becomes an additional required field.
  // No other JSON-Schema constructs are honored — this is intentional; the
  // store schemas are thin and we don't want to drag in a full validator.
  if (Array.isArray(schema.allOf)) {
    for (const clause of schema.allOf) {
      if (!clause || typeof clause !== 'object' || !clause.if || !clause.then) continue;
      const condProps = (clause.if.properties && typeof clause.if.properties === 'object')
        ? clause.if.properties
        : {};
      const condRequired = Array.isArray(clause.if.required) ? clause.if.required : [];

      // All `if.required` fields must be present on the record.
      const allReqPresent = condRequired.every((f) => record[f] !== undefined && record[f] !== null && record[f] !== '');
      if (!allReqPresent) continue;

      // Every const/enum predicate in `if.properties` must match the record.
      // (Plan 12 — added enum support for multi-value type branches)
      let condMatches = true;
      for (const [field, pred] of Object.entries(condProps)) {
        if (pred && Object.prototype.hasOwnProperty.call(pred, 'const')) {
          if (record[field] !== pred.const) { condMatches = false; break; }
        }
        if (pred && Object.prototype.hasOwnProperty.call(pred, 'enum')) {
          if (!pred.enum.includes(record[field])) { condMatches = false; break; }
        }
      }
      if (!condMatches) continue;

      // Clause fired — enforce `then.required`.
      const thenRequired = Array.isArray(clause.then.required) ? clause.then.required : [];
      for (const field of thenRequired) {
        if (record[field] === undefined || record[field] === null || record[field] === '') {
          errors.push({
            category: 'missing-required',
            field,
            message: `missing required field: "${field}"`,
            value:    record[field],
            expected: null,
          });
        }
      }
    }
  }

  return errors;
}

// --- Backfill defaults for --fix mode ---
const BACKFILL = {
  sprint: {
    createdAt: (rec) => rec.completedAt || rec.startDate || rec.endDate || new Date().toISOString(),
  },
  bug: {
    reportedAt: (rec) => rec.resolvedAt || new Date().toISOString(),
  },
  event: {
    eventId:         (_rec, id) => id,
    role:            (rec)        => rec.agent || 'unknown',
    action:          (rec)        => rec.phase  || 'unknown',
    phase:           (rec)        => rec.action || 'unknown',
    iteration:       ()           => 1,
    startTimestamp:  (rec)        => rec.timestamp || new Date().toISOString(),
    endTimestamp:    (rec)        => rec.timestamp || null,
    durationMinutes: ()           => null,
    model: (rec) => {
      if (rec.actor && typeof rec.actor === 'string' && rec.actor.includes('claude')) return rec.actor;
      return 'unknown';
    },
  },
};

module.exports = { validateRecord, MINIMAL_REQUIRED, NULLABLE_FK, BACKFILL, ENTITY_TYPES, ANCILLARY_SCHEMAS };

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
if (require.main === module) {

process.on('uncaughtException', (error) => {
  console.error('Fatal validate-store error:', error);
  process.exit(1);
});

const fs = require('fs');
const path = require('path');
const store = _getStore();
const schemas = _getSchemas();

const DRY_RUN = process.argv.includes('--dry-run');
const FIX_MODE = process.argv.includes('--fix');
const JSON_MODE = process.argv.includes('--json');

// Read engineering root, store root, and project prefix from config for filesystem consistency checks
const CONFIG_PATH = '.forge/config.json';
let engineeringRoot = 'engineering';
let storeRootFromConfig = '.forge/store';
let projectPrefix = '[A-Z]+';
try {
  const cfg = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));
  if (cfg.paths && cfg.paths.engineering) engineeringRoot = cfg.paths.engineering;
  if (cfg.paths && cfg.paths.store) storeRootFromConfig = cfg.paths.store;
  if (cfg.project && cfg.project.prefix) projectPrefix = cfg.project.prefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
} catch (_) {}

// Slug-aware directory discovery regexes
const SPRINT_DIR_RE = new RegExp(`^(${projectPrefix}-S\\d+)(-\\S+)?$`);
const TASK_FULL_RE  = new RegExp(`^(${projectPrefix}-S\\d+-T\\d+)(-\\S+)?$`);
const TASK_SHORT_RE = /^(T\d+)(-\S+)?$/;

let errorsCount = 0;
let warningsCount = 0;
let fixesCount = 0;

// Structured collections for --json mode
const jsonErrors = [];
const jsonWarnings = [];
const jsonFixes = [];

function err(id, msg, category, field, value, expected) {
  errorsCount++;
  if (JSON_MODE) {
    jsonErrors.push({ entity: id.split('/')[0].split('-')[0] || 'unknown', id, category: category || 'unknown', field: field || null, message: msg, value: value || null, expected: expected || null });
  } else {
    console.error(`ERROR  ${id}: ${msg}`);
  }
}

function warn(id, msg, category, field) {
  warningsCount++;
  if (JSON_MODE) {
    jsonWarnings.push({ entity: id.split('/')[0].split('-')[0] || 'unknown', id, category: category || 'unknown', field: field || null, message: msg });
  } else {
    console.log(`WARN   ${id}: ${msg}`);
  }
}

function fixMsg(id, msg, category, field) {
  fixesCount++;
  if (JSON_MODE) {
    jsonFixes.push({ entity: id.split('/')[0].split('-')[0] || 'unknown', id, category: category || 'backfill', field: field || null, message: msg, applied: !DRY_RUN });
  } else {
    console.log(`FIXED  ${id}: ${msg}`);
  }
}

function backfillRecord(id, rec, type) {
  const rules = BACKFILL[type];
  if (!rules) return false;
  let changed = false;
  for (const [field, derive] of Object.entries(rules)) {
    if (rec[field] === undefined || rec[field] === null || rec[field] === '') {
      const val = derive(rec, id);
      rec[field] = val;
      fixMsg(id, `backfilled "${field}" = "${val}"`, 'backfill', field);
      changed = true;
    }
  }
  if (changed) {
    // In dry-run mode, preview the fix without writing
    if (DRY_RUN) return changed;
    // Facade write uses the logic in FSImpl to maintain formatting
    if (type === 'sprint') store.writeSprint(rec);
    else if (type === 'task') store.writeTask(rec);
    else if (type === 'bug') store.writeBug(rec);
    else if (type === 'feature') store.writeFeature(rec);
    // Events are slightly different as they need sprintId
    else if (type === 'event') {
      // We'll handle event writing in the loop where sprintId is known
    }
  }
  return changed;
}

// Load all records up-front for referential integrity checks
const sprintIds = new Set();
const taskIds   = new Set();
const bugIds    = new Set();
const featureIds = new Set();

// --- Pass 1: validate structure, collect IDs ---
const sprints = store.listSprints();
for (const rec of sprints) {
  if (!rec) continue;
  if (FIX_MODE) backfillRecord(rec.sprintId, rec, 'sprint');
  if (rec.sprintId) sprintIds.add(rec.sprintId);
  for (const e of validateRecord(rec, schemas.sprint)) err(rec.sprintId, e.message, e.category, e.field, e.value, e.expected);
  if (!rec.path) warn(rec.sprintId, 'missing optional field "path"', 'missing-optional', 'path');
}

const tasks = store.listTasks();
for (const rec of tasks) {
  if (!rec) continue;
  if (rec.taskId) taskIds.add(rec.taskId);
  for (const e of validateRecord(rec, schemas.task)) err(rec.taskId, e.message, e.category, e.field, e.value, e.expected);
}

const bugs = store.listBugs();
for (const rec of bugs) {
  if (!rec) continue;
  if (FIX_MODE) backfillRecord(rec.bugId, rec, 'bug');
  if (rec.bugId) bugIds.add(rec.bugId);
  for (const e of validateRecord(rec, schemas.bug)) err(rec.bugId, e.message, e.category, e.field, e.value, e.expected);
}

const features = store.listFeatures();
for (const rec of features) {
  if (!rec) continue;
  const featureId = rec.id || 'unknown';
  featureIds.add(featureId);
}

// --- Pass 2: referential integrity ---
for (const rec of sprints) {
  if (!rec) continue;
  if (rec.feature_id && !featureIds.has(rec.feature_id)) {
    if (FIX_MODE) {
      rec.feature_id = null;
      if (!DRY_RUN) store.writeSprint(rec);
      fixMsg(rec.sprintId, `nullified orphaned feature_id "${rec.feature_id}"`, 'orphaned-fk', 'feature_id');
    } else {
      err(rec.sprintId, `feature_id "${rec.feature_id}" references unknown feature`, 'orphaned-fk', 'feature_id', rec.feature_id);
    }
  }
}

for (const rec of tasks) {
  if (!rec) continue;
  if (rec.sprintId && !sprintIds.has(rec.sprintId))
    err(rec.taskId, `sprintId "${rec.sprintId}" references unknown sprint`, 'orphaned-fk', 'sprintId', rec.sprintId);

  if (rec.feature_id && !featureIds.has(rec.feature_id)) {
    if (FIX_MODE) {
      rec.feature_id = null;
      if (!DRY_RUN) store.writeTask(rec);
      fixMsg(rec.taskId, `nullified orphaned feature_id "${rec.feature_id}"`, 'orphaned-fk', 'feature_id');
    } else {
      err(rec.taskId, `feature_id "${rec.feature_id}" references unknown feature`, 'orphaned-fk', 'feature_id', rec.feature_id);
    }
  }
}

for (const rec of bugs) {
  if (!rec) continue;
  for (const ref of (rec.similarBugs || [])) {
    if (!bugIds.has(ref)) err(rec.bugId, `similarBugs references unknown bug "${ref}"`, 'orphaned-fk', 'similarBugs', ref);
  }
}

// --- Events ---
const allSprints = store.listSprints();
for (const sprint of allSprints) {
  if (!sprint) continue;
  const sprintId = sprint.sprintId;
  const eventFileEntries = store.listEventFilenames(sprintId)
    .filter(entry => !entry.id.startsWith('_'));

  for (const entry of eventFileEntries) {
    const filename = entry.id; // filename without .json extension
    const rec = store.getEvent(filename, sprintId);
    if (!rec) continue;

    const eventId = rec.eventId;

    if (FIX_MODE) {
      const rules = BACKFILL.event;
      let changed = false;
      for (const [field, derive] of Object.entries(rules)) {
        if (rec[field] === undefined || rec[field] === null || rec[field] === '') {
          const val = derive(rec, filename);
          rec[field] = val;
          fixMsg(`${sprintId}/${filename}`, `backfilled "${field}" = "${val}"`, 'backfill', field);
          changed = true;
        }
      }

      // If the filename doesn't match the canonical eventId, resolve the mismatch.
      // When the eventId is a valid filename, rename the file to match it.
      // When the eventId is invalid (contains /, is a placeholder like "temp",
      // or cannot be a filename), backfill eventId to the filename instead.
      if (filename !== rec.eventId) {
        const isValidFilename = (id) => id && !id.includes('/') && !id.includes('\\') && id !== '.';
        if (isValidFilename(rec.eventId)) {
          if (!DRY_RUN) {
            try {
              store.renameEvent(sprintId, filename, rec.eventId);
            } catch (renameErr) {
              err(`${sprintId}/${filename}`, `cannot rename to ${rec.eventId}.json: ${renameErr.message}`, 'filename-mismatch', 'eventId');
            }
          }
          fixMsg(`${sprintId}/${filename}`, `renamed to ${rec.eventId}.json`, 'filename-mismatch', 'eventId');
        } else {
          // eventId is invalid for a filename — backfill it to the current filename
          fixMsg(`${sprintId}/${filename}`, `eventId "${rec.eventId}" is not a valid filename, resetting to "${filename}"`, 'filename-mismatch', 'eventId');
          rec.eventId = filename;
          changed = true;
        }
      }

      // Write the updated record (writeEvent now handles ghost detection internally)
      if (changed && !DRY_RUN) store.writeEvent(sprintId, rec);
    }

    for (const e of validateRecord(rec, schemas.event)) err(`${sprintId}/${eventId}`, e.message, e.category, e.field, e.value, e.expected);
    if (rec.taskId && !taskIds.has(rec.taskId) && !bugIds.has(rec.taskId))
      err(`${sprintId}/${eventId}`, `taskId "${rec.taskId}" references unknown task or bug`, 'orphaned-fk', 'taskId', rec.taskId);
    if (rec.sprintId && !sprintIds.has(rec.sprintId) && rec.sprintId !== sprintId)
      err(`${sprintId}/${eventId}`, `sprintId "${rec.sprintId}" references unknown sprint`, 'orphaned-fk', 'sprintId', rec.sprintId);
  }
}

// --- Pass 2b: Orphan event directories ---
// Scan .forge/store/events/ for subdirectories whose name does not match any
// known sprintId and is not a reserved virtual dir.
// Reserved:
//   - SYS-* — system-generated events that predate sprint records
//   - bugs  — virtual sprint dir for fix-bug phase events (see
//             meta/tool-specs/validate-store.spec.md §"event.sprintId")
const RESERVED_EVENT_PREFIX = /^(SYS-|bugs$)/;
const eventsBaseDir = path.join(storeRootFromConfig, 'events');
if (fs.existsSync(eventsBaseDir)) {
  let eventDirEntries;
  try { eventDirEntries = fs.readdirSync(eventsBaseDir); } catch (_) { eventDirEntries = []; }
  for (const entry of eventDirEntries) {
    if (RESERVED_EVENT_PREFIX.test(entry)) continue;  // reserved prefix — skip
    if (sprintIds.has(entry)) continue;                // known sprint — skip
    const entryPath = path.join(eventsBaseDir, entry);
    let isDir = false;
    try { isDir = fs.statSync(entryPath).isDirectory(); } catch (_) {}
    if (!isDir) continue;  // non-directory entries — skip silently
    warn(entry, `event directory "${entry}" has no matching sprint record (ORPHAN_EVENT_DIR)`, 'ORPHAN_EVENT_DIR');
  }
}

// --- Pass 3: Filesystem consistency ---
// Walk engineering/sprints/ to detect orphaned directories and dangling path fields.
// All checks here emit warnings (not errors) for backward compatibility.
const sprintsDir = path.join(engineeringRoot, 'sprints');
if (fs.existsSync(sprintsDir)) {
  let sprintEntries;
  try { sprintEntries = fs.readdirSync(sprintsDir); } catch (_) { sprintEntries = []; }

  for (const entry of sprintEntries) {
    const entryPath = path.join(sprintsDir, entry);
    let isDir = false;
    try { isDir = fs.statSync(entryPath).isDirectory(); } catch (_) {}
    if (!isDir) continue;

    const sprintMatch = SPRINT_DIR_RE.exec(entry);
    if (!sprintMatch) continue; // not a recognised sprint directory pattern — skip silently
    const dirSprintId = sprintMatch[1];

    if (!sprintIds.has(dirSprintId)) {
      warn(dirSprintId, `directory "${entry}" has no sprint record in store`, 'orphan-directory');
      continue; // no point walking tasks for an unregistered sprint
    }

    // Walk for task subdirectories
    let taskEntries;
    try { taskEntries = fs.readdirSync(entryPath); } catch (_) { taskEntries = []; }

    for (const taskEntry of taskEntries) {
      const taskEntryPath = path.join(entryPath, taskEntry);
      let isTaskDir = false;
      try { isTaskDir = fs.statSync(taskEntryPath).isDirectory(); } catch (_) {}
      if (!isTaskDir) continue;

      let dirTaskId = null;

      const taskFullMatch = TASK_FULL_RE.exec(taskEntry);
      if (taskFullMatch) {
        dirTaskId = taskFullMatch[1];
      } else {
        const taskShortMatch = TASK_SHORT_RE.exec(taskEntry);
        if (taskShortMatch) {
          // Construct full task ID from sprint ID + short task number (e.g. T09 → FORGE-S06-T09)
          dirTaskId = `${dirSprintId}-${taskShortMatch[1]}`;
        }
      }

      if (!dirTaskId) continue; // not a recognised task directory pattern — skip silently

      if (!taskIds.has(dirTaskId)) {
        warn(dirTaskId, `directory "${entry}/${taskEntry}" has no task record in store`, 'orphan-directory');
      }
    }
  }
}

// path field cross-check for sprints
for (const rec of sprints) {
  if (!rec || !rec.path) continue;
  if (!fs.existsSync(rec.path)) {
    warn(rec.sprintId, `path "${rec.path}" does not exist on disk`, 'stale-path', 'path');
  }
}

// path field cross-check for tasks
for (const rec of tasks) {
  if (!rec || !rec.path) continue;
  if (!fs.existsSync(rec.path)) {
    warn(rec.taskId, `path "${rec.path}" does not exist on disk`, 'stale-path', 'path');
  }
}

// --- Result ---
if (JSON_MODE) {
  const result = {
    ok: errorsCount === 0,
    errors: jsonErrors,
    warnings: jsonWarnings,
    fixes: jsonFixes,
    summary: {
      sprints: sprintIds.size,
      tasks: taskIds.size,
      bugs: bugIds.size,
      features: featureIds.size,
      errors: errorsCount,
      warnings: warningsCount,
      fixes: fixesCount
    }
  };
  console.log(JSON.stringify(result, null, 2));
  process.exit(errorsCount === 0 ? 0 : 1);
} else {
  if (fixesCount > 0) {
    console.log(`${fixesCount} field(s) backfilled.`);
  }
  if (errorsCount === 0) {
    console.log(`Store validation passed (${sprintIds.size} sprint(s), ${taskIds.size} task(s), ${bugIds.size} bug(s)).`);
    if (warningsCount > 0) console.log(`${warningsCount} warning(s).`);
    process.exit(0);
  } else {
    console.error(`\n${errorsCount} error(s) found.`);
    process.exit(1);
  }
}

} // end if (require.main === module)