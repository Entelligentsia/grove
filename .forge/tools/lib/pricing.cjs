'use strict';

/**
 * pricing.cjs — Shared pricing primitive for Forge cost tooling.
 *
 * Exports:
 *   MODEL_PRICING   - frozen object mapping canonical model name → { input, output, cacheRead, cacheWrite }
 *                     All rates are in USD per token (not per 1M tokens).
 *   canonicalizeModel(model) - normalises model name variants/aliases to canonical form.
 *                              Returns { canonical, contextWindow } or null if unknown.
 *   computeCost(opts)        - computes USD cost from { inputTokens, outputTokens,
 *                              cacheReadTokens, cacheWriteTokens, model }.
 *                              Returns number (may be 0), or null for unknown model.
 *
 * This module is a pure library — no CLI, no process.exit.
 * Published Anthropic pricing (USD/MTok → divide by 1,000,000 for per-token rate).
 * Verified against Anthropic support + the pi vendor model table on 2026-06-08.
 * cacheWrite uses the 5-minute-TTL rate; Forge phases run inside the 5-min cache
 * window, so the 5-min write rate is the one that bills (1-hour TTL is higher but
 * not used). All Opus 4.x generations share one rate tier:
 *   claude-opus-4-5:   input $5.00,  output $25.00, cacheRead $0.50, cacheWrite $6.25
 *   claude-opus-4-6:   input $5.00,  output $25.00, cacheRead $0.50, cacheWrite $6.25
 *   claude-opus-4-7:   input $5.00,  output $25.00, cacheRead $0.50, cacheWrite $6.25
 *   claude-opus-4-8:   input $5.00,  output $25.00, cacheRead $0.50, cacheWrite $6.25
 *   claude-sonnet-4-5: input $3.00,  output $15.00, cacheRead $0.30, cacheWrite $3.75
 *   claude-sonnet-4-6: input $3.00,  output $15.00, cacheRead $0.30, cacheWrite $3.75
 *   claude-haiku-3-5:  input $0.80,  output $4.00,  cacheRead $0.08, cacheWrite $1.00
 */

// All rates in USD per token (divide MTok rate by 1,000,000)
const MODEL_PRICING = Object.freeze({
  'claude-opus-4-5': {
    input:       5.00  / 1_000_000,
    output:     25.00  / 1_000_000,
    cacheRead:   0.50  / 1_000_000,
    cacheWrite:  6.25  / 1_000_000,
  },
  'claude-opus-4-6': {
    input:       5.00  / 1_000_000,
    output:     25.00  / 1_000_000,
    cacheRead:   0.50  / 1_000_000,
    cacheWrite:  6.25  / 1_000_000,
  },
  'claude-opus-4-7': {
    input:       5.00  / 1_000_000,
    output:     25.00  / 1_000_000,
    cacheRead:   0.50  / 1_000_000,
    cacheWrite:  6.25  / 1_000_000,
  },
  'claude-opus-4-8': {
    input:       5.00  / 1_000_000,
    output:     25.00  / 1_000_000,
    cacheRead:   0.50  / 1_000_000,
    cacheWrite:  6.25  / 1_000_000,
  },
  'claude-sonnet-4-5': {
    input:       3.00  / 1_000_000,
    output:     15.00  / 1_000_000,
    cacheRead:   0.30  / 1_000_000,
    cacheWrite:  3.75  / 1_000_000,
  },
  'claude-sonnet-4-6': {
    input:       3.00  / 1_000_000,
    output:     15.00  / 1_000_000,
    cacheRead:   0.30  / 1_000_000,
    cacheWrite:  3.75  / 1_000_000,
  },
  'claude-haiku-3-5': {
    input:       0.80  / 1_000_000,
    output:      4.00  / 1_000_000,
    cacheRead:   0.08  / 1_000_000,
    cacheWrite:  1.00  / 1_000_000,
  },
});

/**
 * Known model families for canonical resolution.
 * Each entry is [baseCanonical, [...synonymPatterns]]
 * Patterns are tested after lower-casing the input model string.
 * The order matters: more specific patterns must come before more general ones.
 */
