'use strict';

// Forge Banner Library
// Reusable agent identity display for tools, hooks, and skills.
//
// Render modes:
//   render(name)             — full ASCII art block + emoji + name + tagline
//   badge(name)              — single line: emoji + bold name + dim tagline
//   mark(name)               — emoji only (for use alongside 〇△× status marks)
//   progressBar(n,total,opt) — unicode block bar with optional gradient + label
//   subtitle(text, opts)     — dim italic line for under-banner taglines
//   phaseHeader(...)         — convenience: badge + em-dash banner + progress bar
//
// Plain mode: ANSI escapes are stripped when any of these is true:
//   - env FORGE_BANNERS_PLAIN is set to a non-empty value
//   - env NO_COLOR is set to a non-empty value
//   - process.stdout.isTTY === false
//   - CLI flag --plain is passed
//
// Usage (module):
//   const banners = require('./banners.cjs');
//   console.log(banners.render('forge'));
//   console.log(banners.progressBar(5, 12, { label: 'Templates', color: [255,208,122] }));
//
// Usage (CLI):
//   node banners.cjs forge
//   node banners.cjs --badge north
//   node banners.cjs --mark tide
//   node banners.cjs --gallery
//   node banners.cjs --list
//   node banners.cjs --plain forge
//   node banners.cjs --subtitle "Forging your SDLC"
//   node banners.cjs --progress 5 12 "Templates"
//   node banners.cjs --phase 7 12 "Workflows" ember
//   node banners.cjs --badge forge --quiet    (automated/orchestrator use: zero stdout)

// ─── Plain-mode detection ─────────────────────────────────────────────────────
// Resolved at call-time so tests can flip env vars dynamically.
function isPlain() {
  if (process.env.FORGE_BANNERS_PLAIN) return true;
  if (process.env.NO_COLOR) return true;
  if (process.stdout && process.stdout.isTTY === false) return true;
  return false;
}

