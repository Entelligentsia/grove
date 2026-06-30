'use strict';
// lib/frontmatter.cjs — canonical YAML frontmatter extractor.
//
// CRLF POSTURE (canonical, enforced by test suite):
//   Normalize to LF on read. All \r characters are stripped before processing.
//   Output (both frontmatter and body) always uses LF-only line endings,
//   regardless of the source line ending style (LF / CRLF / CR-only / mixed).
//
//   Rationale: plugin template files are distributed and consumed across
//   platforms; LF is the canonical line ending for the forge plugin. Two prior
//   divergent implementations (build-base-pack.cjs and substitute-placeholders.cjs)
//   differed in CRLF handling — this module resolves them to a single behavior.
//
//   To preserve CRLF on write, the caller must re-introduce \r\n explicitly;
//   this module does not offer a write path.
//
// API:
//   extractFrontmatter(content: string) → { frontmatter: string|null, body: string }
//
//   - frontmatter: the YAML block including both opening and closing ---
//     delimiters, normalized to LF. null if no valid frontmatter found.
//   - body: the content after the closing --- delimiter, normalized to LF.
//     If no frontmatter found, body is the full input (LF-normalized).
//
// Opening --- MUST appear at column 0 (no leading whitespace).

/**
 * Extract YAML frontmatter from content.
 *
 * @param {string} content - Raw file content (any line ending style).
 * @returns {{ frontmatter: string|null, body: string }}
 */
function extractFrontmatter(content) {
  // Normalize all line endings to LF before any processing.
  // This handles CRLF (\r\n), CR-only (\r), and mixed inputs uniformly.
  const normalized = content.replace(/\r\n/g, '\n').replace(/\r/g, '\n');

  // Opening --- must be at column 0, immediately followed by \n.
  if (!normalized.startsWith('---\n') && normalized !== '---') {
    return { frontmatter: null, body: normalized };
  }

  const lines = normalized.split('\n');

  // Find the closing --- delimiter (must be on its own line, exact match).
  for (let i = 1; i < lines.length; i++) {
    if (lines[i] === '---') {
      const frontmatterLines = lines.slice(0, i + 1);
      const bodyLines = lines.slice(i + 1);

      const frontmatter = frontmatterLines.join('\n') + '\n';
      const body = bodyLines.join('\n');

      return { frontmatter, body };
    }
  }

  // No closing --- found — malformed frontmatter; return full normalized content as body.
  return { frontmatter: null, body: normalized };
}

module.exports = { extractFrontmatter };