const MODEL_FAMILIES = [
  // Opus family
  'claude-opus-4-8',
  'claude-opus-4-7',
  'claude-opus-4-6',
  'claude-opus-4-5',
  // Sonnet family
  'claude-sonnet-4-6',
  'claude-sonnet-4-5',
  // Haiku family
  'claude-haiku-3-5',
];

/**
 * canonicalizeModel(model)
 *
 * Given a raw model string (possibly fragmented / aliased), returns:
 *   { canonical: string, contextWindow: string|null }
 * where:
 *   - canonical is one of the keys in MODEL_PRICING
 *   - contextWindow is '1m' when a 1M-context-window variant was detected, otherwise null
 *
 * Returns null if the model cannot be mapped to a known canonical name.
 *
 * Examples:
 *   'claude-sonnet-4-6'    → { canonical: 'claude-sonnet-4-6', contextWindow: null }
 *   'claude-sonnet-4-6-1m' → { canonical: 'claude-sonnet-4-6', contextWindow: '1m' }
 *   'claude-sonnet-4-6[1m]'→ { canonical: 'claude-sonnet-4-6', contextWindow: '1m' }
 *   'sonnet-4-6'           → { canonical: 'claude-sonnet-4-6', contextWindow: null }
 *   'opus-4-7-1m'          → { canonical: 'claude-opus-4-7',   contextWindow: '1m' }
 *   'totally-unknown-9-9'  → null
 */
function canonicalizeModel(model) {
  if (model == null || typeof model !== 'string' || model.trim() === '') return null;

  const raw = model.trim();

  // Detect and strip 1m context-window suffix variants:
  //   -1m suffix   (e.g. claude-sonnet-4-6-1m)
  //   [1m] suffix  (e.g. claude-sonnet-4-6[1m])
  let contextWindow = null;
  let stripped = raw;
  if (/\[1m\]$/i.test(stripped)) {
    contextWindow = '1m';
    stripped = stripped.replace(/\[1m\]$/i, '').replace(/-+$/, '');
  } else if (/-1m$/i.test(stripped)) {
    contextWindow = '1m';
    stripped = stripped.replace(/-1m$/i, '');
  }

  // Normalise to lowercase for matching
  const lower = stripped.toLowerCase();

  for (const canonical of MODEL_FAMILIES) {
    // Exact match (after stripping)
    if (lower === canonical) {
      return { canonical, contextWindow };
    }
    // Short alias: canonical without leading 'claude-'
    const short = canonical.replace(/^claude-/, '');
    if (lower === short) {
      return { canonical, contextWindow };
    }
    // Contains the canonical family segment (e.g. 'claude-sonnet-4-6' in a longer variant)
    if (lower.includes(canonical)) {
      return { canonical, contextWindow };
    }
  }

  return null;
}

/**
 * computeCost({ inputTokens, outputTokens, cacheReadTokens, cacheWriteTokens, model })
 *
 * Returns the estimated USD cost as a number, or null if the model is unknown.
 * Returns 0 (not null) when all token counts are zero and the model is known.
 * Never returns a negative number.
 *
 * @param {object} opts
 * @param {number} opts.inputTokens
 * @param {number} opts.outputTokens
 * @param {number} opts.cacheReadTokens
 * @param {number} opts.cacheWriteTokens
 * @param {string} opts.model  - raw model string (will be canonicalized)
 * @returns {number|null}
 */
function computeCost({ inputTokens, outputTokens, cacheReadTokens, cacheWriteTokens, model }) {
  const canonResult = canonicalizeModel(model);
  if (canonResult === null) return null;

  const rates = MODEL_PRICING[canonResult.canonical];
  if (!rates) return null;

  const cost =
    (inputTokens      || 0) * rates.input +
    (outputTokens     || 0) * rates.output +
    (cacheReadTokens  || 0) * rates.cacheRead +
    (cacheWriteTokens || 0) * rates.cacheWrite;

  return Math.max(0, cost);
}

module.exports = { MODEL_PRICING, canonicalizeModel, computeCost };
