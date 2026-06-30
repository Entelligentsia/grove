#!/usr/bin/env node
'use strict';

// Forge tool: store-cli
// Deterministic store custodian CLI — wraps store.cjs facade.
// Enforces schema validation on write and status transition rules.
// Usage: node store-cli.cjs <command> <args>

let _store;
function _getStore() { return _store || (_store = require('./store.cjs')); }
let _projectRoot;
function _getProjectRoot() { return _projectRoot || (_projectRoot = require('./lib/project-root.cjs').findProjectRoot()); }

const _path = require('path');
const { loadSchemas } = require('./lib/schema-loader.cjs');

// Path traversal guard — resolves the events directory and validates that
// sprintOrBugId doesn't escape it. Same pattern as store.cjs purgeEvents.
function _resolveEventsDir(sprintOrBugId) {
  const root = _getProjectRoot();
  const eventsBase = root
    ? _path.resolve(root, '.forge', 'store', 'events')
    : _path.resolve('.forge', 'store', 'events');
  const resolvedDir = _path.resolve(eventsBase, sprintOrBugId);
  if (!resolvedDir.startsWith(eventsBase + _path.sep) && resolvedDir !== eventsBase) {
    console.error(`Path traversal blocked: '${sprintOrBugId}' resolves outside events directory`);
    process.exit(1);
  }
  return resolvedDir;
}

// _getSchemas() delegates to lib/schema-loader.cjs (memoized, 4-path search
// chain). Callers use _getSchemas() so no call-sites need updating.
function _getSchemas() {
  return loadSchemas();
}

// ---------------------------------------------------------------------------
// Schema loading — same resolution as validate-store.cjs
// ---------------------------------------------------------------------------

const ENTITY_TYPES = ['sprint', 'task', 'bug', 'event', 'feature'];

const MINIMAL_REQUIRED = {
  sprint:  ['sprintId', 'title', 'status', 'taskIds', 'createdAt'],
  task:    ['taskId', 'sprintId', 'title', 'status', 'path'],
  bug:     ['bugId', 'title', 'severity', 'status', 'path', 'reportedAt'],
  event:   ['eventId', 'taskId', 'sprintId', 'role', 'action', 'phase', 'iteration', 'startTimestamp', 'endTimestamp', 'durationMinutes', 'model'],
  feature: ['id', 'title', 'status', 'created_at']
};

// Shared validator + nullable-field set live in ./lib/validate.js so the
// write-boundary hook can reuse the exact same validation logic as tool writes.
const { validateRecord, NULLABLE_FIELDS } = require('./lib/validate.js');

// FORGE-S22-T03: suggestion engine for "Did you mean?" on validation/transition errors
const { suggest, suggestEntityType, formatSuggestion } = require('./lib/suggest.cjs');
const { resolveSummaryFilename, PHASE_TO_KIND } = require('./lib/artifact-kinds.cjs');

// Valid phase keys for summaries (dot-delimited → underscore in JSON key)
const VALID_SUMMARY_PHASES = new Set(['plan', 'review_plan', 'implementation', 'code_review', 'validation', 'triage', 'approve']);

// Schema for a single phase summary (used by set-summary / set-bug-summary)
// Mirror of bug.schema.json § $defs.phaseSummary. Both definitions MUST stay
// in sync — this constant validates set-summary / set-bug-summary writes;
// the JSON schema validates entity reads. `route` is an optional triage-only
// field carrying the fix-bug pipeline route decision (A | B) and exists here
// for bug.summaries.triage in particular; non-triage phases simply do not
// set it.
const PHASE_SUMMARY_SCHEMA = {
  type: 'object',
  required: ['objective', 'written_at'],
  properties: {
    objective:   { type: 'string', maxLength: 280 },
    key_changes: { type: 'array', items: { type: 'string', maxLength: 200 }, maxItems: 12 },
    findings:    { type: 'array', items: { type: 'string', maxLength: 200 }, maxItems: 12 },
    verdict:     { type: 'string', enum: ['approved', 'revision', 'n/a'] },
    written_at:  { type: 'string' },
    artifact_ref:{ type: 'string' },
    route:       { type: 'string', enum: ['A', 'B'] },
    // forge-engineering#40: implement-phase file provenance. The implement
    // workflow records the repo-relative paths it created/modified;
    // commit-task.cjs derives its staging set from this list instead of the
    // LLM re-deriving the change surface from git each run.
    files_changed: { type: 'array', items: { type: 'string', maxLength: 300 }, maxItems: 100 }
  },
  additionalProperties: false
};

// validateRecord imported from ./lib/validate.js above.

// ---------------------------------------------------------------------------
// Transition tables
// ---------------------------------------------------------------------------

// Canonical transition tables — reconciled by T25 ADR (doc/decisions/state-machine-reconciliation.md).
// Authoritative source: forge/tools/build-enum-catalog.cjs (CANONICAL_*_TRANSITIONS).
// These tables are kept in sync with enum-catalog.json via the drift detection test.
// Task: FORGE-S25-T26 (closes N-E-2 on the plugin side; T27 closes it on forge-cli side).

const TASK_TRANSITIONS = {
  draft:                    ['planned', 'blocked', 'escalated', 'abandoned'],
  planned:                  ['plan-approved', 'plan-revision-required', 'blocked', 'escalated', 'abandoned'],
  'plan-approved':          ['implementing', 'plan-revision-required', 'blocked', 'escalated', 'abandoned'],
  implementing:             ['implemented', 'plan-revision-required', 'code-revision-required', 'blocked', 'escalated', 'abandoned'],
  implemented:              ['review-approved', 'plan-revision-required', 'code-revision-required', 'blocked', 'escalated', 'abandoned'],
  'review-approved':        ['approved', 'plan-revision-required', 'code-revision-required', 'blocked', 'escalated', 'abandoned'],
  approved:                 ['committed', 'plan-revision-required', 'code-revision-required', 'blocked', 'escalated', 'abandoned'],
  'plan-revision-required': ['planned', 'blocked', 'escalated', 'abandoned'],
  'code-revision-required': ['implementing', 'blocked', 'escalated', 'abandoned'],
  // Explicit terminal/sink entries — prevents FAILED_STATES bypass from allowing illegal re-opens.
  committed:                [],
  blocked:                  [],
  escalated:                [],
  abandoned:                [],
};

const SPRINT_TRANSITIONS = {
  planning:              ['active', 'blocked', 'abandoned'],
  active:                ['completed', 'partially-completed', 'blocked', 'abandoned'],
  completed:             ['retrospective-done'],
  'partially-completed': ['retrospective-done'],
  'retrospective-done':  [],
  blocked:               ['active', 'abandoned'],
  abandoned:             [],
};

const BUG_TRANSITIONS = {
  reported:      ['triaged', 'abandoned'],
  triaged:       ['in-progress', 'abandoned'],
  'in-progress': ['fixed', 'abandoned'],
  // Explicit terminal entries.
  fixed:         [],
  abandoned:     [],
  // The `approved` and `verified` enum members were removed (forge#GH-NNN).
  // The architect-approve verdict signal for bugs travels through
  // `bug.summaries.approve.verdict` (see read-verdict.cjs §
  // BUG_PHASE_VERDICT_SOURCE), not `bug.status`. Keeping them in the enum
  // created an LLM-translation trap whereby a task-shaped approve workflow
  // run on a bug improvised `update-status bug ... approved`, which failed
  // either at the schema layer (illegal transition out of terminal
  // `verified`) or, post-FORGE-BUG-002 preflight defence, at the gate
  // layer (forbid bug.status == approved). Dropping the enum removes the
  // trap at its source.
};

const FEATURE_TRANSITIONS = {
  draft:   ['active'],
  active:  ['shipped', 'retired'],
  // Terminal: shipped, retired
};

const TRANSITION_MAP = {
  task:    TASK_TRANSITIONS,
  sprint:  SPRINT_TRANSITIONS,
  bug:     BUG_TRANSITIONS,
  feature: FEATURE_TRANSITIONS
};

const TERMINAL_STATES = new Set([
  'committed', 'abandoned',          // task
  'retrospective-done',              // sprint
  'fixed',                           // bug
  'shipped', 'retired'               // feature
]);

