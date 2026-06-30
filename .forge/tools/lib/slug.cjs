'use strict';
// lib/slug.cjs — canonical slug generator.
//
// DECISION (FORGE-S25-T07, closes N-T-4):
//   Default maxLen=30 — matches the canonical behavior in tools/seed-store.cjs.
//   tools/lib/store-facade.cjs had no truncation; this module corrects that divergence.
//
// API:
//   deriveSlug(title: string, options?: { maxLen?: number }) → string
//
//   Derives a URL-safe slug from a human-readable title:
//   1. Lowercase
//   2. Replace runs of non-alphanumeric characters with a single hyphen
//   3. Strip leading and trailing hyphens
//   4. Truncate to maxLen characters
//   5. Strip trailing hyphens that appear after truncation
//
//   This behavior matches tools/seed-store.cjs:20 exactly (the canonical
//   reference implementation). store-facade.cjs is now the consumer that
//   was previously diverging (no truncation).

/**
 * Derive a URL-safe slug from a title.
 *
 * @param {string} title - Human-readable title string.
 * @param {Object} [options]
 * @param {number} [options.maxLen=30] - Maximum slug length (default 30).
 * @returns {string} - The derived slug.
 */
function deriveSlug(title, { maxLen = 30 } = {}) {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, maxLen)
    .replace(/-+$/g, '');
}

module.exports = { deriveSlug };
