# Changelog

All notable changes to grove are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and grove adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.9] - 2026-06-27

### Added

- **Scope-aware `definition --at`** (ADR 0001, Step 1) — go-to-def from a usage
  position now resolves a name to its nearest enclosing **local** binding
  (parameter or `let`/assignment) before falling back to the directory-wide name
  lookup. A shadowing local correctly wins over a same-named global, so the
  result is the one binding the cursor refers to instead of a candidate list of
  every same-named symbol. Driven by an optional `locals.scm` (tree-sitter's
  standard `@local.scope` / `@local.definition` / `@local.reference` query) added
  per registry dir; grammars without one keep the previous behavior. Shipped for
  the rust/python/javascript dev stub; `ingest`/`index`/`fetch` now carry
  `locals.scm` through to the hosted registry.
- **Import-edge cross-file `definition --at`** (ADR 0001, Step 2) — when a name
  has no local binding, grove now follows an import statement to the **target
  file** and returns the definition there, instead of a directory-wide list of
  every same-named symbol. Aliases resolve to the original symbol
  (`from m import x as y` / `import { x as y } from …`). No index: at most one
  extra file is parsed, bounded by import depth, not repo size. Driven by an
  optional `imports.scm` query plus an `import_resolution` strategy in the
  manifest profile — `dotted_package` (Python `foo.bar` → `foo/bar.py`,
  `__init__.py`, and relative `.`/`..` imports) and `relative_path` (JS/TS
  `./util` → `./util.js`, `.jsx`, `/index.js`; bare specifiers are left to the
  directory-wide fallback). Shipped for python/javascript; carried through
  `ingest`/`index`/`fetch`. Out of scope (degrades to the candidate list):
  method/receiver typing, multi-hop re-exports, wildcard/dynamic imports.

### Changed

- **MCP/CLI/steering descriptions** now advertise `definition --at` as the
  precise, scope-aware, cross-file mode (no tool signatures changed) so agents
  reach for it from a usage position instead of a name lookup.

### Robustness

- Optional registry queries (`locals.scm`/`imports.scm`) compile **non-fatally**
  and their captures are **prefix-matched**, so a query authored against a
  different grammar version (or using subtyped captures like
  `@local.definition.function`) degrades gracefully instead of breaking the
  grammar's core tools.
- grove now **refuses tree-sitter supertype query syntax** (`(a/b)`) in optional
  queries, which can otherwise hard-crash the wasm query engine at match time —
  so a hosted registry file can no longer segfault grove.

## [0.1.8] - 2026-06-25

### Added

- **`grove init --as grammars`** — a fourth integration target that provisions
  grammars and writes `grove.lock`, but writes **no** harness glue: no
  `.mcp.json`, no CLAUDE.md steering block. For embedding hosts (e.g. an editor
  or agent runtime that registers grove's tools in-process and supplies its own
  steering), this leaves the project's own files untouched. The existing `mcp`,
  `skill`, and `both` targets are unchanged — they still write steering, since
  for a cold agent availability isn't adoption (VISION §6.4.1).

## [0.1.7] - 2026-06-23

### Fixed