// Failed/escape states that may be entered from any non-terminal state via the
// FAILED_STATES bypass in isLegalTransition.
//
// FORGE-S25-T26 (T25 ADR canonicalization):
// - `blocked` REMOVED — now has explicit table entries; removing prevents `committed → blocked`
//   and `bug → blocked` (neither in canonical tables).
// - `plan-revision-required`, `code-revision-required` REMOVED — now fully explicit in
//   TASK_TRANSITIONS; removing prevents `draft → plan-revision-required` (D-T-1).
//   All legitimate transitions to these states are covered by explicit table entries.
// - `escalated`, `abandoned` RETAINED — not in every entity's explicit table (e.g.,
//   FEATURE_TRANSITIONS has no escalated/abandoned entries). Universal escape remains.
// - `partially-completed` RETAINED — sprint-only escape.
// See: doc/decisions/state-machine-reconciliation.md; FORGE-S25-T26.
const FAILED_STATES = new Set([
  'escalated', 'abandoned',   // universal escapes (task explicit + feature bypass)
  'partially-completed'       // sprint
]);

function isLegalTransition(entityType, field, currentValue, newValue) {
  if (currentValue === newValue) return true; // no-op

  const table = TRANSITION_MAP[entityType];
  if (!table) return true; // no transition rules for this entity type

  // Terminal states cannot transition out
  if (TERMINAL_STATES.has(currentValue)) return false;

  // Failed states may be entered from any non-terminal state
  if (FAILED_STATES.has(newValue)) return true;

  // Check the explicit transition table
  const allowed = table[currentValue];
  if (!allowed) return false; // current state not in table (unknown state)

  return allowed.includes(newValue);
}

// ---------------------------------------------------------------------------
// Bug timestamp normalization helpers
// ---------------------------------------------------------------------------

// Returns true if the string is a date-only value (YYYY-MM-DD without time).
// These are rejected by the date-time format validator but agents commonly
// supply them for reportedAt/resolvedAt.
function _isDateOnly(ts) {
  return typeof ts === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(ts);
}

// Convert a date-only string (YYYY-MM-DD) into a full ISO datetime by
// appending the current time-of-day in UTC. The date portion is preserved
// from the input; only the time component is auto-populated.
function _dateOnlyToISO(dateStr) {
  const now = new Date();
  const timePart = now.toISOString().slice(10);  // e.g. "T14:32:07.123Z"
  return dateStr + timePart;
}

// Normalize bug datetime fields before writing. When agents supply date-only
// values (YYYY-MM-DD) for reportedAt or resolvedAt, auto-populates the time
// component from the current time-of-day so the value passes date-time format
// validation. Full ISO datetimes are left untouched.
function _normalizeBugTimestamps(data) {
  if (_isDateOnly(data.reportedAt))  data.reportedAt  = _dateOnlyToISO(data.reportedAt);
  if (_isDateOnly(data.resolvedAt))  data.resolvedAt  = _dateOnlyToISO(data.resolvedAt);
  return data;
}

// ---------------------------------------------------------------------------
// Model discovery
// ---------------------------------------------------------------------------

// Deterministic model discovery — probes environment variables in priority
// order to resolve the actual runtime model identifier. Returns "unknown"
// when no signal is available instead of guessing an Anthropic model name.
function discoverModel() {
  const candidates = [
    process.env.CLAUDE_CODE_SUBAGENT_MODEL,
    process.env.ANTHROPIC_MODEL,
    process.env.CLAUDE_MODEL,
  ];
  for (const val of candidates) {
    if (val && val.trim()) return val.trim();
  }
  return 'unknown';
}

// discoverProvider() — runtime provider resolution. Mirrors discoverModel.
// The orchestrator (forge-cli) is expected to set FORGE_PROVIDER explicitly
// when spawning a subagent. Falls back to "unknown" when no signal is available.
function discoverProvider() {
  const candidates = [
    process.env.FORGE_PROVIDER,
    process.env.CLAUDE_CODE_PROVIDER,
  ];
  for (const val of candidates) {
    if (val && val.trim()) return val.trim();
  }
  return 'unknown';
}

