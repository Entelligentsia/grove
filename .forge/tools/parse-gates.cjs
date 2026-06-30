'use strict';

// parse-gates.cjs — minimal DSL for per-phase gate declarations.
//
// Grammar (closed — unknown directives throw):
//   artifact <path> [min=<bytes>]
//   require  <field> <op> <value>
//   forbid   <field> <op> <value>
//   after    <phase> = <approved|revision>
//
// Predicate ops: ==, !=, in [v1, v2, ...]
// Blank lines and lines beginning with # (optionally indented) are ignored.
// Gates live in fenced blocks self-identified by phase via a fence-label
// attribute, e.g. ```gates phase=implement . The fence is self-contained;
// gate blocks can appear anywhere in the workflow file without depending on
// surrounding heading structure.

const FENCE_OPEN = /^```gates\s+phase=([A-Za-z0-9_-]+)\s*$/;
const FENCE_CLOSE = /^```\s*$/;

// Verdicts a workflow can carry in an `after <phase> = <verdict>` directive.
// Mirrors `read-verdict.cjs § ALLOWED_VERDICTS` — both must accept the same
// set. `n/a` is the legitimate verdict for setup phases (plan, implement,
// triage) that do not produce an approval/revision signal. Rejecting it
// here caused EMBERGLOW-BUG-001 (v0.44.1) to halt at preflight exit 2
// when the new fix-bug meta tried to use `after triage = n/a`.
const VALID_VERDICTS = new Set(['approved', 'revision', 'n/a']);

function parseGates(markdown) {
  if (typeof markdown !== 'string' || markdown.length === 0) return {};
  const lines = markdown.split('\n');
  const result = {};

  let currentPhase = null;
  let inFence = false;
  let fenceStartLine = -1;
  let fenceBuffer = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const lineNo = i + 1;

    if (!inFence) {
      const m = line.match(FENCE_OPEN);
      if (m) {
        currentPhase = m[1];
        if (result[currentPhase]) {
          throw new Error(
            `parse-gates: line ${lineNo}: duplicate gates block for phase "${currentPhase}"`,
          );
        }
        inFence = true;
        fenceStartLine = lineNo;
        fenceBuffer = [];
      }
      continue;
    }

    // Inside a gates fence
    if (FENCE_CLOSE.test(line)) {
      result[currentPhase] = parseBlock(fenceBuffer, fenceStartLine);
      inFence = false;
      currentPhase = null;
      continue;
    }
    fenceBuffer.push({ text: line, lineNo });
  }

  if (inFence) {
    throw new Error(`parse-gates: unterminated \`\`\`gates fence opened at line ${fenceStartLine}`);
  }
  return result;
}

function parseBlock(bufferedLines, _fenceStartLine) {
  const spec = { artifacts: [], require: [], forbid: [], after: [] };
  for (const { text, lineNo } of bufferedLines) {
    const trimmed = text.trim();
    if (trimmed === '' || trimmed.startsWith('#')) continue;

    const firstSpace = trimmed.search(/\s/);
    if (firstSpace < 0) {
      throw new Error(`parse-gates: line ${lineNo}: malformed directive "${trimmed}"`);
    }
    const directive = trimmed.slice(0, firstSpace);
    const rest = trimmed.slice(firstSpace + 1).trim();

    switch (directive) {
      case 'artifact':
        spec.artifacts.push(parseArtifact(rest, lineNo));
        break;
      case 'require':
        spec.require.push(parsePredicate(rest, lineNo));
        break;
      case 'forbid':
        spec.forbid.push(parsePredicate(rest, lineNo));
        break;
      case 'after':
        spec.after.push(parseAfter(rest, lineNo));
        break;
      default:
        throw new Error(`parse-gates: line ${lineNo}: unknown directive "${directive}"`);
    }
  }
  return spec;
}

function parseArtifact(rest, lineNo) {
  const minMatch = rest.match(/\bmin=(\d+)\s*$/);
  let path = rest;
  let minBytes = 0;
  if (minMatch) {
    minBytes = parseInt(minMatch[1], 10);
    path = rest.slice(0, minMatch.index).trim();
  }
  if (!path) {
    throw new Error(`parse-gates: line ${lineNo}: artifact directive missing path`);
  }
  return { path, minBytes };
}

