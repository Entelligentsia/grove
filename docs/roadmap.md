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

```
registry/<lang>/   grammar.wasm + tags.scm + manifest.json (the registry stub)
skills/grove/      SKILL.md — the cross-harness skill (npx skills add Entelligentsia/grove)
docs/              install / setup / languages / tools / mcp / roadmap (this site)
src/main.rs        CLI dispatch (clap) — six verbs + init/languages/lock/serve
src/init.rs        `grove init [--as mcp|skill|both]` — detect langs, provision grammars + harness glue
src/fetch.rs       `grove fetch` — download grammars from the hosted registry (GitHub/CDN)
src/ingest.rs      `grove ingest` — build registry artifacts from official tree-sitter releases
registry-sources.json  curated specs (repo/rev/extensions/profile) ingest builds from
src/registry.rs    grammar resolver, extension map, lockfile — the registry spine
src/ops.rs         the operations as a library — the shared engine both faces call
src/mcp.rs         MCP server — newline-delimited JSON-RPC over stdio
src/engine.rs      wasm load + Query-based tags, source slicing, check, position resolution
                   (node-kind profiles are data — they come from each manifest, not code)
docs/assets/       grove_demo.cast + grove_demo.gif — the README demo
```

Data flow: `main` / `mcp` → `ops` → `engine` (+ `registry` for grammar
resolution). Engine logic never lives in `main` or `mcp` — they only format;
`ops` returns typed `Symbol` / `Defect` / etc.

---

Back to [README](../README.md) · [VISION](../VISION.md) · [CHANGELOG](../CHANGELOG.md)