module.exports = { isLegalTransition, validateRecord, TRANSITION_MAP, TERMINAL_STATES, FAILED_STATES, ENTITY_TYPES, MINIMAL_REQUIRED, NULLABLE_FIELDS, VALID_SUMMARY_PHASES, PHASE_SUMMARY_SCHEMA, _isDateOnly, _dateOnlyToISO, _normalizeBugTimestamps, discoverModel, discoverProvider };

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
if (require.main === module) {

process.on('uncaughtException', (error) => {
  console.error('Fatal store-cli error:', error);
  process.exit(1);
});

try {
const fs = require('fs');
const path = require('path');
const store = _getStore();
const schemas = _getSchemas();

const DRY_RUN = process.argv.includes('--dry-run');
const VERBOSE = process.argv.includes('--verbose');

// ---------------------------------------------------------------------------
// Entity ID field mapping
// ---------------------------------------------------------------------------

const ENTITY_ID_FIELD = {
  sprint:  'sprintId',
  task:    'taskId',
  bug:     'bugId',
  event:   'eventId',
  feature: 'id'
};

// ---------------------------------------------------------------------------
// Store accessor mapping
// ---------------------------------------------------------------------------

function getEntity(entity, id) {
  switch (entity) {
    case 'sprint':  return store.getSprint(id);
    case 'task':    return store.getTask(id);
    case 'bug':     return store.getBug(id);
    case 'event':   return store.getEvent(id, null); // needs sprintId separately
    case 'feature': return store.getFeature(id);
    default:        return null;
  }
}

function writeEntity(entity, data) {
  switch (entity) {
    case 'sprint':  return store.writeSprint(data);
    case 'task':    return store.writeTask(data);
    case 'bug':     return store.writeBug(data);
    case 'event':   return store.writeEvent(data.sprintId, data);
    case 'feature': return store.writeFeature(data);
  }
}

function deleteEntity(entity, id) {
  switch (entity) {
    case 'sprint':  return store.deleteSprint(id);
    case 'task':    return store.deleteTask(id);
    case 'bug':     return store.deleteBug(id);
    case 'feature': return store.deleteFeature(id);
    default:
      console.error(`Unknown entity type: ${entity}${formatSuggestion(suggestEntityType(entity, ['sprint', 'task', 'bug', 'feature']))}`);
      process.exit(1);
  }
}

function listEntities(entity, filter) {
  switch (entity) {
    case 'sprint':  return store.listSprints(filter);
    case 'task':    return store.listTasks(filter);
    case 'bug':     return store.listBugs(filter);
    case 'feature': return store.listFeatures(filter);
    case 'event': {
      // Defect D fix: traverse all sub-directories under events/ — sprints write
      // to events/<sprintId>/, bugs to events/bugs/, enhancements to events/enhancement/.
      // Return the union of all event JSONs across every sub-directory, skipping
      // sidecar files (_-prefixed) and non-JSON files.
      const eventsBase = path.join(store.impl.storeRoot, 'events');
      if (!fs.existsSync(eventsBase)) return [];
      const allEvents = [];
      const subDirs = fs.readdirSync(eventsBase, { withFileTypes: true });
      for (const entry of subDirs) {
        if (!entry.isDirectory()) continue;
        const subDir = path.join(eventsBase, entry.name);
        const files = fs.readdirSync(subDir).filter(
          (f) => f.endsWith('.json') && !f.startsWith('_')
        );
        for (const file of files) {
          try {
            const rec = JSON.parse(fs.readFileSync(path.join(subDir, file), 'utf8'));
            if (rec && (!filter || Object.entries(filter).every(([k, v]) => rec[k] === v))) {
              allEvents.push(rec);
            }
          } catch (_) { /* skip malformed files */ }
        }
      }
      return allEvents;
    }
    default:
      console.error(`Unknown entity type: ${entity}${formatSuggestion(suggestEntityType(entity, ['sprint', 'task', 'bug', 'feature']))}`);
      process.exit(1);
  }
}

// ---------------------------------------------------------------------------
// Sidecar handling
// ---------------------------------------------------------------------------

// Canonical event schema token fields
const CANONICAL_TOKEN_FIELDS = [
  'inputTokens', 'outputTokens', 'cacheReadTokens', 'cacheWriteTokens',
  'model', 'provider', 'durationMinutes', 'startTimestamp', 'endTimestamp',
  'tokenSource'
];

// Accepted sidecar fields (includes aliases)
const SIDECAR_ACCEPTED_FIELDS = new Set([
  'inputTokens', 'outputTokens', 'cacheReadTokens', 'cacheWriteTokens',
  'model', 'provider', 'durationMinutes',
  'startTimestamp', 'endTimestamp', 'cacheCreationTokens',
  'tokenSource'
]);

// Alias mapping for sidecar → canonical event
const SIDECAR_ALIASES = {
  'cacheCreationTokens': 'cacheWriteTokens'
};

function resolveSidecarDir(sprintId) {
  const storeRoot = store.impl.storeRoot;
  return path.join(storeRoot, 'events', sprintId);
}

function sidecarPath(sprintId, eventId) {
  return path.join(resolveSidecarDir(sprintId), `_${eventId}_usage.json`);
}

function canonicalEventPath(sprintId, eventId) {
  return path.join(resolveSidecarDir(sprintId), `${eventId}.json`);
}

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

const args = process.argv.slice(2);

if (args.length === 0 || args[0] === '--help' || args[0] === '-h') {
  console.log(`Forge Store Custodian CLI

Usage: node store-cli.cjs <command> <args> [--dry-run]

Commands:
  write <entity> '<json>'                     Write a full entity record
  read <entity> <id> [--json]                 Read an entity record
  list <entity> [key=value ...]               List entities with optional filter
  delete <entity> <id>                        Delete an entity record
  update-status <entity> <id> <field> <value> [--force]
                                              Update status/enum field with transition check
  emit <sprintId> '<json>' [--sidecar] [--allow-synthetic]  Write an event (or sidecar)
  merge-sidecar <sprintId> <eventId>          Merge sidecar into canonical event
  record-usage <sprintId> <eventId> [flags]  Write a token-usage sidecar
  purge-events <sprintId>                     Delete all events for a sprint
  resolve-event <sprintId> <eventId> --status <status> [--proposal-ref <path>] [--bug-ref <url>]
                                              Resolve a friction event (sets resolution field)
  progress <sprintOrBugId> <agentName> <bannerKey> <status> [detail]
                                              Append a progress entry to the log
  progress-clear <sprintOrBugId>              Clear (truncate) the progress log
  write-collation-state '<json>'             Write COLLATION_STATE.json
  validate <entity> '<json>'                  Validate against schema without writing
  set-summary <taskId> <phase> [<jsonFile>]     Set a phase summary on a task record (sidecar auto-resolved from record.path when jsonFile omitted)
  set-bug-summary <bugId> <phase> [<jsonFile>]  Set a phase summary on a bug record (sidecar auto-resolved from record.path when jsonFile omitted)
  describe <entity>                           Print the JSON Schema for an entity
  template <entity>                           Print a canonical sample record (required fields populated)

Alias commands:
  get <entity> <id> [--json]                  Alias for: read <entity> <id> [--json]
  get-task <id> [--json]                      Alias for: read task <id> [--json]
  get-bug <id> [--json]                       Alias for: read bug <id> [--json]
  get-sprint <id> [--json]                    Alias for: read sprint <id> [--json]
  get-summary <taskId> <phase> [--json]       Read summaries[phase] from a task record
  get-bug-summary <bugId> <phase> [--json]    Read summaries[phase] from a bug record

Entities: sprint, task, bug, event, feature
Phases (summaries): plan, review_plan, implementation, code_review, validation

Flags:
  --dry-run    Validate and preview without writing (applies to all write commands)
  --force      Bypass transition check on update-status. Operator-gated: requires
               FORGE_ALLOW_FORCE=1 in the environment. Subagents MUST NOT use this;
               surface the illegal transition to the orchestrator instead.
  --json       Output raw JSON on read (no pretty-print)
  --sidecar          Write as sidecar file on emit (ephemeral _-prefixed)
  --allow-synthetic  Bypass sprintId FK check on emit (for synthetic/test events)

Exit codes: 0 on success, 1 on failure`);
  process.exit(0);
}

// ---------------------------------------------------------------------------
// Alias dispatch table (FORGE-S22-T02)
//
// Read-aliases delegate to `cmdRead` by rewriting argv before the main switch.
// Summary read-throughs (`get-summary`, `get-bug-summary`) are NOT in this
// map — they are registered as direct cases in the switch below to avoid
// any chance of routing through a write verb.
//
// Entries:
//   target          — canonical command this alias rewrites to
//   entity          — if set, inserted as the first positional arg (so
//                     `get-task <id>` becomes `read task <id>`)
//   injectEntity:false — bare `get`; requires the caller to supply the
//                     entity as args[1]. Validated below.
// ---------------------------------------------------------------------------
const ALIAS_MAP = {
  'get':        { target: 'read', injectEntity: false },
  'get-task':   { target: 'read', entity: 'task'   },
  'get-bug':    { target: 'read', entity: 'bug'    },
  'get-sprint': { target: 'read', entity: 'sprint' },
};

let command = args[0];

if (Object.prototype.hasOwnProperty.call(ALIAS_MAP, command)) {
  const entry = ALIAS_MAP[command];
  // Drop the alias verb from args; the remainder are positionals + flags.
  args.splice(0, 1);
  if (entry.entity) {
    // `get-task <id>` → args becomes [<id>, ...flags]; prepend entity so the
    // downstream cmdRead sees [entity, id, ...flags].
    args.unshift(entry.entity);
  } else if (entry.injectEntity === false) {
    // Bare `get`: caller must have supplied `<entity> <id>`. Split flags from
    // positionals so we validate the first positional, not a flag.
    const positionals = args.filter((a) => !a.startsWith('--'));
    if (positionals.length < 2 || !ENTITY_TYPES.includes(positionals[0])) {
      console.error("error: 'get' requires an entity type — usage: get <task|bug|sprint> <id>");
      process.exit(1);
    }
  }
  command = entry.target;
  // Reconstruct argv so cmdRead, which reads args[1]/args[2], sees the
  // rewritten shape `[command, entity, id, ...flags]`.
  args.unshift(command);
}

// ---------------------------------------------------------------------------
// Command implementations
// ---------------------------------------------------------------------------

function cmdWrite() {
  const entity = args[1];
  const jsonStr = args[2];

  if (!entity || !jsonStr) {
    console.error('Usage: store-cli.cjs write <entity> \'<json>\'');
    process.exit(1);
  }

  if (!ENTITY_TYPES.includes(entity)) {
    console.error(`Unknown entity type: ${entity}${formatSuggestion(suggestEntityType(entity))}`);
    process.exit(1);
  }

  let data;
  try {
    data = JSON.parse(jsonStr);
  } catch (e) {
    console.error(`Invalid JSON: ${e.message}`);
    process.exit(1);
  }

  // Auto-populate date-only YYYY-MM-DD values in bug datetime fields.
  // Agents commonly supply date-only values for reportedAt/resolvedAt;
  // this normalizes them to full ISO datetimes before schema validation.
  if (entity === 'bug') {
    _normalizeBugTimestamps(data);
  }

  const errors = validateRecord(data, schemas[entity], { entity });
  if (errors.length > 0) {
    for (const e of errors) console.error(e);
    process.exit(1);
  }

  if (DRY_RUN) {
    console.log(`[dry-run] would write ${entity} ${data[ENTITY_ID_FIELD[entity]]}`);
  } else {
    writeEntity(entity, data);
  }
  if (VERBOSE) console.log(JSON.stringify({ ok: true, entity, id: data[ENTITY_ID_FIELD[entity]], dryRun: DRY_RUN }));
}

// Flatten a record to `key: value` lines for `--format flat` — a non-JSON,
// token-efficient read mode. Nested objects → dotted keys; arrays of primitives
// → comma-joined; arrays of objects → indexed dotted keys. Built-ins only.
function toFlatLines(obj, prefix) {
  const lines = [];
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (v === null || v === undefined) {
      lines.push(`${key}: ${v === null ? 'null' : ''}`);
    } else if (Array.isArray(v)) {
      if (v.length === 0) {
        lines.push(`${key}: []`);
      } else if (v.every((x) => x === null || typeof x !== 'object')) {
        lines.push(`${key}: ${v.map((x) => (x === null ? 'null' : String(x))).join(', ')}`);
      } else {
        v.forEach((item, i) => {
          if (item && typeof item === 'object') lines.push(...toFlatLines(item, `${key}[${i}]`));
          else lines.push(`${key}[${i}]: ${item === null ? 'null' : String(item)}`);
        });
      }
    } else if (typeof v === 'object') {
      const nested = toFlatLines(v, key);
      lines.push(...(nested.length ? nested : [`${key}: {}`]));
    } else {
      lines.push(`${key}: ${String(v)}`);
    }
  }
  return lines;
}

function cmdRead() {
  const entity = args[1];
  const id = args[2];
  const asJson      = args.includes('--json');
  const noSummaries = args.includes('--no-summaries');

  let fieldsProjection = null;
  const fieldsIdx = args.indexOf('--fields');
  if (fieldsIdx !== -1 && args[fieldsIdx + 1] && !args[fieldsIdx + 1].startsWith('--')) {
    fieldsProjection = args[fieldsIdx + 1].split(',').map((f) => f.trim()).filter(Boolean);
  }

  let format = null;
  const formatIdx = args.indexOf('--format');
  if (formatIdx !== -1 && args[formatIdx + 1] && !args[formatIdx + 1].startsWith('--')) {
    format = args[formatIdx + 1].trim();
  }

  if (!entity || !id) {
    console.error('Usage: store-cli.cjs read <entity> <id> [--json] [--format flat] [--no-summaries] [--fields <comma-list>]');
    process.exit(1);
  }

  if (!ENTITY_TYPES.includes(entity)) {
    console.error(`Unknown entity type: ${entity}${formatSuggestion(suggestEntityType(entity))}`);
    process.exit(1);
  }

  // Events need sprintId for lookup — read by eventId with sprintId resolution
  let record;
  if (entity === 'event') {
    // For events, try to find by scanning sprint directories
    const sprints = store.listSprints();
    record = null;
    for (const sprint of sprints) {
      if (!sprint) continue;
      const found = store.getEvent(id, sprint.sprintId);
      if (found) { record = found; break; }
    }
  } else {
    record = getEntity(entity, id);
  }

  if (!record) {
    console.error(`Entity not found: ${entity} ${id}`);
    process.exit(1);
  }

  // Apply projection before formatting
  let projected = record;
  if (noSummaries) {
    projected = Object.fromEntries(Object.entries(projected).filter(([k]) => k !== 'summaries'));
  }
  if (fieldsProjection) {
    projected = Object.fromEntries(fieldsProjection.map((f) => [f, projected[f]]).filter(([, v]) => v !== undefined));
  }

  if (format === 'flat') {
    console.log(toFlatLines(projected).join('\n'));
  } else if (asJson) {
    console.log(JSON.stringify(projected));
  } else {
    console.log(JSON.stringify(projected, null, 2));
  }
}

function cmdList() {
  const entity = args[1];

  if (!entity) {
    console.error('Usage: store-cli.cjs list <entity> [key=value ...] [--no-summaries] [--fields <comma-list>] [--limit N] [--count]');
    process.exit(1);
  }

  if (!ENTITY_TYPES.includes(entity)) {
    console.error(`Unknown entity type: ${entity}${formatSuggestion(suggestEntityType(entity, ['sprint', 'task', 'bug', 'event', 'feature']))}`);
    process.exit(1);
  }

  // Parse projection flags and key=value filter pairs from remaining args
  const noSummaries = args.includes('--no-summaries');
  const countOnly   = args.includes('--count');

  let fieldsProjection = null;
  const fieldsIdx = args.indexOf('--fields');
  if (fieldsIdx !== -1 && args[fieldsIdx + 1] && !args[fieldsIdx + 1].startsWith('--')) {
    fieldsProjection = args[fieldsIdx + 1].split(',').map((f) => f.trim()).filter(Boolean);
  }

  let limitN = null;
  const limitIdx = args.indexOf('--limit');
  if (limitIdx !== -1 && args[limitIdx + 1] && !args[limitIdx + 1].startsWith('--')) {
    const n = parseInt(args[limitIdx + 1], 10);
    if (!isNaN(n) && n >= 0) limitN = n;
  }

  const filter = {};
  for (let i = 2; i < args.length; i++) {
    // Skip flag arguments and their value arguments
    if (args[i].startsWith('--')) { i++; continue; }
    const eqIdx = args[i].indexOf('=');
    if (eqIdx > 0) {
      const key = args[i].slice(0, eqIdx);
      const val = args[i].slice(eqIdx + 1);
      // Try to parse numeric values
      const num = Number(val);
      filter[key] = (val !== '' && !isNaN(num) && val === String(num)) ? num : val;
    }
  }

  let records = listEntities(entity, Object.keys(filter).length > 0 ? filter : undefined);

  // --count: emit bare integer and exit (mutually exclusive with other flags)
  if (countOnly) {
    console.log(String(records.length));
    return;
  }

  // --limit: truncate after filtering
  if (limitN !== null) {
    records = records.slice(0, limitN);
  }

  // Apply projection
  if (fieldsProjection || noSummaries) {
    records = records.map((rec) => {
      let projected = rec;
      if (noSummaries) {
        projected = Object.fromEntries(Object.entries(projected).filter(([k]) => k !== 'summaries'));
      }
      if (fieldsProjection) {
        projected = Object.fromEntries(fieldsProjection.map((f) => [f, projected[f]]).filter(([, v]) => v !== undefined));
      }
      return projected;
    });
  }

  console.log(JSON.stringify(records, null, 2));
}

function cmdDelete() {
  const entity = args[1];
  const id = args[2];

  if (!entity || !id) {
    console.error('Usage: store-cli.cjs delete <entity> <id>');
    process.exit(1);
  }

  if (!ENTITY_TYPES.includes(entity)) {
    console.error(`Unknown entity type: ${entity}${formatSuggestion(suggestEntityType(entity, ['sprint', 'task', 'bug', 'feature']))}`);
    process.exit(1);
  }

  deleteEntity(entity, id);
  if (VERBOSE) console.log(JSON.stringify({ ok: true, deleted: `${entity}/${id}` }));
}

function cmdUpdateStatus() {
  const entity = args[1];
  const id = args[2];
  const field = args[3];
  const value = args[4];
  const force = args.includes('--force');

  if (!entity || !id || !field || !value) {
    console.error('Usage: store-cli.cjs update-status <entity> <id> <field> <value> [--force]');
    process.exit(1);
  }

  if (!TRANSITION_MAP[entity]) {
    const transitionTypes = Object.keys(TRANSITION_MAP);
    const sug = suggestEntityType(entity, transitionTypes);
    if (sug.length > 0) {
      console.error(`No transition rules for entity type: ${entity} ${formatSuggestion(sug)}`);
    } else {
      console.error(`No transition rules for entity type: ${entity}. Valid types: ${transitionTypes.join(', ')}`);
    }
    process.exit(1);
  }

  // Read current record
  const record = getEntity(entity, id);
  if (!record) {
    console.error(`Entity not found: ${entity} ${id}`);
    process.exit(1);
  }

  const currentValue = record[field];
  if (currentValue === undefined) {
    console.error(`Field "${field}" not found on ${entity} ${id}`);
    process.exit(1);
  }

  // --force is operator-gated: it bypasses the FSM safety net, so it must
  // not be reachable by a subagent that simply hit a wall. Require the
  // operator to opt in via FORGE_ALLOW_FORCE=1 (forge#87).
  if (force && process.env.FORGE_ALLOW_FORCE !== '1') {
    console.error('--force is operator-gated: re-run with FORGE_ALLOW_FORCE=1 in the environment to bypass the FSM. Subagents must not invoke --force; surface the illegal transition to the orchestrator instead.');
    process.exit(1);
  }

  // Check transition legality
  if (!force && !isLegalTransition(entity, field, currentValue, value)) {
    const legalTargets = TRANSITION_MAP[entity] && TRANSITION_MAP[entity][currentValue]
      ? TRANSITION_MAP[entity][currentValue]
      : [];
    const transitionSuggestions = suggest(value, legalTargets);
    console.error(`Illegal transition: ${entity} ${id} ${field}: ${currentValue} → ${value}` + (formatSuggestion(transitionSuggestions) ? ' ' + formatSuggestion(transitionSuggestions) : ''));
    process.exit(1);
  }

  if (force && !isLegalTransition(entity, field, currentValue, value)) {
    console.error(`WARN: --force bypassing illegal transition: ${entity} ${id} ${field}: ${currentValue} → ${value}`);
  }

  // Apply update and write back
  if (DRY_RUN) {
    console.log(`[dry-run] would update ${entity} ${id} ${field}: ${currentValue} → ${value}`);
  } else {
    record[field] = value;
    writeEntity(entity, record);
  }
  if (VERBOSE) console.log(JSON.stringify({ ok: true, entity, id, field, from: currentValue, to: value, force, dryRun: DRY_RUN }));
}

// ---------------------------------------------------------------------------
// Timestamp normalization helpers (#56)
// ---------------------------------------------------------------------------

// Returns true if the timestamp string has a zeroed time component (T00:00:00),
// which indicates the caller provided a date-only value instead of a real
// time-of-day. Midnight UTC is treated as "not set" for event timing purposes.
function _isZeroedTimestamp(ts) {
  if (typeof ts !== 'string') return true;  // null / missing → normalize
  return /T00:00:00/.test(ts);
}

// Normalize event timestamps before writing. Replaces any zeroed or absent
// startTimestamp / endTimestamp with the current real time, then recomputes
// durationMinutes from the two timestamps so cost reports are accurate.
function _normalizeEventTimestamps(data) {
  const now = new Date().toISOString();

  if (_isZeroedTimestamp(data.startTimestamp)) data.startTimestamp = now;
  if (_isZeroedTimestamp(data.endTimestamp))   data.endTimestamp   = now;

  // Recompute durationMinutes whenever both timestamps are present.
  if (data.startTimestamp && data.endTimestamp) {
    const diffMs = new Date(data.endTimestamp) - new Date(data.startTimestamp);
    data.durationMinutes = Math.max(0, diffMs / 60000);
  }

  return data;
}

// resolveValidSprintIds() — reads .forge/store/sprints/ and returns an array
// of sprintId strings (filenames without .json extension). Returns [] on ENOENT
// so fresh installs without a sprints dir don't crash.
// Not exported — CLI-only concern.
function resolveValidSprintIds() {
  try {
    const storeRoot = store.storeRoot || '.forge/store';
    const sprintsDir = path.join(storeRoot, 'sprints');
    return fs.readdirSync(sprintsDir)
      .filter(f => f.endsWith('.json'))
      .map(f => f.slice(0, -5));
  } catch (e) {
    if (e.code === 'ENOENT') return [];
    throw e;
  }
}

function cmdEmit() {
  const sprintId = args[1];
  const jsonStr = args[2];
  const isSidecar = args.includes('--sidecar');
  const allowSynthetic = args.includes('--allow-synthetic');

  if (!sprintId || !jsonStr) {
    console.error('Usage: store-cli.cjs emit <sprintId> \'<json>\' [--sidecar] [--allow-synthetic]');
    process.exit(1);
  }

  // FK check: reject sprintIds that are not in the store and not reserved.
  // Reserved sprintIds:
  //   - SYS-*       — system-generated events that predate sprint records
  //   - bugs        — virtual sprint dir for fix-bug phase events (see
  //                   meta/tool-specs/validate-store.spec.md §"event.sprintId")
  //   - enhancement — virtual sprint dir for post-init / post-sprint
  //                   enhancement-trigger events (FORGE-S25-T01)
  // --allow-synthetic bypasses the check for test-harness or synthetic events.
  if (!allowSynthetic) {
    const RESERVED_GLOB = /^(SYS-|bugs$|enhancement$)/;
    if (!RESERVED_GLOB.test(sprintId)) {
      const validSprintIds = resolveValidSprintIds();
      if (!validSprintIds.includes(sprintId)) {
        // Suggestion strategy: Levenshtein first; if no results, try suffix match
        // (e.g., "S01" is a suffix of "FORGE-S01") which Levenshtein alone misses
        // when the prefix distance exceeds maxDist=2.
        let suggestions = suggest(sprintId, validSprintIds);
        if (suggestions.length === 0) {
          const lower = sprintId.toLowerCase();
          const suffixMatch = validSprintIds.filter(v => v.toLowerCase().endsWith(lower) || v.toLowerCase().endsWith('-' + lower));
          if (suffixMatch.length > 0) suggestions = suffixMatch.slice(0, 3);
        }
        const hint = formatSuggestion(suggestions);
        console.error(`Unknown sprintId: ${sprintId}.${hint ? ' ' + hint : ''}`);
        console.error(`Valid sprint IDs: ${validSprintIds.join(', ') || '(none found)'}`);
        process.exit(1);
      }
    }
  }

  let data;
  try {
    data = JSON.parse(jsonStr);
  } catch (e) {
    console.error(`Invalid JSON: ${e.message}`);
    process.exit(1);
  }

  if (isSidecar) {
    // Write sidecar file — validate against the sidecar schema (eventId +
    // optional token/cost fields). Full event-shape enforcement happens
    // on merge into the canonical event.
    const sidecarErrors = validateRecord(data, schemas['event-sidecar']);
    if (sidecarErrors.length > 0) {
      for (const e of sidecarErrors) console.error(e);
      process.exit(1);
    }

    const sidecarDir = resolveSidecarDir(sprintId);
    const filePath = sidecarPath(sprintId, data.eventId);

    if (DRY_RUN) {
      console.log(`[dry-run] would write sidecar _${data.eventId}_usage.json`);
    } else {
      // Ensure directory exists
      if (!fs.existsSync(sidecarDir)) {
        fs.mkdirSync(sidecarDir, { recursive: true });
      }

      fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n', 'utf8');
    }
    if (VERBOSE) console.log(JSON.stringify({ ok: true, sidecar: true, eventId: data.eventId, sprintId, dryRun: DRY_RUN }));
  } else {
    // Normalize zeroed timestamps before validation so agents that provide
    // date-only values (T00:00:00Z) get real time-of-day stamped in (#56).
    _normalizeEventTimestamps(data);

    // Auto-populate model from environment when missing or empty (FORGE-S12-T06).
    // Callers who set model explicitly take priority — discoverModel() is only
    // used as a fallback so we never silently record a wrong model name.
    if (!data.model || !data.model.trim()) {
      data.model = discoverModel();
    }

    // Same for provider — required by event schema. Orchestrators set
    // FORGE_PROVIDER explicitly; fall back to "unknown" rather than guess.
    if (!data.provider || !data.provider.trim()) {
      data.provider = discoverProvider();
    }

    // Validate as event entity
    const errors = validateRecord(data, schemas.event);
    if (errors.length > 0) {
      for (const e of errors) console.error(e);
      process.exit(1);
    }

    if (DRY_RUN) {
      console.log(`[dry-run] would emit event ${data.eventId}`);
    } else {
      store.writeEvent(sprintId, data);
    }
    if (VERBOSE) console.log(JSON.stringify({ ok: true, event: true, eventId: data.eventId, sprintId, dryRun: DRY_RUN }));
  }
}

function cmdMergeSidecar() {
  const sprintId = args[1];
  const eventId = args[2];

  if (!sprintId || !eventId) {
    console.error('Usage: store-cli.cjs merge-sidecar <sprintId> <eventId>');
    process.exit(1);
  }

  // Read sidecar
  const scPath = sidecarPath(sprintId, eventId);
  if (!fs.existsSync(scPath)) {
    console.error(`Sidecar not found: ${scPath}`);
    process.exit(1);
  }

  let sidecar;
  try {
    sidecar = JSON.parse(fs.readFileSync(scPath, 'utf8'));
  } catch (e) {
    console.error(`Invalid sidecar JSON: ${e.message}`);
    process.exit(1);
  }

  // Read canonical event
  const canPath = canonicalEventPath(sprintId, eventId);
  if (!fs.existsSync(canPath)) {
    console.error(`Canonical event not found: ${canPath}`);
    process.exit(1);
  }

  let event;
  try {
    event = JSON.parse(fs.readFileSync(canPath, 'utf8'));
  } catch (e) {
    console.error(`Invalid canonical event JSON: ${e.message}`);
    process.exit(1);
  }

  // Merge token fields from sidecar into event
  for (const [key, value] of Object.entries(sidecar)) {
    // Resolve aliases
    const canonicalKey = SIDECAR_ALIASES[key] || key;
    if (CANONICAL_TOKEN_FIELDS.includes(canonicalKey)) {
      event[canonicalKey] = value;
    }
  }

  // Re-validate the merged canonical event against the event schema. Catches
  // the case where a sidecar's token field is present but the canonical event
  // was already malformed — we do not want to silently persist invalid data.
  const mergedErrors = validateRecord(event, schemas.event);
  if (mergedErrors.length > 0) {
    console.error(`Merged event ${eventId} failed schema validation:`);
    for (const e of mergedErrors) console.error(`  ${e}`);
    process.exit(1);
  }

  if (DRY_RUN) {
    console.log(`[dry-run] would merge sidecar into ${eventId} and delete sidecar`);
  } else {
    // Write updated event via facade (handles ghost-file logic)
    store.writeEvent(sprintId, event);

    // Delete sidecar
    fs.unlinkSync(scPath);
  }

  if (VERBOSE) console.log(JSON.stringify({ ok: true, merged: true, eventId, sprintId, dryRun: DRY_RUN }));
}

function cmdRecordUsage() {
  const sprintId = args[1];
  const eventId = args[2];

  if (!sprintId || !eventId) {
    console.error('Usage: store-cli.cjs record-usage <sprintId> <eventId> [flags]');
    console.error('  Flags:');
    console.error('    --input-tokens <n>          Input token count');
    console.error('    --output-tokens <n>         Output token count');
    console.error('    --cache-read-tokens <n>     Cache read token count');
    console.error('    --cache-write-tokens <n>    Cache write token count');
    console.error('    --token-source <src>         reported | estimated');
    console.error('    --model <model>             Model identifier');
    console.error('    --provider <name>           Provider name (e.g. anthropic, zai, openai)');
    console.error('    --duration-minutes <n>      Duration in minutes');
    process.exit(1);
  }

  // Parse flag arguments from remaining args
  const flagArgs = args.slice(3);
  const sidecar = { eventId };

  for (let i = 0; i < flagArgs.length; i++) {
    const arg = flagArgs[i];
    if (arg === '--input-tokens' && flagArgs[i + 1]) {
      sidecar.inputTokens = parseInt(flagArgs[++i], 10);
    } else if (arg === '--output-tokens' && flagArgs[i + 1]) {
      sidecar.outputTokens = parseInt(flagArgs[++i], 10);
    } else if (arg === '--cache-read-tokens' && flagArgs[i + 1]) {
      sidecar.cacheReadTokens = parseInt(flagArgs[++i], 10);
    } else if (arg === '--cache-write-tokens' && flagArgs[i + 1]) {
      sidecar.cacheWriteTokens = parseInt(flagArgs[++i], 10);
    } else if (arg === '--token-source' && flagArgs[i + 1]) {
      sidecar.tokenSource = flagArgs[++i];
    } else if (arg === '--model' && flagArgs[i + 1]) {
      sidecar.model = flagArgs[++i];
    } else if (arg === '--provider' && flagArgs[i + 1]) {
      sidecar.provider = flagArgs[++i];
    } else if (arg === '--duration-minutes' && flagArgs[i + 1]) {
      sidecar.durationMinutes = parseFloat(flagArgs[++i]);
    } else if (arg === '--estimated-cost-usd') {
      console.error('Error: --estimated-cost-usd is no longer accepted. Cost is derived at collate time from lib/pricing.cjs using authoritative per-model pricing.');
      process.exit(1);
    }
  }

  // Auto-populate model from environment when --model flag not provided (FORGE-S12-T06).
  // An explicit --model flag takes priority — discoverModel() is only a fallback.
  if (!sidecar.model) {
    sidecar.model = discoverModel();
  }

  // Validate against sidecar schema
  const sidecarErrors = validateRecord(sidecar, schemas['event-sidecar']);
  if (sidecarErrors.length > 0) {
    for (const e of sidecarErrors) console.error(e);
    process.exit(1);
  }

  const sidecarDir = resolveSidecarDir(sprintId);
  const filePath = sidecarPath(sprintId, eventId);

  if (DRY_RUN) {
    console.log(`[dry-run] would write sidecar _${eventId}_usage.json`);
  } else {
    if (!fs.existsSync(sidecarDir)) {
      fs.mkdirSync(sidecarDir, { recursive: true });
    }
    fs.writeFileSync(filePath, JSON.stringify(sidecar, null, 2) + '\n', 'utf8');
  }
  if (VERBOSE) console.log(JSON.stringify({ ok: true, sidecar: true, eventId, sprintId, dryRun: DRY_RUN }));
}

function cmdPurgeEvents() {
  const sprintId = args[1];

  if (!sprintId) {
    console.error('Usage: store-cli.cjs purge-events <sprintId>');
    process.exit(1);
  }

  const result = store.purgeEvents(sprintId, { dryRun: DRY_RUN });
  if (DRY_RUN && !result.purged) {
    console.log(`[dry-run] would purge ${result.fileCount} event(s) for ${sprintId}`);
  }
  if (VERBOSE) console.log(JSON.stringify(result, null, 2));
}

function cmdResolveEvent() {
  // resolve-event <sprintId> <eventId> --status <status> [--proposal-ref <path>] [--bug-ref <url>]
  const sprintId = args[1];
  const eventId = args[2];

  if (!sprintId || !eventId) {
    console.error('Usage: store-cli.cjs resolve-event <sprintId> <eventId> --status <status> [--proposal-ref <path>] [--bug-ref <url>]');
    console.error('  Statuses: open, resolved, wontfix, deferred');
    process.exit(1);
  }

  // Parse flags
  const flagArgs = args.slice(3);
  let resolutionStatus = null;
  let proposalRef = null;
  let bugRef = null;

  for (let i = 0; i < flagArgs.length; i++) {
    const arg = flagArgs[i];
    if (arg === '--status' && flagArgs[i + 1]) {
      resolutionStatus = flagArgs[++i];
    } else if (arg === '--proposal-ref' && flagArgs[i + 1]) {
      proposalRef = flagArgs[++i];
    } else if (arg === '--bug-ref' && flagArgs[i + 1]) {
      bugRef = flagArgs[++i];
    }
  }

  const VALID_STATUSES = ['open', 'resolved', 'wontfix', 'deferred'];
  if (!resolutionStatus || !VALID_STATUSES.includes(resolutionStatus)) {
    console.error(`Invalid resolution status: ${resolutionStatus}. Must be one of: ${VALID_STATUSES.join(', ')}`);
    process.exit(1);
  }

  // Read the event
  const eventPath = canonicalEventPath(sprintId, eventId);
  if (!fs.existsSync(eventPath)) {
    console.error(`Event not found: ${eventPath}`);
    process.exit(1);
  }

  let event;
  try {
    event = JSON.parse(fs.readFileSync(eventPath, 'utf8'));
  } catch (e) {
    console.error(`Invalid event JSON: ${e.message}`);
    process.exit(1);
  }

  // Build resolution object
  const resolution = { status: resolutionStatus };
  if (proposalRef) resolution.proposalRef = proposalRef;
  if (bugRef) resolution.bugRef = bugRef;
  resolution.resolvedAt = new Date().toISOString();
  // resolvedBy left absent — orchestrator fills if available

  event.resolution = resolution;

  // Validate the updated event against the schema
  const errors = validateRecord(event, schemas.event);
  if (errors.length > 0) {
    for (const e of errors) console.error(e);
    process.exit(1);
  }

  if (DRY_RUN) {
    console.log(`[dry-run] would resolve event ${eventId} with status=${resolutionStatus}`);
  } else {
    store.writeEvent(sprintId, event);
  }
  if (VERBOSE) console.log(JSON.stringify({ ok: true, event: true, eventId, sprintId, resolution, dryRun: DRY_RUN }));
}

function cmdWriteCollationState() {
  const jsonStr = args[1];

  if (!jsonStr) {
    console.error('Usage: store-cli.cjs write-collation-state \'<json>\'');
    process.exit(1);
  }

  let data;
  try {
    data = JSON.parse(jsonStr);
  } catch (e) {
    console.error(`Invalid JSON: ${e.message}`);
    process.exit(1);
  }

  const csErrors = validateRecord(data, schemas['collation-state']);
  if (csErrors.length > 0) {
    for (const e of csErrors) console.error(e);
    process.exit(1);
  }

  if (DRY_RUN) {
    console.log('[dry-run] would write COLLATION_STATE.json');
  } else {
    store.writeCollationState(data);
  }
  if (VERBOSE) console.log(JSON.stringify({ ok: true, dryRun: DRY_RUN }));
}

function cmdProgress() {
  const sprintOrBugId = args[1];
  const agentName = args[2];
  const bannerKey = args[3];
  const status = args[4];
  const detail = args.slice(5).join(' ');

  if (!sprintOrBugId || !agentName || !bannerKey || !status) {
    console.error('Usage: store-cli.cjs progress <sprintOrBugId> <agentName> <bannerKey> <status> [detail]');
    console.error('  status: start | progress | done | error');
    process.exit(1);
  }

  const timestamp = new Date().toISOString();

  const progressErrors = validateRecord(
    { timestamp, agentName, bannerKey, status, detail: detail || '' },
    schemas['progress-entry']
  );
  if (progressErrors.length > 0) {
    for (const e of progressErrors) console.error(e);
    process.exit(1);
  }

  const line = `${timestamp}|${agentName}|${bannerKey}|${status}|${detail}\n`;

  const dir = _resolveEventsDir(sprintOrBugId);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  const logPath = path.join(dir, 'progress.log');
  fs.appendFileSync(logPath, line, 'utf8');

  // Emit human-readable summary to stdout
  let banners;
  try { banners = require('./banners.cjs'); } catch { banners = null; }
  let emoji = bannerKey;
  if (banners && typeof banners.mark === 'function') {
    try { emoji = banners.mark(bannerKey); } catch { emoji = bannerKey; }
  }
  const summary = `${emoji}  ${agentName}  [${status}]${detail ? '  ' + detail : ''}`;
  if (VERBOSE) process.stdout.write(summary + '\n');
}

function cmdProgressClear() {
  const sprintOrBugId = args[1];

  if (!sprintOrBugId) {
    console.error('Usage: store-cli.cjs progress-clear <sprintOrBugId>');
    process.exit(1);
  }

  const dir = _resolveEventsDir(sprintOrBugId);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  const logPath = path.join(dir, 'progress.log');
  fs.writeFileSync(logPath, '', 'utf8');
  if (VERBOSE) console.log(`Cleared ${logPath}`);
}

function cmdValidate() {
  const entity = args[1];
  const jsonStr = args[2];

  if (!entity || !jsonStr) {
    console.error('Usage: store-cli.cjs validate \u003centity\u003e \'\u003cjson\u003e\'');
    process.exit(1);
  }

  if (!ENTITY_TYPES.includes(entity)) {
    console.error(`Unknown entity type: ${entity}${formatSuggestion(suggestEntityType(entity))}`);
    process.exit(1);
  }

  let data;
  try {
    data = JSON.parse(jsonStr);
  } catch (e) {
    console.error(`Invalid JSON: ${e.message}`);
    process.exit(1);
  }

  const errors = validateRecord(data, schemas[entity], { entity });
  if (errors.length > 0) {
    for (const e of errors) console.error(e);
    process.exit(1);
  }

  if (VERBOSE) console.log(JSON.stringify({ ok: true, entity, valid: true }));
}

function _setSummaryOnEntity(entityKind, entityId, phase, summaryFilePath) {
  if (!VALID_SUMMARY_PHASES.has(phase)) {
    console.error(`Unknown phase "${phase}". Valid phases: ${[...VALID_SUMMARY_PHASES].join(', ')}`);
    process.exit(1);
  }

  // Load entity first — its `path` is the authoritative artifact directory and
  // is needed both to self-resolve the sidecar (when no file arg is given) and
  // to merge the summary below.
  const record = entityKind === 'task' ? store.getTask(entityId) : store.getBug(entityId);
  if (!record) {
    console.error(`${entityKind} not found: ${entityId}`);
    process.exit(1);
  }

  // ADR artifact-resolution Phase 1: when the caller omits the JSON file, derive
  // the sidecar from record.path + the canonical phase→filename map. The agent
  // never hand-builds the path (kills the Class-1 failures + the arity bug).
  let selfResolved = false;
  if (!summaryFilePath) {
    if (typeof record.path !== 'string' || record.path.length === 0) {
      console.error(`Cannot self-resolve summary sidecar: ${entityKind} ${entityId} has no "path". Pass an explicit <jsonFile>.`);
      process.exit(1);
    }
    // record.path is the entity directory; defensively strip a trailing filename.
    const dir = /\.(md|json)$/i.test(record.path) ? path.dirname(record.path) : record.path;
    summaryFilePath = path.join(dir, resolveSummaryFilename(entityKind, phase));
    selfResolved = true;
  }

  // Read and validate summary JSON
  if (!fs.existsSync(summaryFilePath)) {
    // v1.0.10: self-resolve looks for the CANONICAL filename. If an agent wrote a
    // non-canonical sidecar (e.g. VALIDATE-SUMMARY.json via the Write tool instead
    // of forge_artifact's VALIDATION-SUMMARY.json), surface the near-name file so
    // the error is fixable in one step rather than a silent dead-end.
    let hint = '';
    if (selfResolved) {
      try {
        const dir = path.dirname(summaryFilePath);
        const expected = path.basename(summaryFilePath);
        const nearby = fs.readdirSync(dir).filter((f) => f !== expected && f.endsWith('.json') && /summary/i.test(f));
        if (nearby.length > 0) {
          const kind = PHASE_TO_KIND[phase];
          hint = ` (expected ${expected}; found ${nearby.join(', ')} in the same dir — write the sidecar via ` +
            `forge_artifact artifact:"${kind}", or rename it to ${expected})`;
        }
      } catch (_) { /* dir unreadable — fall back to the plain message */ }
    }
    console.error(`Summary file not found: ${summaryFilePath}${hint}`);
    process.exit(1);
  }

  let summary;
  try {
    summary = JSON.parse(fs.readFileSync(summaryFilePath, 'utf8'));
  } catch (e) {
    console.error(`Invalid JSON in summary file: ${e.message}`);
    process.exit(1);
  }

  const errors = validateRecord(summary, PHASE_SUMMARY_SCHEMA);
  if (errors.length > 0) {
    for (const e of errors) console.error(e);
    process.exit(1);
  }

  // Merge summary (record was loaded above for path resolution)
  if (!record.summaries) record.summaries = {};
  record.summaries[phase] = summary;

  // Atomic write: tmp + rename
  const entityDirKey = entityKind === 'task' ? 'tasks' : 'bugs';
  const idField = entityKind === 'task' ? record.taskId : record.bugId;
  const storeRoot = store.impl.storeRoot;
  const filePath = path.join(storeRoot, entityDirKey, `${idField}.json`);
  const tmpPath = filePath + '.tmp';

  if (DRY_RUN) {
    console.log(`[dry-run] would set ${entityKind} ${entityId} summaries.${phase}`);
  } else {
    fs.writeFileSync(tmpPath, JSON.stringify(record, null, 2) + '\n', 'utf8');
    fs.renameSync(tmpPath, filePath);
  }

  if (VERBOSE) console.log(JSON.stringify({ ok: true, entityKind, id: entityId, phase, dryRun: DRY_RUN }));
}

// ---------------------------------------------------------------------------
// Summary read-through commands (FORGE-S22-T02)
//
// `get-summary <taskId> <phase> [--json]` and `get-bug-summary <bugId> <phase>
// [--json]` extract a single phase summary from a task/bug record. Registered
// as direct switch cases (NOT via ALIAS_MAP) so they cannot accidentally
// route through a write verb (e.g. set-bug-summary).
// ---------------------------------------------------------------------------
function _readSummary(entityKind, id, phase, asJson) {
  const record = entityKind === 'task' ? store.getTask(id) : store.getBug(id);
  if (!record) {
    console.error(`error: ${entityKind} ${id} not found`);
    process.exit(1);
  }
  const summary = record.summaries && record.summaries[phase];
  if (summary === undefined || summary === null) {
    console.error(`error: no summary for phase '${phase}' on ${entityKind} ${id}`);
    process.exit(1);
  }
  if (asJson) {
    console.log(JSON.stringify({ entityId: id, phase, summary }, null, 2));
  } else {
    if (typeof summary === 'string') {
      console.log(summary);
    } else {
      console.log(JSON.stringify(summary, null, 2));
    }
  }
}

function cmdGetSummary() {
  const taskId = args[1];
  const phase  = args[2];
  const asJson = args.includes('--json');
  if (!taskId || !phase) {
    console.error('Usage: store-cli.cjs get-summary <taskId> <phase> [--json]');
    process.exit(1);
  }
  _readSummary('task', taskId, phase, asJson);
}

function cmdGetBugSummary() {
  const bugId = args[1];
  const phase = args[2];
  const asJson = args.includes('--json');
  if (!bugId || !phase) {
    console.error('Usage: store-cli.cjs get-bug-summary <bugId> <phase> [--json]');
    process.exit(1);
  }
  _readSummary('bug', bugId, phase, asJson);
}

function cmdSetSummary() {
  const taskId      = args[1];
  const phase       = args[2];
  const summaryFile = args[3];

  if (!taskId || !phase) {
    console.error('Usage: store-cli.cjs set-summary <taskId> <phase> [<jsonFile>]');
    process.exit(1);
  }

  _setSummaryOnEntity('task', taskId, phase, summaryFile);
}

function cmdSetBugSummary() {
  const bugId       = args[1];
  const phase       = args[2];
  const summaryFile = args[3];

  if (!bugId || !phase) {
    console.error('Usage: store-cli.cjs set-bug-summary <bugId> <phase> [<jsonFile>]');
    process.exit(1);
  }

  _setSummaryOnEntity('bug', bugId, phase, summaryFile);
}

// ---------------------------------------------------------------------------
// describe / template — schema introspection helpers (FORGE-BUG-029-friction)
//
// Reduce write→reject→retry friction. `describe <entity>` returns the raw
// JSON Schema; `template <entity>` walks the schema and returns a canonical
// sample record with required fields populated.
// ---------------------------------------------------------------------------

const VALID_ENTITY_DESCRIBE = new Set(['sprint', 'task', 'bug', 'event', 'feature']);

function cmdDescribe() {
  const entity = args[1];
  if (!entity) {
    console.error('Usage: store-cli.cjs describe <entity>');
    console.error(`Entities: ${Array.from(VALID_ENTITY_DESCRIBE).join(', ')}`);
    process.exit(1);
  }
  if (!VALID_ENTITY_DESCRIBE.has(entity)) {
    console.error(`Unknown entity: ${entity}${formatSuggestion(suggestEntityType(entity, Array.from(VALID_ENTITY_DESCRIBE)))}`);
    console.error(`Entities: ${Array.from(VALID_ENTITY_DESCRIBE).join(', ')}`);
    process.exit(1);
  }
  const schema = _getSchemas()[entity];
  if (!schema) {
    console.error(`Schema not found for entity: ${entity}`);
    process.exit(1);
  }
  console.log(JSON.stringify(schema, null, 2));
}

// Field-name → placeholder ID heuristics. Covers the common shapes without
// inventing schema-specific knowledge in the generator itself.
function _idPlaceholder(entity, fieldName) {
  if (fieldName === 'sprintId')   return 'PROJECT-S01';
  if (fieldName === 'taskId')     return 'PROJECT-S01-T01';
  if (fieldName === 'bugId')      return 'PROJECT-BUG-001';
  if (fieldName === 'feature_id' || fieldName === 'featureId') return 'PROJECT-F01';
  if (fieldName === 'eventId')    return '20260101T000000000Z_PROJECT-S01-T01_phase_start';
  if (entity === 'feature' && fieldName === 'id') return 'PROJECT-F01';
  return `<${fieldName}>`;
}

function _generateSample(schema, entity) {
  const sample = {};
  const required = schema.required || [];
  const properties = schema.properties || {};
  for (const field of required) {
    const def = properties[field] || {};
    sample[field] = _generateValue(def, field, entity);
  }
  return sample;
}

function _generateValue(def, fieldName, entity) {
  // Resolve type — handle union (array of types) by picking the first non-null.
  let t = def.type;
  if (Array.isArray(t)) t = t.find((x) => x !== 'null') || t[0];

  if (def.enum && Array.isArray(def.enum) && def.enum.length > 0) {
    return def.enum[0];
  }
  if (t === 'string') {
    if (def.format === 'date-time') return new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');
    if (/Id$|^id$/.test(fieldName) || fieldName === 'feature_id' || fieldName === 'eventId') {
      return _idPlaceholder(entity, fieldName);
    }
    if (fieldName === 'title')       return 'Sample title';
    if (fieldName === 'description') return 'Sample description';
    if (fieldName === 'path')        return `engineering/sprints/PROJECT-S01/${fieldName}-sample.md`;
    return `<${fieldName}>`;
  }
  if (t === 'integer' || t === 'number') return 0;
  if (t === 'boolean') return false;
  if (t === 'array') return [];
  if (t === 'object') return {};
  return null;
}

function cmdTemplate() {
  const entity = args[1];
  if (!entity) {
    console.error('Usage: store-cli.cjs template <entity>');
    console.error(`Entities: ${Array.from(VALID_ENTITY_DESCRIBE).join(', ')}`);
    process.exit(1);
  }
  if (!VALID_ENTITY_DESCRIBE.has(entity)) {
    console.error(`Unknown entity: ${entity}${formatSuggestion(suggestEntityType(entity, Array.from(VALID_ENTITY_DESCRIBE)))}`);
    console.error(`Entities: ${Array.from(VALID_ENTITY_DESCRIBE).join(', ')}`);
    process.exit(1);
  }
  const schema = _getSchemas()[entity];
  if (!schema) {
    console.error(`Schema not found for entity: ${entity}`);
    process.exit(1);
  }
  const sample = _generateSample(schema, entity);
  console.log(JSON.stringify(sample, null, 2));
}

// ---------------------------------------------------------------------------
// Command dispatch
// ---------------------------------------------------------------------------

switch (command) {
  case 'write':                cmdWrite(); break;
  case 'read':                 cmdRead(); break;
  case 'list':                 cmdList(); break;
  case 'delete':               cmdDelete(); break;
  case 'update-status':        cmdUpdateStatus(); break;
  case 'emit':                 cmdEmit(); break;
  case 'merge-sidecar':        cmdMergeSidecar(); break;
  case 'record-usage':        cmdRecordUsage(); break;
  case 'purge-events':         cmdPurgeEvents(); break;
  case 'resolve-event':        cmdResolveEvent(); break;
  case 'write-collation-state': cmdWriteCollationState(); break;
  case 'validate':             cmdValidate(); break;
  case 'progress':             cmdProgress(); break;
  case 'progress-clear':       cmdProgressClear(); break;
  case 'set-summary':          cmdSetSummary(); break;
  case 'set-bug-summary':      cmdSetBugSummary(); break;
  case 'get-summary':          cmdGetSummary(); break;
  case 'get-bug-summary':      cmdGetBugSummary(); break;
  case 'describe':             cmdDescribe(); break;
  case 'template':             cmdTemplate(); break;
  case 'query':
  case 'nlp':
  case 'schema': {
    // Delegate to store-query.cjs — query engine lives there
    const { spawnSync } = require('child_process');
    const queryBin = path.join(__dirname, 'store-query.cjs');
    const result = spawnSync(process.execPath, [queryBin, command, ...args.slice(1)], {
      stdio: 'inherit',
      cwd: process.cwd(),
    });
    process.exit(result.status ?? 1);
    break;
  }
  default: {
    // Build candidate pool: all command names + ALIAS_MAP keys
    const commandCandidates = [
      'write', 'read', 'list', 'delete', 'update-status', 'emit', 'merge-sidecar',
      'record-usage', 'purge-events', 'resolve-event', 'write-collation-state',
      'validate', 'progress', 'progress-clear', 'set-summary', 'set-bug-summary',
      'get-summary', 'get-bug-summary', 'describe', 'template', 'query', 'nlp', 'schema',
      ...Object.keys(ALIAS_MAP)
    ];
    const cmdSuggestions = suggest(command, commandCandidates);
    console.error(`Unknown command: ${command}${formatSuggestion(cmdSuggestions)}`);
    console.error('Run with --help for usage information.');
    process.exit(1);
  }
}

} catch (err) {
  console.error(err.message);
  process.exit(1);
}

} // end if (require.main === module)