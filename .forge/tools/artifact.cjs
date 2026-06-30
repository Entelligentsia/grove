#!/usr/bin/env node
'use strict';

// Forge tool: artifact
// Read, write, or list phase artifacts (PLAN.md, PROGRESS.md, *-SUMMARY.json, etc.)
// for a task, bug, or sprint. Resolves paths from entity store record.
//
// Usage:
//   node artifact.cjs read <entity> <entityId> <artifact>
//   node artifact.cjs write <entity> <entityId> <artifact> <content|@file>
//   node artifact.cjs list <entity> <entityId>
//
// Exit codes: 0 = success, 1 = usage/validation error, 2 = not found

const fs = require('fs');
const path = require('path');
const { findProjectRoot } = require('./lib/project-root.cjs');

// ── Artifact catalog ─────────────────────────────────────────────────────────
//
// The catalog, bug-mode overrides, and filename resolution now live in the
// canonical registry at lib/artifact-kinds.cjs (ADR artifact-resolution Phase 1),
// consumed here and by store-cli.cjs so there is ONE source of truth.
const {
  ARTIFACT_CATALOG,
  ARTIFACT_FILENAME_OVERRIDES,
  ARTIFACT_NAMES,
  resolveArtifactFilename,
} = require('./lib/artifact-kinds.cjs');

// The provider seam (ArtifactStore/FsArtifactImpl) and entity-dir resolution
// live in artifact-store.cjs (ADR Phase 3). This CLI is a thin layer over the
// facade: it owns arg parsing, @-file expansion, JSON validation, and display;
// all filesystem access goes through the provider.
const artifactStore = require('./artifact-store.cjs');
const { ArtifactStore, FsArtifactImpl, resolveEntityDir } = artifactStore;

// ── Summary JSON validation ──────────────────────────────────────────────────

const SUMMARY_REQUIRED = ['objective', 'key_changes', 'verdict', 'written_at'];

function validateSummaryJson(content) {
  let obj;
  try {
    obj = JSON.parse(content);
  } catch (e) {
    return `Invalid JSON: ${e.message}`;
  }
  const missing = SUMMARY_REQUIRED.filter((f) => !(f in obj));
  if (missing.length > 0) return `Missing required fields: ${missing.join(', ')}`;
  if (typeof obj.objective !== 'string') return `"objective" must be a string`;
  if (!Array.isArray(obj.key_changes)) return `"key_changes" must be an array`;
  if (typeof obj.verdict !== 'string') return `"verdict" must be a string`;
  if (typeof obj.written_at !== 'string') return `"written_at" must be a string`;
  return null;
}

// ── CLI ───────────────────────────────────────────────────────────────────────

