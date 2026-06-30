#!/usr/bin/env node
'use strict';

// Forge tool: generation-manifest
// Track and verify generated file integrity via content hashes.
// Usage: generation-manifest record <path>
//        generation-manifest record-all
//        generation-manifest check <path>
//        generation-manifest list [--modified]
//        generation-manifest status
//        generation-manifest remove <path>
//        generation-manifest clear-namespace <prefix>  Remove all entries whose path starts with <prefix>.
//                                                       prefix must start with .forge/ or .claude/ and end with /

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const MANIFEST_PATH = path.join(process.cwd(), '.forge', 'generation-manifest.json');

function normalize(content) {
  return content
    .replace(/\r\n/g, '\n')
    .split('\n')
    .map(line => line.trimEnd())
    .join('\n');
}

function hashContent(content) {
  return 'sha256:' + crypto.createHash('sha256').update(normalize(content), 'utf8').digest('hex');
}

// ── exports (for testing) ────────────────────────────────────────────────────

module.exports = { normalize, hashContent, MANIFEST_PATH };

// ── CLI guard ────────────────────────────────────────────────────────────────

if (require.main === module) {
process.on('uncaughtException', (e) => {
  process.stderr.write(`× ${e.message}\n`);
  process.exit(1);
});

function readManifest() {
  if (!fs.existsSync(MANIFEST_PATH)) return { files: {} };
  try {
    return JSON.parse(fs.readFileSync(MANIFEST_PATH, 'utf8'));
  } catch (e) {
    process.stderr.write(`× reading manifest: ${e.message}\n`);
    process.exit(1);
  }
}

function writeManifest(manifest) {
  const dir = path.dirname(MANIFEST_PATH);
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
  const tmp = MANIFEST_PATH + '.tmp.' + process.pid;
  try {
    fs.writeFileSync(tmp, JSON.stringify(manifest, null, 2) + '\n', 'utf8');
    fs.renameSync(tmp, MANIFEST_PATH);
  } catch (e) {
    try { fs.unlinkSync(tmp); } catch {}
    process.stderr.write(`× writing manifest: ${e.message}\n`);
    process.exit(1);
  }
}

function getForgeVersion() {
  try {
    const configPath = path.join(process.cwd(), '.forge', 'config.json');
    if (fs.existsSync(configPath)) {
      const cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));
      if (cfg.version) return cfg.version;
    }
  } catch {}
  return 'unknown';
}

function toRelPath(filePath) {
  return path.relative(process.cwd(), path.resolve(filePath));
}

function fileStatus(relPath, entry) {
  const absPath = path.resolve(relPath);
  if (!fs.existsSync(absPath)) return 'missing';
  const current = hashContent(fs.readFileSync(absPath, 'utf8'));
  return current === entry.hash ? 'pristine' : 'modified';
}

const STATUS_SYMBOL = { pristine: '〇', modified: '△', missing: '×' };

// ── subcommands ──────────────────────────────────────────────────────────────

const [,, subcmd, ...args] = process.argv;

if (!subcmd) {
  process.stderr.write([
    'Usage: generation-manifest <subcommand> [options]',
    '',
    'Subcommands:',
    '  record <path>        Hash and store/update a file in the manifest',
    '  record-all           Re-hash all files currently tracked in the manifest',
    '  check <path>         Exit 0=pristine  1=modified  2=untracked  3=file missing',
    '  list [--modified]    Table of tracked files with status',
    '  status               Summary counts',
    '  remove <path>        Remove a file from tracking',
    '  clear-namespace <prefix>  Remove all entries whose path starts with <prefix>.',
    '                            prefix must start with .forge/ or .claude/ and end with /',
  ].join('\n') + '\n');
  process.exit(2);
}

// ── record ───────────────────────────────────────────────────────────────────

if (subcmd === 'record') {
  const filePath = args[0];
  if (!filePath) {
    process.stderr.write('Usage: generation-manifest record <path>\n');
    process.exit(2);
  }
  const relPath = toRelPath(filePath);
  const absPath = path.resolve(filePath);
  if (!fs.existsSync(absPath)) {
    process.stderr.write(`× File not found: ${filePath}\n`);
    process.exit(1);
  }
  const manifest = readManifest();
  if (!manifest.files) manifest.files = {};
  manifest.files[relPath] = {
    hash: hashContent(fs.readFileSync(absPath, 'utf8')),
    generatedAt: new Date().toISOString(),
    generatedByVersion: getForgeVersion(),
  };
  writeManifest(manifest);
  process.stdout.write(`〇 Recorded: ${relPath}\n`);
  process.exit(0);
}

// ── record-all ───────────────────────────────────────────────────────────────

if (subcmd === 'record-all') {
  const manifest = readManifest();
  const files = manifest.files || {};
  if (Object.keys(files).length === 0) {
    process.stdout.write('── No files tracked — nothing to re-hash.\n');
    process.exit(0);
  }
  let updated = 0, missing = 0;
  const version = getForgeVersion();
  for (const relPath of Object.keys(files)) {
    const absPath = path.resolve(relPath);
    if (!fs.existsSync(absPath)) {
      process.stdout.write(`△ Missing (skipped): ${relPath}\n`);
      missing++;
      continue;
    }
    files[relPath] = {
      hash: hashContent(fs.readFileSync(absPath, 'utf8')),
      generatedAt: new Date().toISOString(),
      generatedByVersion: version,
    };
    updated++;
  }
  writeManifest(manifest);
  process.stdout.write(`〇 Re-hashed ${updated} file(s).${missing ? `  △ ${missing} missing.` : ''}\n`);
  process.exit(0);
}

