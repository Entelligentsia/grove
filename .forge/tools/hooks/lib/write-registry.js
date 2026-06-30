'use strict';

// Write-boundary registry: maps Forge-owned filesystem path patterns to the
// schema (and kind) that the write-boundary hook enforces on Write/Edit/
// MultiEdit. FIRST match wins, so order matters — place more specific
// patterns before their generalizations (sidecar before event).
//
// Patterns are anchored to absolute-path suffixes so they match whether the
// tool invocation used an absolute or project-relative path. Every pattern
// assumes the Forge store lives at `.forge/store/` (the installed default).
//
// If NO pattern matches a write, the hook is a no-op: Forge does not validate
// non-Forge-owned files. The registry is an allowlist of paths we claim, not
// a denylist of paths we reject.

const REGISTRY = [
  // Core store entities — flat files under .forge/store/{kind}s/<id>.json
  { pattern: /(?:^|\/)\.forge\/store\/features\/[^/]+\.json$/,             schema: 'feature.schema.json',         kind: 'feature' },
  { pattern: /(?:^|\/)\.forge\/store\/sprints\/[^/]+\.json$/,              schema: 'sprint.schema.json',          kind: 'sprint' },
  { pattern: /(?:^|\/)\.forge\/store\/tasks\/[^/]+\.json$/,                schema: 'task.schema.json',            kind: 'task' },
  { pattern: /(?:^|\/)\.forge\/store\/bugs\/[^/]+\.json$/,                 schema: 'bug.schema.json',             kind: 'bug' },

  // Event sidecars — prefixed with `_` and suffixed with `_usage.json`.
  // Must come BEFORE the general event pattern so the leading `_` file is
  // classified as a sidecar, not an event with a malformed id.
  { pattern: /(?:^|\/)\.forge\/store\/events\/[^/]+\/_[^/]+_usage\.json$/, schema: 'event-sidecar.schema.json',   kind: 'event-sidecar' },

  // Canonical events — any other .json under events/<bucket>/ that does NOT
  // start with an underscore. (Ghost events are not written through Write/Edit
  // tools, so we don't special-case them here.)
  { pattern: /(?:^|\/)\.forge\/store\/events\/[^/]+\/[^_/][^/]*\.json$/,   schema: 'event.schema.json',           kind: 'event' },

  // Collation watermark
  { pattern: /(?:^|\/)\.forge\/store\/COLLATION_STATE\.json$/,             schema: 'collation-state.schema.json', kind: 'collation-state' },

  // Progress log — line-oriented, not JSON. The hook's parser splits newly
  // appended lines and validates each as a pipe-delimited progress entry.
  { pattern: /(?:^|\/)\.forge\/store\/events\/[^/]+\/progress\.log$/,      schema: 'progress-entry.schema.json',  kind: 'progress-line', format: 'line-pipe-delimited' },
];

function matchRegistry(absPath) {
  if (!absPath || typeof absPath !== 'string') return null;
  for (const entry of REGISTRY) {
    if (entry.pattern.test(absPath)) return entry;
  }
  return null;
}

// REGISTRY is intentionally not exported — it is a private implementation detail
// of this module. Callers use matchRegistry() for lookups. Exporting REGISTRY
// would expose internal pattern structures to external consumers and create
// coupling that makes future pattern changes harder. (H-3, FORGE-S25-T14)
module.exports = { matchRegistry };