// Strip ANSI escape sequences (CSI). Conservative — matches \x1b[...m and friends.
const ANSI_RE = /\x1b\[[0-9;]*[A-Za-z]/g;
function stripAnsi(s) {
  return String(s).replace(ANSI_RE, '');
}

// Soft override — set by `--plain` CLI flag to force plain mode for one run.
let FORCE_PLAIN = false;

// Quiet mode — set by `--quiet` CLI flag to suppress all stdout output.
// Used by the orchestrator loop so banner output does not enter the LLM context
// window (output remains visible on a real TTY via the human's terminal but is
// not fed back as tool-call response text).
let QUIET_MODE = false;

// ─── ANSI helpers ─────────────────────────────────────────────────────────────
const R = '\x1b[0m';
const B = '\x1b[1m';
const D = '\x1b[2m';
const I = '\x1b[3m';
const f = (r, g, b) => `\x1b[38;2;${r};${g};${b}m`;

// ─── Mode tints ───────────────────────────────────────────────────────────────
// Mode-aware accent colour for progress bars and subtitles. Used by phaseHeader.
const MODE_TINTS = {
  full: [255, 138, 60],    // ember orange
};

// ─── Zen blue ─────────────────────────────────────────────────────────────────
// System-wide horizontal-rule tint. Applied to em-dash separators in
// phaseHeader and ruleLine. Picked to read calm and cool without competing
// with banner colours.
const ZEN_BLUE = [100, 140, 200];

// ─── Banner registry ───────────────────────────────────────────────────────────
const BANNERS = {

  ember: {
    emoji:   '🔥',
    tagline: 'heat · ignition · drive',
    name:    'EMBER',
    kanji:   '炎',
    color:   [255, 170, 60],
    art:     `  ${f(255,240,100)})  ${f(255,200,50)})  ${f(255,140,20)}(${f(230,80,10)}🔥${f(255,140,20)})  ${f(230,80,10)}~∿~  ${f(170,30,5)}≋≋≋`,
  },

  tide: {
    emoji:   '🌊',
    tagline: 'rhythm · pull · depth',
    name:    'TIDE',
    kanji:   '潮',
    color:   [110, 200, 255],
    art:     `  ${f(210,240,255)}∿  ${f(130,200,245)}≋≋≋  ${f(60,140,210)}≋≋≋  ${f(25,85,175)}▓▓▓  ${f(10,45,130)}▓▓▓`,
  },

  oracle: {
    emoji:   '🌕',
    tagline: 'sight · pattern · knowing',
    name:    'ORACLE',
    kanji:   '神託',
    color:   [210, 160, 255],
    art:     `  ${f(160,80,240)}· ◌  ${f(190,110,255)}◎  ${f(255,220,100)}◉  ${f(190,110,255)}◎  ${f(160,80,240)}◌ ·`,
  },

  rift: {
    emoji:   '⚡',
    tagline: 'edge · fracture · crossing',
    name:    'RIFT',
    kanji:   '裂',
    color:   [100, 255, 240],
    art:     `  ${f(0,240,230)}▓▒░  ${f(180,0,220)}╲ ${f(0,255,240)}⚡${f(180,0,220)} ╱  ${f(0,240,230)}░▒▓`,
  },

  bloom: {
    emoji:   '🌸',
    tagline: 'growth · opening · becoming',
    name:    'BLOOM',
    kanji:   '開花',
    color:   [255, 160, 190],
    art:     `  ${f(255,160,200)}✿ ✿  ${f(255,120,170)}✾  ${f(255,220,100)}✽  ${f(255,120,170)}✾  ${f(255,160,200)}✿ ✿`,
  },

  north: {
    emoji:   '🧭',
    tagline: 'direction · clarity · cold',
    name:    'NORTH',
    kanji:   '北',
    color:   [190, 225, 255],
    art:     `  ${f(200,230,255)}✦  ${f(150,195,240)}╱  ${f(100,160,230)}◈  ${f(150,195,240)}╲  ${f(200,230,255)}✦`,
  },

  lumen: {
    emoji:   '✨',
    tagline: 'light · warmth · clarity',
    name:    'LUMEN',
    kanji:   '光',
    color:   [255, 245, 150],
    art:     `  ${f(255,255,200)}✧  ${f(255,245,160)}──  ${f(255,255,255)}◉  ${f(255,245,160)}──  ${f(255,255,200)}✧`,
  },

  forge: {
    emoji:   '🔨',
    tagline: 'making · heat · craft',
    name:    'FORGE',
    kanji:   '鍛冶',
    color:   [255, 160, 40],
    art:     `  ${f(255,230,80)}✦  ${f(200,80,20)}▄${f(230,100,30)}▓▓▓▓▓${f(200,80,20)}▄  ${f(255,160,40)}≋ ≋ ≋`,
  },

  drift: {
    emoji:   '🍃',
    tagline: 'ease · letting go · flow',
    name:    'DRIFT',
    kanji:   '漂流',
    color:   [150, 200, 165],
    art:     `  ${f(160,200,170)}·  ${f(140,180,155)}·   ${f(120,165,140)}·  ${f(100,150,125)}·   ${f(80,135,110)}·`,
  },

  void: {
    emoji:   '🌑',
    tagline: 'depth · silence · potential',
    name:    'VOID',
    kanji:   '虚空',
    color:   [130, 100, 200],
    art:     `  ${f(30,20,60)}  ·  ${f(70,50,120)}◌  ${f(50,35,90)}·  `,
  },

  entelligentsia: {
    emoji:   '🔗',
    tagline: 'linked · intellect · becoming',
    name:    'ENTELLIGENTSIA',
    kanji:   '知性',
    color:   [140, 200, 60],
    art:     `  ${f(110,110,115)}╭──╯  ${f(140,200,60)}─╮ ╭─  ${f(110,110,115)}╰──╮`,
  },

};

// ─── Render helpers ────────────────────────────────────────────────────────────

/** Dim dot rule — use between banners in a gallery */
function rule() {
  return `  ${f(45,45,65)}${'·'.repeat(32)}${R}`;
}

/**
 * Full banner: ASCII art block + emoji + name + tagline.
 * @param {string} name  Banner key (case-insensitive).
 * @returns {string}
 */
function render(name) {
  const b = _get(name);
  const [r, g, bl] = b.color;
  const kanji = b.kanji ? `  ${D}${f(r,g,bl)}${b.kanji}${R}` : '';
  const label = `  ${b.emoji}  ${B}${f(r,g,bl)}${b.name}${R}${kanji}   ${D}${f(r,g,bl)}${b.tagline}${R}`;
  const out = '\n' + b.art + R + '\n' + label + '\n';
  return _maybePlain(out);
}

/**
 * Badge: single line — emoji + bold name + dim tagline.
 * Fits inline in status output or skill headers.
 * @param {string} name
 * @returns {string}
 */
function badge(name) {
  const b = _get(name);
  const [r, g, bl] = b.color;
  const kanji = b.kanji ? `  ${D}${f(r,g,bl)}${b.kanji}${R}` : '';
  return _maybePlain(`${b.emoji}  ${B}${f(r,g,bl)}${b.name}${R}${kanji}  ${D}${f(r,g,bl)}${b.tagline}${R}`);
}

/**
 * Mark: emoji only.
 * Use alongside 〇△× status marks for agent attribution.
 * @param {string} name
 * @returns {string}
 */
function mark(name) {
  return _get(name).emoji;
}

/**
 * Progress bar: unicode block bar with optional gradient tint and label.
 *   ▰▰▰▰▰▱▱▱▱▱▱▱  Phase 5/12 · Templates
 *
 * @param {number} n      Filled cells (clamped to [0, total])
 * @param {number} total  Total cells across the bar's logical range
 * @param {object} [opts]
 * @param {number} [opts.width=12]   Cells in the bar (independent of n/total)
 * @param {number[]} [opts.color]    [r,g,b] tint for filled segment
 * @param {string} [opts.label]      Trailing label after the bar
 * @param {string} [opts.fillGlyph='▰'] Override filled cell glyph
 * @param {string} [opts.emptyGlyph='▱'] Override empty cell glyph
 * @returns {string}
 */
function progressBar(n, total, opts) {
  const o = opts || {};
  const width = Math.max(1, o.width || 12);
  const fillGlyph = o.fillGlyph || '▰';
  const emptyGlyph = o.emptyGlyph || '▱';
  const safeTotal = Math.max(1, total || 1);
  const safeN = Math.max(0, Math.min(n || 0, safeTotal));
  const filledCells = Math.round((safeN / safeTotal) * width);
  const emptyCells = width - filledCells;

  let bar = '';
  if (filledCells > 0) {
    const tint = Array.isArray(o.color) ? f(o.color[0], o.color[1], o.color[2]) : '';
    const reset = tint ? R : '';
    bar += `${tint}${fillGlyph.repeat(filledCells)}${reset}`;
  }
  if (emptyCells > 0) {
    bar += `${D}${emptyGlyph.repeat(emptyCells)}${R}`;
  }

  const counter = `  ${safeN}/${safeTotal}`;
  const label = o.label ? `  ${D}·${R}  ${o.label}` : '';
  return _maybePlain(bar + counter + label);
}

/**
 * Subtitle: dim italic line under a banner. Single line.
 *
 * @param {string} text
 * @param {object} [opts]
 * @param {number[]} [opts.color]   Optional tint
 * @returns {string}
 */
function subtitle(text, opts) {
  const o = opts || {};
  const tint = Array.isArray(o.color) ? f(o.color[0], o.color[1], o.color[2]) : '';
  return _maybePlain(`  ${D}${I}${tint}${text}${R}`);
}

/**
 * Em-dash rule line, tinted zen blue by default.
 *   ━━━ {text} ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 *
 * Use as a horizontal section separator anywhere in the system. Calling
 * with no text produces a plain rule (just em-dashes, no label).
 *
 * @param {string} [text]   Optional label to embed in the rule
 * @param {object} [opts]
 * @param {number} [opts.width=65]   Total line width
 * @param {number[]} [opts.color]    [r,g,b] override tint; defaults to ZEN_BLUE
 * @returns {string}
 */
function ruleLine(text, opts) {
  const o = opts || {};
  const width = o.width || 65;
  const color = Array.isArray(o.color) ? o.color : ZEN_BLUE;
  const tint = f(color[0], color[1], color[2]);

  let line;
  if (text && text.length > 0) {
    const head = `━━━ ${text} `;
    const padCount = Math.max(3, width - head.length);
    line = head + '━'.repeat(padCount);
  } else {
    line = '━'.repeat(width);
  }
  return _maybePlain(`${tint}${line}${R}`);
}

/**
 * Phase header: multi-line — badge, em-dash banner, progress bar.
 * Concatenates with newlines so a single println prints all three.
 *
 * @param {number} n           Phase number (1-based)
 * @param {number} total       Total phases
 * @param {string} name        Phase display name
 * @param {string} bannerKey   Banner registry key (e.g. 'north', 'forge')
 * @param {object|string} [opts]
 *        Pass a string to use as the mode tint key ('fast' | 'full'),
 *        or an opts object with { mode, color, width }.
 * @returns {string}
 */
function phaseHeader(n, total, name, bannerKey, opts) {
  let o = {};
  if (typeof opts === 'string') {
    o = { mode: opts };
  } else if (opts) {
    o = opts;
  }
  const tint = o.color || (o.mode && MODE_TINTS[o.mode.toLowerCase()]) || null;

  const bar = progressBar(n, total, {
    width: o.width || 12,
    color: tint,
    label: name,
  });

  // Em-dash banner — match the existing format used across all forge commands,
  // tinted zen blue for system-wide visual consistency on horizontal rules.
  const emBanner = ruleLine(`Phase ${n}/${total} — ${name}`);

  const lines = [
    badge(bannerKey),
    emBanner,
    bar,
  ];
  return lines.join('\n');
}

/**
 * All available banner names.
 * @returns {string[]}
 */
function list() {
  return Object.keys(BANNERS);
}

/**
 * Full gallery of all banners separated by rules.
 * @returns {string}
 */
function gallery() {
  return list().map((name, i) => {
    const sep = i > 0 ? '\n' + rule() + '\n' : '';
    return sep + render(name);
  }).join('');
}

// ─── Internal ──────────────────────────────────────────────────────────────────

function _get(name) {
  const b = BANNERS[name.toLowerCase()];
  if (!b) {
    throw new Error(`Unknown banner "${name}". Available: ${list().join(', ')}`);
  }
  return b;
}

function _maybePlain(s) {
  return (FORCE_PLAIN || isPlain()) ? stripAnsi(s) : s;
}

// ─── CLI ───────────────────────────────────────────────────────────────────────
// node banners.cjs [<name>] [--gallery] [--badge <name>] [--mark <name>] [--list]
// node banners.cjs --badge <name> --quiet   (suppress all stdout; orchestrator use)

if (require.main === module) {
  let args = process.argv.slice(2);

  // --plain may appear anywhere; consume it first.
  const plainIdx = args.indexOf('--plain');
  if (plainIdx !== -1) {
    FORCE_PLAIN = true;
    args = args.slice(0, plainIdx).concat(args.slice(plainIdx + 1));
  }

  // --quiet may appear anywhere; consume it before dispatch.
  // When set, all stdout is suppressed (exit 0 still guaranteed on success).
  const quietIdx = args.indexOf('--quiet');
  if (quietIdx !== -1) {
    QUIET_MODE = true;
    args = args.slice(0, quietIdx).concat(args.slice(quietIdx + 1));
  }

  // Helper: write to stdout only when not in quiet mode.
  const emit = (s) => { if (!QUIET_MODE) process.stdout.write(s); };

  try {
    if (!args.length || args[0] === '--gallery') {
      emit(gallery() + '\n');
    } else if (args[0] === '--list') {
      emit(list().map(n => {
        const b = BANNERS[n];
        return `${b.emoji}  ${n.padEnd(8)} — ${b.tagline}`;
      }).join('\n') + '\n');
    } else if (args[0] === '--badge') {
      emit(badge(args[1] || '') + '\n');
    } else if (args[0] === '--mark') {
      emit(mark(args[1] || '') + '\n');
    } else if (args[0] === '--subtitle') {
      emit(subtitle(args.slice(1).join(' ')) + '\n');
    } else if (args[0] === '--progress') {
      const n = Number(args[1]);
      const total = Number(args[2]);
      const label = args.slice(3).join(' ') || undefined;
      emit(progressBar(n, total, { label }) + '\n');
    } else if (args[0] === '--phase') {
      const n = Number(args[1]);
      const total = Number(args[2]);
      const name = args[3] || '';
      const bannerKey = args[4] || 'forge';
      const mode = args[5];   // optional 'fast' | 'full'
      emit(phaseHeader(n, total, name, bannerKey, mode ? { mode } : undefined) + '\n');
    } else if (args[0] === '--rule') {
      // --rule [text]   Zen-blue em-dash horizontal rule (with optional label)
      const text = args.slice(1).join(' ') || undefined;
      emit(ruleLine(text) + '\n');
    } else {
      emit(render(args[0]) + '\n');
    }
  } catch (e) {
    console.error(e.message);
    process.exit(1);
  }
}

// ─── Exports ───────────────────────────────────────────────────────────────────
module.exports = {
  render, badge, mark, list, gallery, rule, BANNERS,
  progressBar, subtitle, phaseHeader, ruleLine,
  isPlain, stripAnsi, MODE_TINTS, ZEN_BLUE,
};
