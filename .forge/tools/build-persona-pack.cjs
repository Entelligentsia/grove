#!/usr/bin/env node
'use strict';

/**
 * build-persona-pack.cjs — compile persona/skill YAML frontmatter from
 * forge/meta/personas/* and forge/meta/skills/* into a compact JSON pack
 * at .forge/cache/persona-pack.json. The pack is used by meta-orchestrate
 * and meta-fix-bug to inject persona references (not verbatim prose) into
 * subagent prompts.
 *
 * CLI:
 *   node build-persona-pack.cjs [--meta-root <path>] [--out <path>]
 *
 * Exported API:
 *   parseFrontmatter(content, filePath)    → object (throws on missing/malformed)
 *   buildPack({ personaDir, skillDir })    → pack object
 *   computeSourceHash({ personaDir, skillDir }) → "sha256:..."
 *   writePack(pack, outPath)               → atomic write
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

// ── Persona parser — two variants: base-pack (no frontmatter) and meta (YAML frontmatter) ──
//
// Base-pack personas: first line is a markdown heading, e.g. "# Architect" or
// "# Architect — subtitle". The rest is the body. id and role are derived from
// the filename. summary is the first sentence of the first non-heading line.
// responsibilities and outputs are derived from section headers in the body.
// file_ref is derived from the file path.
//
// Meta personas: YAML frontmatter block (--- ... ---) containing all required
// fields. The body is ignored.

function parsePersona(content, filePath) {
  if (content.startsWith('---')) {
    // Meta format — parse frontmatter then ignore body
    const fm = parseFrontmatter(content, filePath);
    // Re-validate required fields for meta personas
    for (const field of REQUIRED_PERSONA_FIELDS) {
      if (!(field in fm)) {
        throw new Error(`${filePath}: frontmatter missing required field '${field}'`);
      }
    }
    return fm;
  }

  // Base-pack format — no frontmatter, derive fields from content
  const lines = content.split(/\r?\n/);
  const fileName = path.basename(filePath, '.md'); // e.g. "architect"

  // Derive id and role from filename (e.g. "meta-architect.md" → "architect", "architect.md" → "architect")
  const id = fileName.replace(/^meta-/, '').replace(/-skills$/, '');
  const role = id;

  // Extract summary: first non-heading, non-empty line
  let summary = '';
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed && !trimmed.startsWith('#') && !trimmed.startsWith('{{')) {
      // Grab first sentence (up to first period + space or first line break)
      const sentenceEnd = trimmed.search(/\.\s/);
      summary = sentenceEnd >= 0 ? trimmed.slice(0, sentenceEnd + 1) : trimmed;
      break;
    }
  }

  // Derive responsibilities and outputs from section headers
  const responsibilities = [];
  const outputs = [];
  for (const line of lines) {
    if (/^##\s+(What I Produce|Outputs?)/i.test(line)) {
      // Collect bullet points under this header until next ## or end
      let i = lines.indexOf(line) + 1;
      while (i < lines.length && !lines[i].startsWith('##')) {
        const m = lines[i].match(/^[-*]\s+(.+)/);
        if (m) outputs.push(m[1].trim());
        i++;
      }
    }
    if (/^##\s+(Capabilities?)/i.test(line)) {
      let i = lines.indexOf(line) + 1;
      while (i < lines.length && !lines[i].startsWith('##')) {
        const m = lines[i].match(/^[-*]\s+(.+)/);
        if (m) responsibilities.push(m[1].trim());
        i++;
      }
    }
  }

  // file_ref: base-pack personas become .forge/personas/<id>.md
  const file_ref = `.forge/personas/${id}.md`;

  return { id, role, summary, responsibilities, outputs, file_ref };
}

// ── Skill parser — base-pack skills (with frontmatter, no file_ref) and meta skills (with file_ref) ──

function parseSkill(content, filePath) {
  const fm = parseFrontmatter(content, filePath);

  // Skills must have: id, applies_to, summary, capabilities
  for (const field of ['id', 'applies_to', 'summary', 'capabilities']) {
    if (!(field in fm)) {
      throw new Error(`${filePath}: frontmatter missing required field '${field}'`);
    }
  }

  // file_ref is present in meta skills; derive it for base-pack skills from the path
  if (!fm.file_ref) {
    fm.file_ref = `.forge/skills/${path.basename(filePath)}`;
  }

  return fm;
}

// ── YAML frontmatter parser ──────────────────────────────────────────────────
// Narrow-scope parser: handles scalars, folded scalars (`>`), block lists
// (`- item`) under a key, and inline flow lists (`[a, b]`). Anything else
// throws a descriptive error with the source file path.

function parseFrontmatter(content, filePath) {
  const lines = content.split(/\r?\n/);
  if (lines[0] !== '---') {
    throw new Error(`${filePath}: no frontmatter block found (missing opening '---')`);
  }
  let end = -1;
  for (let i = 1; i < lines.length; i++) {
    if (lines[i] === '---') { end = i; break; }
  }
  if (end === -1) {
    throw new Error(`${filePath}: frontmatter block is unterminated (missing closing '---')`);
  }
  const body = lines.slice(1, end);
  return parseBlock(body, filePath);
}

function parseBlock(lines, filePath) {
  const out = {};
  let i = 0;
  while (i < lines.length) {
    const raw = lines[i];
    if (raw.trim() === '' || raw.trim().startsWith('#')) { i++; continue; }
    const m = raw.match(/^([A-Za-z_][A-Za-z0-9_-]*):\s*(.*)$/);
    if (!m) {
      throw new Error(`${filePath}: cannot parse frontmatter line ${i + 1}: ${JSON.stringify(raw)}`);
    }
    const key = m[1];
    const rest = m[2];

    // Folded scalar: `>` — consume indented continuation lines
    if (rest === '>' || rest === '>-' || rest === '>+') {
      const chunks = [];
      i++;
      while (i < lines.length && /^\s+\S/.test(lines[i])) {
        chunks.push(lines[i].trim());
        i++;
      }
      out[key] = chunks.join(' ').trim();
      continue;
    }

    // Inline flow list: `[a, b, c]`
    if (/^\[.*\]$/.test(rest)) {
      out[key] = rest
        .slice(1, -1)
        .split(',')
        .map((s) => stripQuotes(s.trim()))
        .filter((s) => s.length > 0);
      i++;
      continue;
    }

    // Block list: key with no inline value, followed by `- item` lines
    if (rest === '') {
      const items = [];
      i++;
      while (i < lines.length && /^\s*-\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*-\s+/, '').trim());
        i++;
      }
      if (items.length === 0) {
        // empty map value (rare) — leave as empty string
        out[key] = '';
      } else {
        out[key] = items;
      }
      continue;
    }

    // Plain scalar
    out[key] = stripQuotes(rest.trim());
    i++;
  }
  return out;
}

function stripQuotes(s) {
  if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) {
    return s.slice(1, -1);
  }
  return s;
}

// ── Pack building ────────────────────────────────────────────────────────────

const REQUIRED_PERSONA_FIELDS = ['id', 'role', 'summary', 'responsibilities', 'outputs', 'file_ref'];
const REQUIRED_SKILL_FIELDS = ['id', 'applies_to', 'summary', 'capabilities', 'file_ref'];

function listMarkdown(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir)
    .filter((f) => f.endsWith('.md') && f !== 'README.md')
    .sort()
    .map((f) => path.join(dir, f));
}

function loadPersonas(dir) {
  const entries = {};
  for (const filePath of listMarkdown(dir)) {
    const content = fs.readFileSync(filePath, 'utf8');
    try {
      const persona = parsePersona(content, filePath);
      entries[persona.id] = persona;
    } catch (err) {
      throw err;
    }
  }
  return entries;
}

function loadSkills(dir) {
  const entries = {};
  for (const filePath of listMarkdown(dir)) {
    const content = fs.readFileSync(filePath, 'utf8');
    try {
      const skill = parseSkill(content, filePath);
      entries[skill.id] = skill;
    } catch (err) {
      throw err;
    }
  }
  return entries;
}

function buildPack({ personaDir, skillDir }) {
  const personas = loadPersonas(personaDir);
  const skills = loadSkills(skillDir);

  const personaCount = Object.keys(personas).length;
  const skillCount = Object.keys(skills).length;
  if (personaCount === 0 && skillCount === 0) {
    throw new Error('no personas or skills found; refusing to write empty pack');
  }

  return {
    version: 1,
    built_at: new Date().toISOString(),
    source_hash: computeSourceHash({ personaDir, skillDir }),
    personas,
    skills,
  };
}

function computeSourceHash({ personaDir, skillDir }) {
  const files = [...listMarkdown(personaDir), ...listMarkdown(skillDir)].sort();
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

// ── Atomic write ─────────────────────────────────────────────────────────────

function writePack(pack, outPath) {
  const dir = path.dirname(outPath);
  fs.mkdirSync(dir, { recursive: true });
  const tmp = outPath + '.tmp';
  fs.writeFileSync(tmp, JSON.stringify(pack, null, 2) + '\n', 'utf8');
  fs.renameSync(tmp, outPath);
}

// ── CLI ──────────────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--meta-root') out.metaRoot = argv[++i];
    else if (a === '--out') out.out = argv[++i];
    else if (a === '--persona-dir') out.personaDir = argv[++i];
    else if (a === '--skill-dir') out.skillDir = argv[++i];
  }
  return out;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const metaRoot = args.metaRoot || path.resolve(__dirname, '..', 'meta');
  const personaDir = args.personaDir || path.join(metaRoot, 'personas');
  const skillDir = args.skillDir || path.join(metaRoot, 'skills');
  const out = args.out || path.resolve(process.cwd(), '.forge/cache/persona-pack.json');

  const pack = buildPack({ personaDir, skillDir });
  writePack(pack, out);
  process.stdout.write(
    `persona-pack: wrote ${Object.keys(pack.personas).length} personas, ${Object.keys(pack.skills).length} skills → ${out}\n`,
  );
}

if (require.main === module) {
  try {
    main();
  } catch (err) {
    process.stderr.write(`build-persona-pack: ${err.message}\n`);
    process.exit(1);
  }
}

module.exports = {
  parseFrontmatter,
  parsePersona,
  parseSkill,
  buildPack,
  computeSourceHash,
  writePack,
};
