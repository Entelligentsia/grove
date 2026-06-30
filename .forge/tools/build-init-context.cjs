'use strict';
// build-init-context.cjs
// Deterministic project brief builder for /forge:init Phase 7 fan-out.
// Reads: .forge/config.json, .forge/personas/, .forge/templates/, {kb}/
// Writes: .forge/init-context.md  (markdown — for LLM prompts)
//         .forge/init-context.json (same data — for deterministic consumers)
//
// Uses only Node.js built-ins — no npm dependencies.
// Exports: buildBrief, extractPersonaSymbol, parseEntities (for testing)

'use strict';
const fs   = require('fs');
const path = require('path');

// ── Skill → persona wiring (mirrors meta-skill-recommendations.md) ─────────
// Maps installed skill name to the persona roles it wires into.
const SKILL_PERSONA_MAP = {
  'vue-best-practices':   ['engineer', 'supervisor'],
  'stripe-integration':   ['engineer', 'bug-fixer'],
  'frontend-design':      ['engineer', 'supervisor'],
  'typescript-lsp':       ['engineer', 'supervisor'],
  'pyright-lsp':          ['engineer', 'supervisor'],
  'ruby-lsp':             ['engineer', 'supervisor'],
  'gopls-lsp':            ['engineer', 'supervisor'],
  'rust-analyzer-lsp':    ['engineer', 'supervisor'],
  'jdtls-lsp':            ['engineer', 'supervisor'],
  'kotlin-lsp':           ['engineer', 'supervisor'],
  'csharp-lsp':           ['engineer', 'supervisor'],
  'clangd-lsp':           ['engineer', 'supervisor'],
  'php-lsp':              ['engineer', 'supervisor'],
  'swift-lsp':            ['engineer', 'supervisor'],
  'lua-lsp':              ['engineer', 'supervisor'],
  'security-guidance':    ['supervisor', 'engineer'],
  'mcp-server-dev':       ['engineer'],
  'agent-sdk-dev':        ['engineer'],
  'threejs-skills':       ['engineer'],
  'meta-webxr-skills':    ['engineer'],
  'freshdesk-api':        ['engineer'],
};

// ── extractPersonaSymbol ──────────────────────────────────────────────────────
// Reads the first 15 lines of a persona file and extracts the emoji symbol.
// Handles two formats:
//   1. First-line emoji (generated persona style):
//        🗻 **emberglow Architect** — tagline
//   2. YAML frontmatter "symbol:" line:
//        ---
//        symbol: 🏛
//        ---
// Returns '·' if no symbol is found.
function extractPersonaSymbol(content) {
  const lines = content.split('\n').slice(0, 15);

  // Format 1: first non-blank line starts with an emoji character
  // Unicode emoji range check — covers most common emoji blocks
  const firstNonBlank = lines.find(l => l.trim().length > 0);
  if (firstNonBlank) {
    // Match a leading emoji (one or more emoji codepoints before a space or end)
    const emojiMatch = firstNonBlank.match(/^(\p{Emoji_Presentation}|\p{Extended_Pictographic})/u);
    if (emojiMatch) return emojiMatch[0];
  }

  // Format 2: "symbol: <value>" in YAML frontmatter
  for (const line of lines) {
    const m = line.match(/^symbol:\s*(\S.*?)\s*$/i);
    if (m) return m[1].trim();
  }

  return '·';
}