function parsePredicate(rest, lineNo) {
  // in-list form: <field> in [v1, v2, ...]
  const inMatch = rest.match(/^(\S+)\s+in\s+\[(.*)\]\s*$/);
  if (inMatch) {
    const field = inMatch[1];
    const value = inMatch[2]
      .split(',')
      .map((v) => v.trim())
      .filter((v) => v.length > 0);
    return { field, op: 'in', value };
  }
  // binary form: <field> <op> <value>
  const binMatch = rest.match(/^(\S+)\s+(==|!=)\s+(.+?)\s*$/);
  if (binMatch) {
    return { field: binMatch[1], op: binMatch[2], value: binMatch[3].trim() };
  }
  throw new Error(
    `parse-gates: line ${lineNo}: unknown predicate op or malformed predicate "${rest}"`,
  );
}

function parseAfter(rest, lineNo) {
  const m = rest.match(/^(\S+)\s*=\s*(\S+)\s*$/);
  if (!m) {
    throw new Error(`parse-gates: line ${lineNo}: malformed "after" directive "${rest}"`);
  }
  const verdict = m[2].toLowerCase();
  if (!VALID_VERDICTS.has(verdict)) {
    throw new Error(
      `parse-gates: line ${lineNo}: "after" verdict must be "approved", "revision", or "n/a", got "${m[2]}"`,
    );
  }
  return { phase: m[1], verdict };
}

// parseOutputs — same fence-scan logic as parseGates but for ```outputs phase=X blocks.
// Closed grammar: only `artifact` and `require` directives are allowed.
// Unknown directives throw (parity with parseGates).
const OUTPUTS_FENCE_OPEN = /^```outputs\s+phase=([A-Za-z0-9_-]+)\s*$/;

function parseOutputs(markdown) {
  if (typeof markdown !== 'string' || markdown.length === 0) return {};
  const lines = markdown.split('\n');
  const result = {};
  let currentPhase = null;
  let inFence = false;
  let fenceStartLine = -1;
  let fenceBuffer = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const lineNo = i + 1;
    if (!inFence) {
      const m = line.match(OUTPUTS_FENCE_OPEN);
      if (m) {
        currentPhase = m[1];
        if (result[currentPhase]) {
          throw new Error(
            `parse-gates: line ${lineNo}: duplicate outputs block for phase "${currentPhase}"`,
          );
        }
        inFence = true;
        fenceStartLine = lineNo;
        fenceBuffer = [];
      }
      continue;
    }
    // Inside an outputs fence
    if (FENCE_CLOSE.test(line)) {
      result[currentPhase] = parseOutputsBlock(fenceBuffer, fenceStartLine);
      inFence = false;
      currentPhase = null;
      fenceBuffer = [];
      continue;
    }
    fenceBuffer.push({ text: line, lineNo });
  }
  if (inFence) {
    throw new Error(`parse-gates: unterminated \`\`\`outputs fence opened at line ${fenceStartLine}`);
  }
  return result;
}

function parseOutputsBlock(bufferedLines, _fenceStartLine) {
  const spec = { artifacts: [], require: [] };
  for (const { text, lineNo } of bufferedLines) {
    const trimmed = text.trim();
    if (trimmed === '' || trimmed.startsWith('#')) continue;
    const firstSpace = trimmed.search(/\s/);
    if (firstSpace < 0) {
      throw new Error(`parse-gates: line ${lineNo}: malformed directive "${trimmed}"`);
    }
    const directive = trimmed.slice(0, firstSpace);
    const rest = trimmed.slice(firstSpace + 1).trim();
    switch (directive) {
      case 'artifact':
        spec.artifacts.push(parseArtifact(rest, lineNo));
        break;
      case 'require':
        spec.require.push(parsePredicate(rest, lineNo));
        break;
      default:
        throw new Error(`parse-gates: line ${lineNo}: unknown directive "${directive}"`);
    }
  }
  return spec;
}

module.exports = { parseGates, parseOutputs };
