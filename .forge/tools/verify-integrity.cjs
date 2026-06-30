#!/usr/bin/env node
'use strict';

// Forge tool: verify-integrity
// Runtime integrity verifier — reads integrity.json, re-hashes each file, reports drift.
// Usage: node verify-integrity.cjs [--forge-root <path>]
// Exit codes: 0 = all clean, 1 = one or more files modified or missing, 2 = manifest missing

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

function computeHash(filePath) {
  const content = fs.readFileSync(filePath);
  return crypto.createHash('sha256').update(content).digest('hex');
}

function verifyIntegrity(forgeRoot) {
  const manifestPath = path.join(forgeRoot, 'integrity.json');

  if (!fs.existsSync(manifestPath)) {
    const output = '× integrity.json not found — run /forge:update to restore';
    return { exitCode: 1, output, modified: [], missing: [] };
  }

  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  } catch (e) {
    const output = `× integrity.json is not valid JSON: ${e.message}`;
    return { exitCode: 1, output, modified: [], missing: [] };
  }

  const { files } = manifest;
  if (!files || typeof files !== 'object') {
    const output = '× integrity.json has no "files" field';
    return { exitCode: 1, output, modified: [], missing: [] };
  }

  const modified = [];
  const missing = [];

  for (const [rel, expectedHash] of Object.entries(files)) {
    const abs = path.join(forgeRoot, rel);
    if (!fs.existsSync(abs)) {
      missing.push(rel);
      continue;
    }
    const actual = computeHash(abs);
    if (actual !== expectedHash) {
      modified.push(rel);
    }
  }

  const totalFiles = Object.keys(files).length;
  const problemCount = modified.length + missing.length;

  if (problemCount === 0) {
    const output = `〇 Plugin integrity — all ${totalFiles} files unmodified`;
    return { exitCode: 0, output, modified: [], missing: [] };
  }

  const lines = [`△ Plugin integrity — ${problemCount} file${problemCount === 1 ? '' : 's'} modified or missing:`];
  for (const f of modified) {
    lines.push(`  · ${f} (hash mismatch — run /forge:update to restore)`);
  }
  for (const f of missing) {
    lines.push(`  · ${f} (missing — run /forge:update to restore)`);
  }

  return { exitCode: 1, output: lines.join('\n'), modified, missing };
}

module.exports = { verifyIntegrity };

if (require.main === module) {
  const args = process.argv.slice(2);
  const forgeRootIdx = args.indexOf('--forge-root');
  const forgeRoot = forgeRootIdx !== -1
    ? path.resolve(args[forgeRootIdx + 1])
    : path.resolve(__dirname, '..');

  const result = verifyIntegrity(forgeRoot);
  console.log(result.output);
  process.exit(result.exitCode);
}
