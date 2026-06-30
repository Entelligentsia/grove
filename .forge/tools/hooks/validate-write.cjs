#!/usr/bin/env node
// Forge write-boundary hook — runs PreToolUse on Write / Edit / MultiEdit.
//
// Purpose: enforce Forge schemas at the filesystem boundary so agents remain
// free to bypass deterministic tools (store-cli), as long as any write they
// do against Forge-owned paths honors the schema contract.
//
// Protocol (Claude Code PreToolUse hook):
//   - stdin: JSON envelope { tool_name, tool_input, ... }
//   - exit 0: allow the tool call
//   - exit 2 with stderr payload: block the tool call and surface the message
//
// Fail-open philosophy: any internal error (unreadable schema, parse bug,
// unexpected tool input shape) exits 0 with a stderr warning. A broken
// validator must never block legitimate work — validate-store.cjs remains
// as a post-hoc auditor.

'use strict';

process.on('uncaughtException', (err) => {
  try { process.stderr.write(`forge validate-write: internal error (fail-open): ${err.message}\n`); } catch (_) {}
  process.exit(0);
});

const fs = require('fs');
const path = require('path');

const { matchRegistry } = require('./lib/write-registry.js');

// Schema resolution — delegates to lib/schema-loader.cjs which implements the
// same 4-path search chain (project → in-tree → plugin-installed → bundled)
// that store-cli.cjs uses. This ensures the hook sees the exact same schemas
// as tool writes do, and gains the bundled .schemas/ lookup that the previous
// inline loadSchema() was missing.
const PLUGIN_ROOT = process.env.CLAUDE_PLUGIN_ROOT || path.join(__dirname, '..');

function resolveValidator() {
  // store-cli's shared validator lives at forge/tools/lib/validate.js. Require
  // it relative to the plugin root so the hook works from both dev tree and
  // installed plugin cache.
  const candidates = [
    path.join(PLUGIN_ROOT, 'tools', 'lib', 'validate.js'),
    path.join(__dirname, '..', 'tools', 'lib', 'validate.js'),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return require(c);
  }
  throw new Error(`validate.js not found (looked in: ${candidates.join(', ')})`);
}

function resolveSchemaLoader() {
  // schema-loader.cjs lives at forge/tools/lib/schema-loader.cjs. Resolve it
  // relative to the plugin root so the hook works from both dev tree and
  // installed plugin cache.
  const candidates = [
    path.join(PLUGIN_ROOT, 'tools', 'lib', 'schema-loader.cjs'),
    path.join(__dirname, '..', 'tools', 'lib', 'schema-loader.cjs'),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return require(c);
  }
  throw new Error(`schema-loader.cjs not found (looked in: ${candidates.join(', ')})`);
}

function loadSchema(filename) {
  // filename is e.g. "task.schema.json"; the schema-loader key is "task".
  const typeKey = filename.replace(/\.schema\.json$/, '');
  const { loadSchemas } = resolveSchemaLoader();
  const schemas = loadSchemas();
  const schema = schemas[typeKey];
  if (!schema) {
    throw new Error(`schema not found: ${filename}`);
  }
  return schema;
}

function readStdinSync() {
  try {
    return fs.readFileSync(0, 'utf8');
  } catch (_) {
    return '';
  }
}

// Apply Edit semantics: replace old_string with new_string in contents.
// If replace_all is true, replace every occurrence; otherwise replace the
// first (and only) occurrence. Errors if old_string is absent or ambiguous
// when replace_all is false — mirrors the Edit tool contract.
function applyEdit(contents, oldStr, newStr, replaceAll) {
  if (oldStr === '' && contents === '') return newStr; // new-file Edit
  if (oldStr === '') throw new Error('Edit: old_string is empty');
  if (replaceAll) return contents.split(oldStr).join(newStr);
  const idx = contents.indexOf(oldStr);
  if (idx === -1) throw new Error('Edit: old_string not found in file');
  const next = contents.indexOf(oldStr, idx + oldStr.length);
  if (next !== -1) throw new Error('Edit: old_string is ambiguous (appears more than once)');
  return contents.slice(0, idx) + newStr + contents.slice(idx + oldStr.length);
}

function computePostEditContents(toolName, toolInput) {
  const filePath = toolInput.file_path;
  if (toolName === 'Write') {
    return { filePath, contents: toolInput.content != null ? String(toolInput.content) : '' };
  }
  const prior = fs.existsSync(filePath) ? fs.readFileSync(filePath, 'utf8') : '';
  if (toolName === 'Edit') {
    return {
      filePath,
      contents: applyEdit(prior, toolInput.old_string || '', toolInput.new_string || '', !!toolInput.replace_all),
    };
  }
  // MultiEdit
  let cur = prior;
  const edits = Array.isArray(toolInput.edits) ? toolInput.edits : [];
  for (const e of edits) {
    cur = applyEdit(cur, e.old_string || '', e.new_string || '', !!e.replace_all);
  }
  return { filePath, contents: cur, prior };
}