// ── check ────────────────────────────────────────────────────────────────────

if (subcmd === 'check') {
  const filePath = args[0];
  if (!filePath) {
    process.stderr.write('Usage: generation-manifest check <path>\n');
    process.exit(2);
  }
  const relPath = toRelPath(filePath);
  const manifest = readManifest();
  const entry = manifest.files && manifest.files[relPath];

  if (!entry) {
    process.stdout.write(`── ${relPath}: untracked\n`);
    process.exit(2);
  }
  const absPath = path.resolve(relPath);
  if (!fs.existsSync(absPath)) {
    process.stdout.write(`× ${relPath}: file not found\n`);
    process.exit(3);
  }
  const current = hashContent(fs.readFileSync(absPath, 'utf8'));
  if (current === entry.hash) {
    process.stdout.write(`〇 ${relPath}: pristine\n`);
    process.exit(0);
  } else {
    process.stdout.write(`△ ${relPath}: modified (generated by v${entry.generatedByVersion || '?'})\n`);
    process.exit(1);
  }
}

// ── list ─────────────────────────────────────────────────────────────────────

if (subcmd === 'list') {
  const modifiedOnly = args.includes('--modified');
  const manifest = readManifest();
  const files = manifest.files || {};

  if (Object.keys(files).length === 0) {
    process.stdout.write('── No files tracked.\n');
    process.exit(0);
  }

  const rows = [];
  for (const [relPath, entry] of Object.entries(files)) {
    const status = fileStatus(relPath, entry);
    if (modifiedOnly && status === 'pristine') continue;
    rows.push({
      symbol: STATUS_SYMBOL[status],
      status,
      relPath,
      version: entry.generatedByVersion || '—',
      date: entry.generatedAt ? entry.generatedAt.slice(0, 10) : '—',
    });
  }

  if (rows.length === 0) {
    process.stdout.write('〇 All tracked files are pristine.\n');
    process.exit(0);
  }

  process.stdout.write('| Status | File | Version | Date |\n');
  process.stdout.write('|--------|------|---------|------|\n');
  for (const r of rows) {
    process.stdout.write(`| ${r.symbol} ${r.status} | \`${r.relPath}\` | ${r.version} | ${r.date} |\n`);
  }
  process.exit(0);
}

// ── status ───────────────────────────────────────────────────────────────────

if (subcmd === 'status') {
  const manifest = readManifest();
  const files = manifest.files || {};
  let pristine = 0, modified = 0, missing = 0;
  for (const [relPath, entry] of Object.entries(files)) {
    const s = fileStatus(relPath, entry);
    if (s === 'pristine') pristine++;
    else if (s === 'modified') modified++;
    else missing++;
  }
  const total = pristine + modified + missing;
  if (total === 0) {
    process.stdout.write('── No files tracked.\n');
    process.exit(0);
  }
  process.stdout.write(`── ${total} file(s) tracked\n`);
  if (pristine > 0) process.stdout.write(`〇 ${pristine} pristine\n`);
  if (modified > 0) process.stdout.write(`△ ${modified} modified\n`);
  if (missing  > 0) process.stdout.write(`× ${missing} missing\n`);
  process.exit(0);
}

// ── remove ───────────────────────────────────────────────────────────────────

if (subcmd === 'remove') {
  const filePath = args[0];
  if (!filePath) {
    process.stderr.write('Usage: generation-manifest remove <path>\n');
    process.exit(2);
  }
  const relPath = toRelPath(filePath);
  const manifest = readManifest();
  if (!manifest.files || !manifest.files[relPath]) {
    process.stderr.write(`× Not tracked: ${relPath}\n`);
    process.exit(1);
  }
  delete manifest.files[relPath];
  writeManifest(manifest);
  process.stdout.write(`〇 Removed from tracking: ${relPath}\n`);
  process.exit(0);
}

// ── clear-namespace ──────────────────────────────────────────────────────────

if (subcmd === 'clear-namespace') {
  const prefix = args[0];
  if (!prefix) {
    process.stderr.write('Usage: generation-manifest clear-namespace <prefix>\n');
    process.exit(2);
  }
  const validPrefix = (prefix.startsWith('.forge/') || prefix.startsWith('.claude/')) && prefix.endsWith('/');
  if (!validPrefix) {
    process.stderr.write('Usage error: prefix must start with .forge/ or .claude/ and end with /\n');
    process.exit(2);
  }
  const manifest = readManifest();
  const files = manifest.files || {};
  const keys = Object.keys(files).filter(k => k.startsWith(prefix));
  if (keys.length === 0) {
    process.stdout.write(`── No entries matching ${prefix}\n`);
  } else {
    for (const k of keys) delete files[k];
    writeManifest(manifest);
    process.stdout.write(`〇 Cleared ${keys.length} entries matching ${prefix}\n`);
  }
  process.exit(0);
}

process.stderr.write(`× Unknown subcommand: ${subcmd}\n`);
process.exit(2);
} // end if (require.main === module)
