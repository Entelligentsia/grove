#!/usr/bin/env node
'use strict';
// backfill-provider.cjs — Plan-11 / Slice 2 / #26.
//
// One-shot migration helper for 0.43.13 → 0.43.14. Walks
// `<root>/events/**/*.json` (default `<cwd>/.forge/store/events`) and stamps
// `provider: "unknown"` onto any event missing the field. Sidecar files
// (leading underscore) are skipped.
//
// Honest absence beats misleading presence — we stamp "unknown" rather than
// guessing a provider from the model name, because the old events were
// written before provider was a first-class field and there is no reliable
// way to recover it after the fact.

const fs = require('node:fs');
const path = require('node:path');

function parseArgs(argv) {
  const out = { root: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--root') {
      out.root = argv[++i];
    } else {
      process.stderr.write(`backfill-provider: unknown flag ${a}\n`);
      process.exit(2);
    }
  }
  return out;
}

function resolveEventsDir(rootArg) {
  if (rootArg) {
    if (path.basename(rootArg) === 'events') return rootArg;
    return path.join(rootArg, 'events');
  }
  return path.join(process.cwd(), '.forge', 'store', 'events');
}

function* walkJsonFiles(dir) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch (err) {
    if (err.code === 'ENOENT') return;
    throw err;
  }
  for (const ent of entries) {
    const fp = path.join(dir, ent.name);
    if (ent.isDirectory()) {
      yield* walkJsonFiles(fp);
    } else if (ent.isFile() && ent.name.endsWith('.json')) {
      yield fp;
    }
  }
}

function main(argv) {
  const args = parseArgs(argv);
  const eventsDir = resolveEventsDir(args.root);

  let scanned = 0;
  let stamped = 0;
  let alreadyHadProvider = 0;
  let skippedSidecar = 0;
  let skippedMalformed = 0;

  for (const fp of walkJsonFiles(eventsDir)) {
    if (path.basename(fp).startsWith('_')) {
      skippedSidecar++;
      continue;
    }
    scanned++;
    let raw;
    try {
      raw = fs.readFileSync(fp, 'utf8');
    } catch {
      skippedMalformed++;
      continue;
    }
    let data;
    try {
      data = JSON.parse(raw);
    } catch {
      skippedMalformed++;
      continue;
    }
    if (typeof data !== 'object' || data === null || Array.isArray(data)) {
      skippedMalformed++;
      continue;
    }
    if (typeof data.provider === 'string' && data.provider.length > 0) {
      alreadyHadProvider++;
      continue;
    }
    data.provider = 'unknown';
    fs.writeFileSync(fp, `${JSON.stringify(data, null, 2)}\n`, 'utf8');
    stamped++;
  }

  process.stdout.write(
    `backfill-provider: scanned=${scanned} stamped=${stamped} ` +
    `had-provider=${alreadyHadProvider} sidecars-skipped=${skippedSidecar} ` +
    `malformed-skipped=${skippedMalformed}\n`,
  );
}

if (require.main === module) {
  main(process.argv.slice(2));
}

module.exports = { _walkJsonFiles: walkJsonFiles, _parseArgs: parseArgs };