// ── parseEntities ─────────────────────────────────────────────────────────────
// Extracts domain entity names from MASTER_INDEX.md content.
// Looks for a heading containing "Entities" or "Domain"; reads the first
// non-empty content line(s) below it as either comma-separated names or
// markdown list items. Returns a sorted, deduplicated array.
function parseEntities(content) {
  const lines = content.split('\n');
  let inSection = false;
  const found = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (/^#{1,4}\s.*(entities|domain)/i.test(line)) {
      inSection = true;
      continue;
    }

    if (inSection) {
      // Stop at the next heading
      if (/^#{1,4}\s/.test(line)) break;

      const trimmed = line.trim();
      if (!trimmed) continue;

      // List items: "- Entity" or "* Entity"
      if (/^[-*]\s+/.test(trimmed)) {
        const name = trimmed.replace(/^[-*]\s+/, '').trim();
        if (name) found.push(name);
        continue;
      }

      // Comma-separated line
      if (trimmed.includes(',')) {
        for (const part of trimmed.split(',')) {
          const name = part.trim();
          if (name) found.push(name);
        }
        break; // Only read one comma-separated line
      }

      // Single name on its own line
      if (trimmed && !trimmed.startsWith('#')) {
        found.push(trimmed);
      }
    }
  }

  // Deduplicate and sort
  return [...new Set(found)].sort();
}

// ── buildBrief ────────────────────────────────────────────────────────────────
// Main builder. Takes options object:
//   { configPath, personasDir, templatesDir, kbPath }
// Returns: { markdown: string, json: object }
function buildBrief({ configPath, personasDir, templatesDir, kbPath }) {
  // 1. Read config
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  const projectName   = (config.project && config.project.name)   || '';
  const projectPrefix = (config.project && config.project.prefix) || '';
  const commands      = config.commands || {};
  const paths         = config.paths    || {};
  const installedSkills = Array.isArray(config.installedSkills) ? config.installedSkills : [];

  const syntaxCheck = commands.syntaxCheck || '';
  const testCmd     = commands.test        || '';
  const buildCmd    = commands.build       || '';
  const lintCmd     = commands.lint        || '';

  // 2. Personas — exclude README.md, sort by role name
  const personas = [];
  if (fs.existsSync(personasDir)) {
    const files = fs.readdirSync(personasDir)
      .filter(f => f.endsWith('.md') && f.toLowerCase() !== 'readme.md')
      .sort();

    for (const file of files) {
      const role    = path.basename(file, '.md');
      const filePath = path.join(personasDir, file);
      const content  = fs.readFileSync(filePath, 'utf8');
      const symbol   = extractPersonaSymbol(content);
      // One-liner: first non-blank, non-YAML, non-heading line after the opening block
      const bodyLines = content.split('\n').filter(l => {
        const t = l.trim();
        return t && !t.startsWith('#') && !t.startsWith('---') && !t.match(/^\w+:/);
      });
      const oneLiner = bodyLines[0] ? bodyLines[0].trim() : '';
      personas.push({ role, file: path.join(personasDir, file), symbol, oneLiner });
    }
  }

  // 3. Templates — stems only, exclude README.md, sort
  const templates = [];
  if (fs.existsSync(templatesDir)) {
    const files = fs.readdirSync(templatesDir)
      .filter(f => f.endsWith('.md') && f.toLowerCase() !== 'readme.md')
      .sort();
    for (const f of files) {
      templates.push(path.basename(f, '.md'));
    }
  }

  // 4. Architecture docs — filenames only, sorted
  const archDir = path.join(kbPath, 'architecture');
  const architectureDocs = [];
  if (fs.existsSync(archDir)) {
    const files = fs.readdirSync(archDir)
      .filter(f => f.endsWith('.md'))
      .sort();
    architectureDocs.push(...files);
  }

  // 5. Domain entities — from MASTER_INDEX.md
  const masterIndexPath = path.join(kbPath, 'MASTER_INDEX.md');
  let entities = [];
  if (fs.existsSync(masterIndexPath)) {
    entities = parseEntities(fs.readFileSync(masterIndexPath, 'utf8'));
  }

  // 6. Skill wiring
  const skillWiring = installedSkills
    .filter(s => SKILL_PERSONA_MAP[s])
    .map(s => ({ skill: s, personas: SKILL_PERSONA_MAP[s] }))
    .sort((a, b) => a.skill.localeCompare(b.skill));

  // ── Build JSON ──────────────────────────────────────────────────────────────
  const json = {
    project:          { name: projectName, prefix: projectPrefix },
    commands:         { syntaxCheck, test: testCmd, build: buildCmd, lint: lintCmd },
    paths,
    personas,
    templates,
    architectureDocs,
    entities,
    skillWiring,
  };

  // ── Build Markdown ──────────────────────────────────────────────────────────
  const lines = [];

  lines.push(`# ${projectName || 'Project'} — Init Context`);
  lines.push('');

  // Commands
  lines.push('## Commands');
  lines.push(`{SYNTAX_CHECK} = ${syntaxCheck}`);
  lines.push(`{TEST_COMMAND}  = ${testCmd}`);
  lines.push(`{BUILD_COMMAND} = ${buildCmd}`);
  lines.push(`{LINT_COMMAND}  = ${lintCmd}`);
  lines.push('');

  // Paths
  lines.push('## Paths');
  for (const [key, val] of Object.entries(paths).sort(([a], [b]) => a.localeCompare(b))) {
    lines.push(`${key.padEnd(12)} = ${val}`);
  }
  lines.push('');

  // Personas
  lines.push('## Personas');
  for (const p of personas) {
    lines.push(`${p.role} | ${p.file} | ${p.symbol} | ${p.oneLiner}`);
  }
  lines.push('');

  // Templates
  lines.push('## Templates');
  lines.push(templates.join(', '));
  lines.push('');

  // Architecture Docs
  lines.push('## Architecture Docs');
  lines.push(architectureDocs.join(', '));
  lines.push('');

  // Domain Entities
  lines.push('## Domain Entities');
  lines.push(entities.join(', '));
  lines.push('');

  // Installed Skill Wiring
  lines.push('## Installed Skill Wiring');
  if (skillWiring.length === 0) {
    lines.push('(none)');
  } else {
    for (const { skill, personas: ps } of skillWiring) {
      lines.push(`${skill} → ${ps.join(', ')}`);
    }
  }
  lines.push('');

  const markdown = lines.join('\n');

  return { markdown, json };
}

