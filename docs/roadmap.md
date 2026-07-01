# Roadmap & repo layout

## Not yet (roadmap)

- **No staleness / incremental reparse** — grove parses on demand; a file watcher
  + `Tree::edit` is ahead.
- **`callers` / `definition` are name-based** — no receiver-type or local-scope
  resolution (the tags `locals` query is a Tier-3 item).
- **12 languages ship a minimal profile** (core tools only); css/html/json/regex
  have no upstream `tags.scm` (they still `check`). See
  [Languages & grammars](languages.md#profiles-why-some-languages-do-more).
- **No `map`-over-repo / scope-aware `grep` yet** — repo-orient (`map`) and a
  structural `grep` are next in the loop.

See [VISION §9 Build order / roadmap](../VISION.md) for the full plan.

## Repo layout

grove is a **Cargo workspace** with two member crates: `grove-core` (the
library — engine, registry, fetch, ingest) and `grove` (the `cli/` binary —
CLI dispatch + MCP server, consuming `grove-core`).

```
Cargo.toml         [workspace] — members = ["core", "cli"]
core/              grove-core — the reusable library crate (publishable to crates.io)
  src/lib.rs         curated public surface — re-exports ops + provision_project + return types
  src/ops.rs         the operations as a library — the shared engine both faces call
  src/engine.rs      wasm load + Query-based tags, source slicing, check, position resolution
                     (node-kind profiles are data — they come from each manifest, not code)
  src/registry.rs    grammar resolver, extension map, lockfile — the registry spine
  src/fetch.rs       `grove fetch` — download grammars from the hosted registry (GitHub/CDN)
  src/ingest.rs      `grove ingest` — build registry artifacts from official tree-sitter releases
  src/init.rs        `provision_project` — detect langs + provision grammars (clap-free)
cli/               grove — the binary crate (depends on grove-core by path)
  src/main.rs        CLI dispatch (clap) — six verbs + init/languages/lock/serve
  src/init.rs        `grove init [--as mcp|skill|both]` — harness glue over provision_project
  src/mcp.rs         MCP server — newline-delimited JSON-RPC over stdio
registry/<lang>/   grammar.wasm + tags.scm + manifest.json (the registry stub)
registry-sources.json  curated specs (repo/rev/extensions/profile) ingest builds from
skills/grove/      SKILL.md — the cross-harness skill (npx skills add Entelligentsia/grove)
docs/              install / setup / languages / tools / mcp / roadmap (this site)
docs/assets/       grove_demo.cast + grove_demo.gif — the README demo
docs/assets/langs/  language logos (devicon SVGs) for the README language grid
```

Data flow: `main` / `mcp` (in `cli/`) → `grove_core::ops` → `engine` (+
`registry` for grammar resolution). Engine logic never lives in `main` or `mcp` —
they only format; `ops` returns typed `Symbol` / `Defect` / etc.

---

Back to [README](../README.md) · [VISION](../VISION.md) · [CHANGELOG](../CHANGELOG.md)