function parseProgressLine(line) {
  // timestamp|agentName|bannerKey|status|detail
  const parts = line.split('|');
  if (parts.length < 4) return null;
  return {
    timestamp: parts[0],
    agentName: parts[1],
    bannerKey: parts[2],
    status:    parts[3],
    detail:    parts.slice(4).join('|'),
  };
}

function validateProgressAppend(prior, proposed, validator, schema) {
  // Only validate lines that are NEW — anything already on disk is grandfathered.
  if (!proposed.startsWith(prior)) {
    // Wholesale rewrite, not an append. Validate every non-empty line.
    const lines = proposed.split('\n').filter(l => l.length > 0);
    return lines.flatMap((l, i) => annotateLine(validator, schema, l, i));
  }
  const suffix = proposed.slice(prior.length);
  const newLines = suffix.split('\n').filter(l => l.length > 0);
  return newLines.flatMap((l, i) => annotateLine(validator, schema, l, i));
}

function annotateLine(validator, schema, line, idx) {
  const rec = parseProgressLine(line);
  if (!rec) return [`line ${idx + 1}: malformed (expected 4+ pipe-delimited fields)`];
  const errors = validator.validateRecord(rec, schema);
  return errors.map(e => `line ${idx + 1}: ${e}`);
}

function writeBypassAudit(filePath, reason) {
  try {
    const m = /\/\.forge\/store\/events\/([^/]+)\//.exec(filePath) || /\.forge\/store\/events\/([^/]+)\//.exec(filePath);
    const bucket = m ? m[1] : 'unknown';
    const logPath = path.join(process.cwd(), '.forge', 'store', 'events', bucket, 'progress.log');
    if (!fs.existsSync(path.dirname(logPath))) return;
    const ts = new Date().toISOString();
    const line = `${ts}|forge-hook|write-boundary|progress|${reason}\n`;
    fs.appendFileSync(logPath, line, 'utf8');
  } catch (_) { /* audit best-effort */ }
}

function block(message) {
  process.stderr.write(message + '\n');
  process.exit(2);
}

function main() {
  const raw = readStdinSync();
  if (!raw) process.exit(0);

  let envelope;
  try { envelope = JSON.parse(raw); } catch (_) { process.exit(0); }

  const toolName = envelope.tool_name;
  if (!['Write', 'Edit', 'MultiEdit'].includes(toolName)) process.exit(0);

  const toolInput = envelope.tool_input || {};
  const filePath = toolInput.file_path;
  if (!filePath || typeof filePath !== 'string') process.exit(0);

  const entry = matchRegistry(filePath);
  if (!entry) process.exit(0);

  if (process.env.FORGE_SKIP_WRITE_VALIDATION === '1') {
    writeBypassAudit(filePath, `FORGE_SKIP_WRITE_VALIDATION=1 bypass on ${toolName} ${path.relative(process.cwd(), filePath)}`);
    process.exit(0);
  }

  let validator, schema, post;
  try {
    validator = resolveValidator();
    schema = loadSchema(entry.schema);
    post = computePostEditContents(toolName, toolInput);
  } catch (err) {
    process.stderr.write(`forge validate-write: setup error (fail-open): ${err.message}\n`);
    process.exit(0);
  }

  const relPath = path.relative(process.cwd(), post.filePath);

  if (entry.format === 'line-pipe-delimited') {
    const prior = post.prior != null ? post.prior : (fs.existsSync(post.filePath) ? fs.readFileSync(post.filePath, 'utf8') : '');
    const errs = validateProgressAppend(prior, post.contents, validator, schema);
    if (errs.length > 0) {
      block(
        '❌ Forge schema violation — write blocked\n' +
        `Path: ${relPath}\n` +
        `Kind: ${entry.kind}\n` +
        `Violations:\n  - ${errs.join('\n  - ')}\n` +
        `Hint: progress.log lines must be "timestamp|agentName|bannerKey|status|detail"; see forge/schemas/${entry.schema}.\n` +
        'To bypass for one turn (emergency repair): FORGE_SKIP_WRITE_VALIDATION=1.'
      );
    }
    process.exit(0);
  }

  // JSON payloads
  let parsed;
  try {
    parsed = JSON.parse(post.contents);
  } catch (err) {
    block(
      '❌ Forge schema violation — write blocked\n' +
      `Path: ${relPath}\n` +
      `Kind: ${entry.kind}\n` +
      `Violation: Invalid JSON: ${err.message}\n` +
      `Hint: see forge/schemas/${entry.schema} for the expected shape.\n` +
      'To bypass for one turn (emergency repair): FORGE_SKIP_WRITE_VALIDATION=1.'
    );
  }

  const errs = validator.validateRecord(parsed, schema);
  if (errs.length > 0) {
    block(
      '❌ Forge schema violation — write blocked\n' +
      `Path: ${relPath}\n` +
      `Kind: ${entry.kind}\n` +
      `Violations:\n  - ${errs.join('\n  - ')}\n` +
      `Hint: see forge/schemas/${entry.schema} for the full shape.\n` +
      'To bypass for one turn (emergency repair): FORGE_SKIP_WRITE_VALIDATION=1.'
    );
  }

  process.exit(0);
}

main();
