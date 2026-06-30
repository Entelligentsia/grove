'use strict';

// Vocab-drift detector + "Did you mean?" suggestion engine (FORGE-S22-T03)
//
// Provides Levenshtein-based fuzzy matching with a curated DRIFT_MAP for
// high-frequency agent misconception patterns that pure distance alone cannot
// resolve (e.g., "task" → "taskId", "completed" → "committed").
//
// Suggestion pipeline (apply in order):
//   1. Store original input
//   2. Normalize (underscores → hyphens)
//   3. Suppress if original exactly matches a valid candidate (true noise)
//   4. Normalization-hint: if normalized matches a candidate but original doesn't,
//      suggest the normalized form (underscore→hyphen hint)
//   5. DRIFT_MAP exact match (after normalization), intersected with candidate pool
//   6. Levenshtein fallback (distance ≤ 2, max 3 suggestions)
//   7. Format as "(Did you mean ...?)" string

const MAX_CANDIDATES = 200;
const DEFAULT_MAX_DIST = 2;
const MAX_SUGGESTIONS = 3;

// ---------------------------------------------------------------------------
// Levenshtein distance
// ---------------------------------------------------------------------------

/**
 * Compute the Levenshtein distance between two strings.
 * @param {string} a
 * @param {string} b
 * @returns {number}
 */
function levenshtein(a, b) {
  if (a.length === 0) return b.length;
  if (b.length === 0) return a.length;

  // Swap so that the shorter string is b (reduces memory)
  if (a.length < b.length) {
    const tmp = a; a = b; b = tmp;
  }

  const prevRow = new Array(b.length + 1);
  for (let j = 0; j <= b.length; j++) prevRow[j] = j;

  for (let i = 1; i <= a.length; i++) {
    const curRow = new Array(b.length + 1);
    curRow[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      curRow[j] = Math.min(
        curRow[j - 1] + 1,       // insertion
        prevRow[j] + 1,           // deletion
        prevRow[j - 1] + cost    // substitution
      );
    }
    // Copy curRow into prevRow for next iteration
    for (let j = 0; j <= b.length; j++) prevRow[j] = curRow[j];
  }

  return prevRow[b.length];
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/**
 * Normalize a string for matching: replace underscores with hyphens.
 * @param {string} s
 * @returns {string}
 */
function normalizeForMatch(s) {
  return s.replace(/_/g, '-');
}

// ---------------------------------------------------------------------------
// DRIFT_MAP
// ---------------------------------------------------------------------------

/**
 * Curated drift map for high-frequency agent misconceptions.
 * Keys are *normalized* input strings; values are arrays of suggested targets.
 * Targets are intersected against the candidate pool at suggest-time.
 */
const DRIFT_MAP = {
  'task':           ['taskId'],
  'completed':      ['committed'],
  'in-progress':    ['implementing'],
  'timestamp':      ['startTimestamp', 'endTimestamp'],
  'task-completed': ['task-committed'],
  'implemented':    ['implementing'],
  'set':            ['set-summary'],
  'start':          ['startTimestamp'],
  'create':         ['write']
};

// ---------------------------------------------------------------------------
// suggest() — main suggestion pipeline
// ---------------------------------------------------------------------------

/**
 * Generate "Did you mean?" suggestions for an input string.
 *
 * @param {string} input        The misspelled/invalid input string
 * @param {string[]} candidates Valid candidate strings
 * @param {object} [opts]       Options
 * @param {number} [opts.maxDist=2]        Maximum Levenshtein distance
 * @param {number} [opts.maxCandidates=200] Safety guard — return [] if pool exceeds this
 * @returns {string[]} Array of suggested corrections (0–3 items)
 */
function suggest(input, candidates, opts) {
  if (!input || !Array.isArray(candidates) || candidates.length === 0) return [];

  const maxDist = (opts && opts.maxDist !== undefined) ? opts.maxDist : DEFAULT_MAX_DIST;
  const maxCandidates = (opts && opts.maxCandidates !== undefined) ? opts.maxCandidates : MAX_CANDIDATES;

  // Safety guard: don't process enormous candidate pools
  if (candidates.length > maxCandidates) return [];

  const original = String(input);
  const normalized = normalizeForMatch(original);

  // Step 3: Suppress if original exactly matches a valid candidate (true noise)
  if (candidates.includes(original)) return [];

  // Step 4: Normalization-hint — if normalized matches a candidate but original doesn't
  if (normalized !== original && candidates.includes(normalized)) {
    return [normalized];
  }

  // Step 5: DRIFT_MAP exact match (after normalization), intersected with candidate pool
  if (DRIFT_MAP[normalized]) {
    const filtered = DRIFT_MAP[normalized].filter(function(t) {
      return candidates.includes(t);
    });
    if (filtered.length > 0) return filtered.slice(0, MAX_SUGGESTIONS);
    // If no DRIFT_MAP targets are in the pool, fall through to Levenshtein
  }

  // Step 6: Levenshtein fallback
  const scored = [];
  for (let i = 0; i < candidates.length; i++) {
    const candidate = candidates[i];
    const d = levenshtein(normalized, candidate);
    if (d <= maxDist && d > 0) {
      scored.push({ candidate: candidate, distance: d });
    }
  }

  // Sort by distance ascending, then alphabetically
  scored.sort(function(a, b) {
    if (a.distance !== b.distance) return a.distance - b.distance;
    return a.candidate < b.candidate ? -1 : a.candidate > b.candidate ? 1 : 0;
  });

  const results = [];
  for (let i = 0; i < Math.min(MAX_SUGGESTIONS, scored.length); i++) {
    results.push(scored[i].candidate);
  }
  return results;
}

// ---------------------------------------------------------------------------
// suggestEntityType() — shared helper for entity-type error sites
// ---------------------------------------------------------------------------

/**
 * Suggest entity types for an invalid input. Uses a narrower default pool
 * that excludes 'event' for commands that don't handle events.
 *
 * @param {string} input           The invalid entity type string
 * @param {string[]} [candidates] Valid entity types (defaults to all 5)
 * @returns {string[]} Array of suggested entity types
 */
function suggestEntityType(input, candidates) {
  const pool = candidates || ['sprint', 'task', 'bug', 'event', 'feature'];
  return suggest(input, pool);
}

// ---------------------------------------------------------------------------
// formatSuggestion() — render suggestions as "(Did you mean ...?)" string
// ---------------------------------------------------------------------------

/**
 * Format an array of suggestions into a human-readable "(Did you mean ...?)" string.
 *
 * @param {string[]} suggestions Array of 1–3 suggestion strings
 * @returns {string} Formatted string, e.g. '(Did you mean "X"?)' or empty string
 */
function formatSuggestion(suggestions) {
  if (!Array.isArray(suggestions) || suggestions.length === 0) return '';

  const quoted = suggestions.map(function(s) { return '"' + s + '"'; });

  if (quoted.length === 1) {
    return '(Did you mean ' + quoted[0] + '?)';
  }
  if (quoted.length === 2) {
    return '(Did you mean ' + quoted[0] + ' or ' + quoted[1] + '?)';
  }
  // 3 items: Oxford comma
  return '(Did you mean ' + quoted[0] + ', ' + quoted[1] + ', or ' + quoted[2] + '?)';
}

module.exports = {
  levenshtein,
  normalizeForMatch,
  suggest,
  suggestEntityType,
  formatSuggestion,
  DRIFT_MAP
};