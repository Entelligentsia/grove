#!/usr/bin/env node
'use strict';

/**
 * build-context-pack.cjs — build a compact architecture context pack from
 * engineering/architecture/*.md files. Writes:
 *   .forge/cache/context-pack.md   (human-readable summary)
 *   .forge/cache/context-pack.json (machine-readable index)
 *
 * CLI:
 *   node build-context-pack.cjs [--arch-dir <path>] [--out-md <path>] [--out-json <path>]
 *
 * Exported API:
 *   extractDoc(filePath)                      → { title, firstPara, sections, lineCount, filePath }
 *   buildContextPack({ archDir, existingPackPath? })  → pack object (or { skipped: true })
 *   computeSourceHash(archDir)                → "sha256:..."
 *   writeContextPack(pack, outMd, outJson)    → void (atomic)
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ensureDir } = require('./lib/fsutil.cjs');

const PACK_LINE_LIMIT = 400;

// ── Document extraction ──────────────────────────────────────────────────────

/**
 * Extract H1 title, first paragraph, and ## Key / ## Summary sections from
 * a single markdown file.
 */
function extractDoc(filePath) {
  const raw = fs.readFileSync(filePath, 'utf8');
  const lines = raw.split('\n');
  const lineCount = lines[lines.length - 1] === '' ? lines.length - 1 : lines.length;

  let title = '';
  let firstPara = '';
  const sections = {};

  let i = 0;

  // Find H1
  while (i < lines.length) {
    const m = lines[i].match(/^#\s+(.+)$/);
    if (m) {
      title = m[1].trim();
      i++;
      break;
    }
    i++;
  }

  // First paragraph: non-empty lines after H1, before next heading
  const firstParaLines = [];
  while (i < lines.length && !lines[i].startsWith('#')) {
    if (lines[i].trim()) firstParaLines.push(lines[i]);
    else if (firstParaLines.length) break; // blank line ends paragraph
    i++;
  }
  firstPara = firstParaLines.join(' ').trim();

  // Remaining: scan for ## Key * or ## Summary sections
  while (i < lines.length) {
    const hm = lines[i].match(/^##\s+(Key\s+\S.*|Summary)\s*$/i);
    if (hm) {
      const heading = hm[1].trim();
      i++;
      const sectionLines = [];
      while (i < lines.length && !lines[i].match(/^##/)) {
        sectionLines.push(lines[i]);
        i++;
      }
      // Trim leading/trailing blank lines
      while (sectionLines.length && !sectionLines[0].trim()) sectionLines.shift();
      while (sectionLines.length && !sectionLines[sectionLines.length - 1].trim()) sectionLines.pop();
      if (sectionLines.length) {
        sections[heading] = sectionLines.join('\n');
      }
    } else {
      i++;
    }
  }

  return { title, firstPara, sections, lineCount, filePath };
}

// ── Source hash ──────────────────────────────────────────────────────────────

function listArchFiles(archDir) {
  if (!fs.existsSync(archDir)) {
    throw new Error(`Architecture directory not found: ${archDir}`);
  }
  return fs.readdirSync(archDir)
    .filter((f) => f.endsWith('.md') && !f.endsWith('.draft.md'))
    .sort()
    .map((f) => path.join(archDir, f));
}

function computeSourceHash(archDir) {
  const files = listArchFiles(archDir);
  const hash = crypto.createHash('sha256');
  // FR-012: Content-based hashing for reproducibility.
  // Old mtime-based hash was non-deterministic across runs after git checkout.
  // New pattern: filePath\0 + fileContents + \0 — null-byte separators prevent
  // concatenation ambiguity and make the hash a pure function of content.
  for (const f of files) {
    hash.update(`${f}\0`);
    hash.update(fs.readFileSync(f));
    hash.update('\0');
  }
  return `sha256:${hash.digest('hex')}`;
}

// ── Pack composition ─────────────────────────────────────────────────────────

function composePack(docs, builtAt, sourceHash, archDir) {
  const lines = [];

  lines.push('# Architecture Context Pack', '');
  lines.push(`Built: ${builtAt}`);
  lines.push(`Source hash: ${sourceHash}`);
  lines.push('');

  // Aggregate Key/Summary sections from all docs, grouped by heading name
  const allHeadings = [];
  const headingDocs = {}; // heading → [{basename, content}]
  for (const doc of docs) {
    for (const [heading, content] of Object.entries(doc.sections)) {
      if (!headingDocs[heading]) {
        allHeadings.push(heading);
        headingDocs[heading] = [];
      }
      headingDocs[heading].push({ basename: path.basename(doc.filePath), content });
    }
  }

  if (allHeadings.length) {
    for (const heading of allHeadings) {
      lines.push(`## ${heading}`, '');
      for (const { basename, content } of headingDocs[heading]) {
        if (headingDocs[heading].length > 1) {
          lines.push(`*From ${basename}:*`, '');
        }
        lines.push(content, '');
      }
    }
  }

  // Document summaries
  lines.push('## Document summaries', '');
  for (const doc of docs) {
    const basename = path.basename(doc.filePath);
    lines.push(`### ${doc.title || basename}`);
    if (doc.firstPara) {
      lines.push('');
      lines.push(doc.firstPara);
    }
    lines.push('');
  }

  // File index
  lines.push('## File index', '');
  for (const doc of docs) {
    const relPath = path.relative(process.cwd(), doc.filePath);
    lines.push(`- ${relPath} — ${doc.title || path.basename(doc.filePath)} (${doc.lineCount} lines)`);
  }

  return lines;
}

// ── Build ────────────────────────────────────────────────────────────────────

function buildContextPack({ archDir, existingPackPath }) {
  // Check for manual override
  if (existingPackPath && fs.existsSync(existingPackPath)) {
    const existing = fs.readFileSync(existingPackPath, 'utf8');
    if (/^---[\s\S]*?manual:\s*true[\s\S]*?---/m.test(existing)) {
      return { skipped: true };
    }
  }

  const files = listArchFiles(archDir);
  const docs = files.map((f) => extractDoc(f));

  const builtAt = new Date().toISOString();
  const sourceHash = computeSourceHash(archDir);

  const mdLines = composePack(docs, builtAt, sourceHash, archDir);
  let markdown = mdLines.join('\n') + '\n';

  // Enforce 400-line cap (count real lines, excluding trailing empty after final \n)
  const splitLines = markdown.split('\n');
  const realLineCount = splitLines[splitLines.length - 1] === '' ? splitLines.length - 1 : splitLines.length;
  if (realLineCount > PACK_LINE_LIMIT) {
    // Keep PACK_LINE_LIMIT - 1 lines so the marker fits within the cap
    const truncated = splitLines.slice(0, PACK_LINE_LIMIT - 1);
    truncated.push('<!-- TRUNCATED: pack exceeded 400 lines. Architecture KB has grown beyond summary capacity. Run /forge:rebuild after pruning docs. -->');
    markdown = truncated.join('\n') + '\n';
  }

  const sources = files.map((f) => {
    const stat = fs.statSync(f);
    return {
      path: path.relative(process.cwd(), f),
      size: stat.size,
      mtime: new Date(stat.mtimeMs).toISOString(),
    };
  });

  return {
    version: 1,
    built_at: builtAt,
    source_hash: sourceHash,
    sources,
    markdown,
  };
}

// ── Atomic write ─────────────────────────────────────────────────────────────

function writeContextPack(pack, outMd, outJson) {
  ensureDir(path.dirname(outMd));
  ensureDir(path.dirname(outJson));

  // Write markdown
  const tmpMd = outMd + '.tmp';
  fs.writeFileSync(tmpMd, pack.markdown, 'utf8');
  fs.renameSync(tmpMd, outMd);

  // Write JSON index
  const jsonRecord = {
    version: pack.version,
    built_at: pack.built_at,
    source_hash: pack.source_hash,
    sources: pack.sources,
    summary_path: outMd,
  };
  const tmpJson = outJson + '.tmp';
  fs.writeFileSync(tmpJson, JSON.stringify(jsonRecord, null, 2) + '\n', 'utf8');
  fs.renameSync(tmpJson, outJson);
}

// ── CLI ──────────────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--arch-dir') out.archDir = argv[++i];
    else if (a === '--out-md') out.outMd = argv[++i];
    else if (a === '--out-json') out.outJson = argv[++i];
  }
  return out;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const archDir = args.archDir || path.resolve(process.cwd(), 'engineering/architecture');
  const outMd = args.outMd || path.resolve(process.cwd(), '.forge/cache/context-pack.md');
  const outJson = args.outJson || path.resolve(process.cwd(), '.forge/cache/context-pack.json');

  const pack = buildContextPack({ archDir, existingPackPath: outMd });

  if (pack.skipped) {
    process.stdout.write('context-pack: manual: true — skipping rebuild\n');
    return;
  }

  writeContextPack(pack, outMd, outJson);
  process.stdout.write(
    `context-pack: wrote ${pack.sources.length} sources → ${outMd}\n`,
  );
}

if (require.main === module) {
  try {
    main();
  } catch (err) {
    process.stderr.write(`build-context-pack: ${err.message}\n`);
    process.exit(1);
  }
}

module.exports = {
  extractDoc,
  buildContextPack,
  computeSourceHash,
  writeContextPack,
};