- **Lines/columns are now 1-based across the whole surface** (CLI, MCP,
  symbol-ids, `outline`, `map`, `definition --at`). Previously grove reported
  raw 0-based tree-sitter rows, so every citation was one line low; the editor /
  `grep -n` convention now holds everywhere (#31).
- **`callers`**: include all reference kinds (not just `call`) — type
  references, implementation references, etc. are now surfaced as structural
  hits, so heavily-used class/type names return results instead of `[]` (#33).
- **`callers`**: textual fallback finds whole-word references the tags query
  misses (type annotations, imports, dynamic dispatch) with provenance
  (`structural` vs `textual`) so the agent can prioritise (#33).
- **`symbols`/`definition`/`callers`**: skip generated `.d.ts`/`.d.cts`/`.d.mts`
  declaration files during directory walks so these tools answer from real
  source, not machine-generated decls (#32).
- **`map`**: report 1-based lines (it previously inherited the 0-based row bug).

> Note: #32 and #33 were listed under 0.1.6 below, but did not land in the
> 0.1.6 release code; they ship here in 0.1.7.

## [0.1.6] - 2026-06-23

### Added

- **`map` tool**: returns a compact structural map of a directory — every
  definition grouped by file, with each definition's outgoing references
  (which other symbols it calls/uses). No source bodies, just the dependency
  graph. Replaces many `symbols`+`source` round-trips with one call (#34).
- **Breadth control steering**: tool descriptions, MCP `instructions()`,
  init-written CLAUDE.md, and SKILL.md now actively steer agents toward `map`
  for architectural questions and away from sequential `source` fan-out (#34).
- CLI: `grove map <dir> [--kind K] [--name SUB]` with human and `--json` output.
- MCP: 7th tool `map` with plain `{type: object, properties}` schema (no
  top-level `anyOf`).

## [0.1.5] - 2026-06-22

### Added

- **C grammar navigation**: full symbol bodies (function definitions include
  the body, not just the declarator), callers with enclosing-function context,
  and file-scope variable captures (#25, #26, #27).
- **`definition`**: `file` column in output so multi-file results are
  self-locating without a follow-up `symbols`.
- **`struct`/`union` kind alias**: `--kind struct` and `--kind union` match the
  `class` kind (since tree-sitter tags all struct/union-likes as `class`).
- **Cross-harness skill** (`skills/grove/SKILL.md` + `.claude-plugin/marketplace.json`):
  grove's capabilities as an Agent Skill, installable across 70+ harnesses with
  `npx skills add Entelligentsia/grove`. The skill prefers grove's MCP tools
  when the host exposes them and falls back to the `grove` CLI otherwise, and
  self-installs the binary on first use if it's missing (#22).
- **`grove init --as mcp|skill|both`**: grammar provisioning + `grove.lock`
  happen for every target; `.mcp.json` + CLAUDE.md are written only for the
  MCP targets. Default is `mcp` (fully backward-compatible) (#22).
- **CI/release**: Node 24 pin; cross-compile and release workflow.

### Fixed

- **MCP `source` / `definition`**: dropped the top-level `anyOf` from their
  input schemas. Some MCP clients can't normalize a top-level `anyOf` and
  silently drop the tool during registration (#21).

## [0.1.4] - 2026-06-22

### Added

- **Cross-harness skill** (`skills/grove/SKILL.md` + `.claude-plugin/marketplace.json`):
  grove's capabilities as an Agent Skill, installable across 70+ harnesses with
  `npx skills add Entelligentsia/grove`. The skill prefers grove's MCP tools when
  the host exposes them and falls back to the `grove` CLI otherwise, and
  self-installs the binary on first use if it's missing (#22).
- **`grove init --as mcp|skill|both`**: grammar provisioning + `grove.lock`
  happen for every target; `.mcp.json` + CLAUDE.md are written only for the MCP
  targets. Default is `mcp` (fully backward-compatible) (#22).

### Fixed

- **MCP `source` / `definition`**: dropped the top-level `anyOf` from their input
  schemas. Some MCP clients can't normalize a top-level `anyOf` and silently drop
  the tool during registration, so those two tools went missing while the four
  flat-schema tools registered fine. Both now use plain object schemas; the
  mutually-exclusive argument forms are enforced at runtime (#21).

## [0.1.3] - 2026-06-22

### Security

- **`fetch`**: reject path traversal in catalog-supplied names — a hostile or
  MITM'd `index.json` can no longer escape the cache directory via `..` or path
  separators (#8).

### Fixed

- **`source`**: honor the `@row` in a symbol-id so duplicate-named definitions
  resolve to the requested one (#9).
- **`callers`**: drive the call-site filter from the grammar profile
  (`call_kinds`) instead of a hardcoded `"call"`, so grammars using
  `@reference.send` / `@reference.invocation` (Ruby/Elixir-style) report callers
  instead of silently returning none (#10).
- **MCP `definition`**: return one consistent `{resolved, definitions}` shape for
  both the `name` and `at` modes (was a bare array vs. an object) (#11).
- **MCP `outline`**: validate `detail` against `{0,1,2}` and return a tool error
  on anything else, instead of silently truncating via `as u8` (`256` → tier 0)
  (#12).

### Performance

- **`callers`**: parse each matched file once. `engine::extract_with_tree`
  returns the parse tree for the enclosing-function pass instead of re-parsing
  the identical bytes (#13).

### Changed

- **`index`**: `Cmd::Index` delegates path resolution and the catalog write to
  `registry::write_index`, keeping `main` thin (#14).
- **`registry`**: a single shared `registry::sha256` helper replaces three
  byte-for-byte copies, so the artifact hash format can never drift between the
  index/lockfile producer and `fetch`'s verifier (#15).

### Tests

- First real test suite: in-module unit tests across every module plus a CLI
  integration suite (`tests/cli.rs`) driving the built binary against the dev
  stub. Line coverage ~84% (#18).

## [0.1.2] - 2026-06-21

- Pre-CHANGELOG release. See the
  [v0.1.2 release](https://github.com/Entelligentsia/grove/releases/tag/v0.1.2).

## [0.1.1] - 2026-06-21

- Pre-CHANGELOG release. See the
  [v0.1.1 release](https://github.com/Entelligentsia/grove/releases/tag/v0.1.1).

## [0.1.0] - 2026-06-21

- Initial release.

[0.1.7]: https://github.com/Entelligentsia/grove/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/Entelligentsia/grove/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/Entelligentsia/grove/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/Entelligentsia/grove/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/Entelligentsia/grove/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/Entelligentsia/grove/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/Entelligentsia/grove/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Entelligentsia/grove/releases/tag/v0.1.0
