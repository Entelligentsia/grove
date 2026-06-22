# Changelog

All notable changes to grove are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and grove adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.3]: https://github.com/Entelligentsia/grove/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/Entelligentsia/grove/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/Entelligentsia/grove/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Entelligentsia/grove/releases/tag/v0.1.0