if (require.main === module) {

const argv = process.argv.slice(2);

if (argv.length === 0 || argv[0] === '--help' || argv[0] === '-h') {
  process.stderr.write([
    'Usage: node artifact.cjs <subcommand> <entity> <entityId> [artifact] [content|@file]',
    '',
    'Subcommands:',
    '  list <entity> <entityId>                   List existing artifacts',
    '  read <entity> <entityId> <artifact>         Read artifact content',
    '  write <entity> <entityId> <artifact> <content|@file>',
    '                                              Write artifact (content or @/path/to/file)',
    '  exists <entity> <entityId> <artifact>       Exit 0 if present, 2 if absent',
    '  url <entity> <entityId> <artifact>          Print the backend URL (file:// for fs)',
    '  delete <entity> <entityId> <artifact>       Delete an artifact',
    '',
    'Entities: task, bug, sprint',
    `Known artifacts: ${ARTIFACT_NAMES.join(', ')}`,
    '',
    'Exit codes: 0=success, 1=usage/validation error, 2=not found',
  ].join('\n') + '\n');
  process.exit(1);
}

const [subcmd, entity, entityId] = argv;

if (!subcmd || !entity || !entityId) {
  process.stderr.write('Usage: artifact.cjs <list|read|write> <entity> <entityId> [artifact] [content]\n');
  process.exit(1);
}

const VALID_ENTITIES = ['task', 'bug', 'sprint'];
if (!VALID_ENTITIES.includes(entity)) {
  process.stderr.write(`Unknown entity type: ${entity}. Valid: ${VALID_ENTITIES.join(', ')}\n`);
  process.exit(1);
}

const projectRoot = findProjectRoot();
if (!projectRoot) {
  process.stderr.write('Cannot find project root (.forge/config.json not found)\n');
  process.exit(1);
}

// Read engineering path from config
let engineeringPath = 'engineering';
try {
  const cfg = JSON.parse(fs.readFileSync(path.join(projectRoot, '.forge', 'config.json'), 'utf8'));
  if (typeof cfg.paths?.engineering === 'string') engineeringPath = cfg.paths.engineering;
} catch (_) { /* default */ }

const toolDir = __dirname;

const entityDir = resolveEntityDir(entity, entityId, engineeringPath, toolDir, projectRoot);
if (!entityDir) {
  process.stderr.write(
    `Cannot resolve ${entity} directory for "${entityId}". ` +
    `Expected ID pattern: task=PREFIX-SNN-TNN, bug=PREFIX-BNN[-slug], sprint=PREFIX-SNN.\n`
  );
  process.exit(1);
}

const absDir = path.resolve(projectRoot, entityDir);

// Provider instance — all filesystem access for read/write/exists/url/delete
// goes through the facade (ADR Phase 3). Wired with the already-resolved dir so
// the CLI and the provider agree on location without a second store read.
const store = new ArtifactStore(new FsArtifactImpl({
  projectRoot, engineeringPath, toolDir,
  resolveDir: () => entityDir,
}));

// ── list ────────────────────────────────────────────────────────────────────

if (subcmd === 'list') {
  if (!fs.existsSync(absDir)) {
    process.stdout.write(`No artifacts found — directory does not exist: ${entityDir}/\n`);
    process.exit(0);
  }
  const files = fs.readdirSync(absDir).filter((f) => f.endsWith('.md') || f.endsWith('.json'));
  const known = [];
  const other = [];
  for (const f of files) {
    // Recognise the per-entity overrides as well, so e.g. BUG_FIX_PLAN.md
    // surfaces as the `plan` artifact in bug-mode listings.
    const overrideEntry = ARTIFACT_FILENAME_OVERRIDES[entity]
      ? Object.entries(ARTIFACT_FILENAME_OVERRIDES[entity]).find(([, fn]) => fn === f)
      : undefined;
    const catalogEntry = overrideEntry
      || Object.entries(ARTIFACT_CATALOG).find(([, v]) => v.filename === f);
    if (catalogEntry) {
      known.push(`  ${catalogEntry[0]} → ${f}`);
    } else {
      other.push(`  (unlisted) ${f}`);
    }
  }
  const lines = [`Artifacts in ${entityDir}/:`];
  if (known.length > 0) lines.push(...known);
  if (other.length > 0) lines.push(...other);
  if (known.length === 0 && other.length === 0) lines.push('  (empty)');
  process.stdout.write(lines.join('\n') + '\n');
  process.exit(0);
}

// ── read / write — need artifact name ───────────────────────────────────────

const artifactName = argv[3];
if (!artifactName) {
  process.stderr.write(`"artifact" is required for ${subcmd}. Known: ${ARTIFACT_NAMES.join(', ')}\n`);
  process.exit(1);
}

const catalogEntry = ARTIFACT_CATALOG[artifactName];
if (!catalogEntry) {
  const suggestions = ARTIFACT_NAMES.filter((n) => n.includes(artifactName.toLowerCase()));
  process.stderr.write(
    `Unknown artifact "${artifactName}". Known: ${ARTIFACT_NAMES.join(', ')}.` +
    (suggestions.length > 0 ? ` Did you mean: ${suggestions.join(', ')}?` : '') + '\n'
  );
  process.exit(1);
}

const resolvedFilename = resolveArtifactFilename(entity, artifactName);
const displayPath = path.join(entityDir, resolvedFilename);
const handle = { entity, entityId, kind: artifactName };

// ── read ─────────────────────────────────────────────────────────────────────

if (subcmd === 'read') {
  if (!store.exists(handle)) {
    process.stderr.write(`Artifact not found: ${displayPath}\n`);
    process.exit(2);
  }
  process.stdout.write(store.read(handle));
  process.exit(0);
}

// ── exists ─────────────────────────────────────────────────────────────────────

if (subcmd === 'exists') {
  // Boolean check: always exit 0 with a "true"/"false" token so callers
  // (including the forge-cli subprocess surface) never treat absence as an error.
  process.stdout.write(store.exists(handle) ? 'true\n' : 'false\n');
  process.exit(0);
}

// ── url ────────────────────────────────────────────────────────────────────────

if (subcmd === 'url') {
  process.stdout.write(store.url(handle) + '\n');
  process.exit(0);
}

// ── delete ─────────────────────────────────────────────────────────────────────

if (subcmd === 'delete') {
  const removed = store.delete(handle);
  process.stdout.write(removed ? `Deleted ${displayPath}\n` : `Nothing to delete: ${displayPath}\n`);
  process.exit(0);
}

// ── write ─────────────────────────────────────────────────────────────────────

if (subcmd === 'write') {
  let rawContent = argv[4];
  if (!rawContent) {
    process.stderr.write('"content" is required for write. Pass inline or use @/path/to/file for large content.\n');
    process.exit(1);
  }

  // @-prefix convention: read content from file when arg starts with @
  let content;
  if (rawContent.startsWith('@')) {
    const contentFile = rawContent.slice(1);
    if (!fs.existsSync(contentFile)) {
      process.stderr.write(`@-file not found: ${contentFile}\n`);
      process.exit(1);
    }
    content = fs.readFileSync(contentFile, 'utf8');
  } else {
    content = rawContent;
  }

  if (catalogEntry.type === 'json') {
    const validationError = validateSummaryJson(content);
    if (validationError) {
      process.stderr.write(
        `Summary validation failed for ${resolvedFilename}: ${validationError}. ` +
        `Required fields: ${SUMMARY_REQUIRED.join(', ')}.\n`
      );
      process.exit(1);
    }
  }

  const res = store.write(handle, content);
  process.stdout.write(`Wrote ${res.bytes} bytes to ${displayPath}\n`);
  process.exit(0);
}

process.stderr.write(`Unknown subcommand: ${subcmd}. Valid: list, read, write, exists, url, delete\n`);
process.exit(1);

} // end if (require.main === module)

module.exports = { ARTIFACT_CATALOG, ARTIFACT_NAMES, validateSummaryJson, resolveEntityDir };