// ── CLI entry point ───────────────────────────────────────────────────────────

function parseCliArgs(argv) {
  const args = {};
  for (let i = 2; i < argv.length; i++) {
    if (argv[i].startsWith('--') && argv[i + 1] && !argv[i + 1].startsWith('--')) {
      args[argv[i].slice(2)] = argv[i + 1];
      i++;
    }
  }
  return args;
}

if (require.main === module) {
  const args = parseCliArgs(process.argv);

  const configPath   = args['config'];
  const personasDir  = args['personas'];
  const templatesDir = args['templates'];
  const kbPath       = args['kb'];
  const outMd        = args['out'];
  const outJson      = args['json-out'];

  if (!configPath || !personasDir || !templatesDir || !kbPath || !outMd) {
    process.stderr.write(
      'Usage: node build-init-context.cjs ' +
      '--config <path> --personas <dir> --templates <dir> --kb <dir> --out <md-path> ' +
      '[--json-out <json-path>]\n'
    );
    process.exit(1);
  }

  try {
    const { markdown, json } = buildBrief({ configPath, personasDir, templatesDir, kbPath });
    fs.mkdirSync(path.dirname(path.resolve(outMd)), { recursive: true });
    fs.writeFileSync(outMd, markdown, 'utf8');

    const jsonPath = outJson || outMd.replace(/\.md$/, '.json');
    fs.writeFileSync(jsonPath, JSON.stringify(json, null, 2) + '\n', 'utf8');

    // Print summary line to stdout (consumed by Phase 7 orchestrator)
    const nPersonas = json.personas.length;
    const nTemplates = json.templates.length;
    const nDocs = json.architectureDocs.length;
    process.stdout.write(
      `\u25CB Brief written \u2014 ${nPersonas} personas, ${nTemplates} templates, ${nDocs} architecture docs\n`
    );
  } catch (err) {
    process.stderr.write(`build-init-context: ${err.message}\n`);
    process.exit(1);
  }
}

module.exports = { buildBrief, extractPersonaSymbol, parseEntities };
