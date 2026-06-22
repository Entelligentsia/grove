# grove — developer guide for Claude

grove gives coding agents **structural, byte-precise, token-cheap access to a
codebase** via tree-sitter, instead of reading whole files. It is a single Rust
binary with two faces — a human CLI (`grove <verb>`) and an MCP server
(`grove serve`) — over one engine. Grammars load at runtime from a WASM registry,
so new languages need no recompile.

Read [`VISION.md`](VISION.md) for the product vision and [`README.md`](README.md)
for usage. This file is the orientation for *continuing development*.

## Architecture — one engine, two faces

```
src/main.rs      CLI dispatch (clap) — every verb; thin, delegates to modules
src/ops.rs       the operations as a library — the shared engine BOTH faces call
src/mcp.rs       MCP server — newline-delimited JSON-RPC 2.0 over stdio (hand-rolled)
src/engine.rs    wasm load + Query-based tag extraction + check + position helpers
src/registry.rs  grammar resolver, cache-location precedence, catalog/index, lockfile
src/fetch.rs     `grove fetch` — install grammars from the hosted registry
src/ingest.rs    `grove ingest` — build registry artifacts from upstream releases
src/init.rs      `grove init` — wire grove into a project (.mcp.json + CLAUDE.md + lock)
```

Data flow: `main`/`mcp` → `ops` → `engine` (+ `registry` for grammar resolution).
**Never put engine logic in `main` or `mcp`** — they only format. `ops` returns
typed `Symbol`/`Defect`/etc.; the CLI prints tables, the MCP server emits JSON.

## The tool surface (6 tools, the agent loop)

`outline` (file skeleton) · `symbols` (find across a dir) · `source` (one symbol's
code) · `check` (ERROR/MISSING nodes — post-edit verify) · `callers` (call sites +
enclosing fn) · `definition` (go-to-def by name or `--at file:row:col`).

All carry a stable `symbol-id` (`<lang>:<relpath>#<name>@<row>`). `outline` is
tiered (`--kind`, `--detail 0|1|2`) so big files stay cheap. MCP results are
compact JSON; tool errors come back as `isError: true` so the model can recover.

## How grammars work (important)

- A grammar = `grammar.wasm` (tree-sitter parser, native **`dylink.0`** module) +
  `tags.scm` (definition/reference query) + `manifest.json` (extensions,
  `source` provenance, and a node-kind **`profile`**).
- **Tags are extracted via the Query engine, NOT `tree-sitter-tags`** — that crate
  can't drive a wasm-loaded language (it sets a language with no wasm store). See
  `engine::extract`: it runs `tags.scm` and interprets `@definition.*`/
  `@reference.*`/`@name` captures, dedups overlapping matches by (range, is_def).
- The **profile is data in the manifest** (`function_kinds`, `containers`,
  `identifier_kinds`), not code. It drives `parent` grouping, `callers`' enclosing
  function, and go-to-def. A new language gets the full surface by dropping a
  registry dir — no recompile. Languages without a profile still get the core tools.
- `engine.rs` caches a loaded grammar (wasm store + parser + compiled query) per
  process in a thread-local, keyed by language name.

## Registry & cache

- **Resolution precedence** (first existing wins): `GROVE_REGISTRY` env →
  `<project>/.grove/grammars/` → OS cache (`~/.cache/grove/grammars` on Linux, etc.)
  → dev tree (`registry/` next to the crate). `grove registry` shows it.
- The repo ships a **3-language dev stub** in `registry/` (rust, python,
  javascript). The full 27-language registry lives in the **separate
  `Entelligentsia/grove-registry` repo**, installed via `grove fetch`.
- **Hosted layout (split host):** small text (`index.json`, per-lang `tags.scm` +
  `manifest.json`) in the repo (served via `raw.githubusercontent.com`); heavy
  `grammar.wasm` as **GitHub Release assets** (`grammars-v1`). The catalog
  (`index.json`, schema 2) has `release_base` + per-file `{sha256, asset?}`;
  `fetch` routes wasm→release, text→repo, and **verifies every hash** before
  writing (atomic).
- `registry-sources.json` is the curated spec (`repo`, `rev`, `wasm_asset`,
  `extensions`, `profile`) that `grove ingest` builds the registry from.

## Commands

```
# agent-facing tools
grove outline <file> [--kind K] [--detail 0|1|2]
grove symbols <dir> [--kind K] [--name SUB] [--refs]
grove source  <id> | <file> <name>
grove check   <file>
grove callers <name> [-d <dir>]
grove definition <name> [-d <dir>] | --at <file:row:col>
grove serve                         # MCP server over stdio

# setup / registry
grove init [path] [--dry-run]       # write .mcp.json + CLAUDE.md + grove.lock
grove fetch [langs...] [--force]    # install grammars into the OS cache
grove languages                     # list registry languages
grove registry                      # show resolved registry root + search order
grove lock                          # write grove.lock (version + wasm sha256)

# registry maintainer
grove ingest [langs...] [--sources registry-sources.json] [--out registry]
grove index  [dir] [--release-base <url>] [-o index.json]
```

## Build / test / run

- **Build:** `cargo build --release`. First build compiles wasmtime (~30s, the
  `tree-sitter` `wasm` feature); after that it's incremental (~2s).
- Toolchain here is **cargo 1.87** — do NOT path-depend on the tree-sitter
  workspace crates (they target edition 2024 / rust 1.90). Use crates.io.
- **Run grammars from the cache or dev stub.** To exercise a language not in the
  dev stub without publishing, `GROVE_REGISTRY=<dir> grove ...` or `grove fetch` it.
- **Test beds:** `../grove-test` (a ripgrep clone, with `.mcp.json` + `CLAUDE.md`
  for manual MCP testing — see its `GROVE_TESTING.md`). Rust on ripgrep should
  yield **3317 definitions** (regression anchor).
- **MCP smoke test** without an agent:
  ```bash
  printf '%s\n' \
   '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}' \
   '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
   | ./target/release/grove serve
  ```
- **Tests:** `cargo test` runs in-module unit tests (every `src/*.rs` has a
  `#[cfg(test)] mod tests`) plus the CLI integration suite in `tests/cli.rs`,
  which shells out to the built binary against the dev stub
  (`GROVE_REGISTRY=registry`). Unit tests resolve real grammars via the registry
  precedence (OS cache or dev stub); keep assertions registry-root-agnostic
  unless the test pins `GROVE_REGISTRY` (the integration suite does, so it can
  assert the 3-language counts). Coverage: `cargo llvm-cov --summary-only`
  (~83% lines; the gaps are network paths in `fetch`/`ingest`/`init` and the
  `serve` stdio loop). Network-dependent paths are tested up to the
  error-before-fetch boundary, not mocked.

## Conventions

- Rust, `anyhow` for errors with `.context(...)`; fail fast with descriptive
  messages. Match the surrounding style; keep `main`/`mcp` thin.
- "Boring and obvious" over clever. Single responsibility per function.
- Files end with a newline. `cargo build` must be warning-clean.
- **Commits:** do NOT add `Co-Authored-By` lines. Branch before committing to a
  default branch. Conventional-commit style (`feat(registry): …`).
- This repo dogfoods itself: `.mcp.json` registers `grove serve`. A session here
  can use grove's own tools on its Rust source (rust is in the dev stub).

## Key design decisions (don't re-litigate)

- **WASM registry, not statically-linked grammar crates** — add languages with no
  recompile, no toolchain on the consumer. (CodeRLM took the static path; grove's
  wedge is the registry + native MCP.)
- **Native `dylink.0` wasms only.** The npm/web `tree-sitter-wasms` use legacy
  `dylink` and will NOT load. Use official tree-sitter *release* wasms, or build
  with `tree-sitter build --wasm` (wasi-sdk).
- **grove owns the served artifacts** (content-addressed, provenance-attributed) —
  reproducibility and integrity over redirecting to upstream URLs at fetch time.
- **Availability ≠ adoption** (VISION §6.4.1): registering the MCP server isn't
  enough; `init` also writes a `CLAUDE.md` steering directive, because a cold agent
  defaults to grep/whole-file reads otherwise.

## Roadmap (what's next)

Tier 1 — make it real: **ship the grove binary** (`cargo install` + GitHub release
prebuilts + brew/npm); **CI** (wire `cargo test` — the suite now exists, see
Conventions — into GitHub Actions); **registry CI** (automate ingest→index→
publish on new tree-sitter releases).
Tier 2 — complete the loop: **`map`** (repo orient — highest agent value) and
**`grep`** (scope-aware); **adoption/eval** (E0/E1 + token-savings harness).
Tier 3 — depth: scope-aware `callers`/`definition` via the tags `locals` query;
staleness/incremental (`Tree::edit` + watcher); more agent adapters
(Codex/Cursor `AGENTS.md`); `grove add <repo>` for BYO grammars; profiles/tags for
the 12 minimal-profile languages (bash/julia/haskell ship no upstream tags).

## Gotchas

- After `cd` in a shell, `./target/release/grove` breaks — use an absolute path.
- jsDelivr 502s intermittently on per-file cold fetches; default host is raw
  GitHub for that reason. Wasm is on Releases' CDN regardless.
- `grove-registry` git history still holds the pre-split 55MB; serving is fine
  (refs serve the slim tree), clones are heavy — squash is a pending follow-up.
- Multi-grammar repos (typescript→typescript+tsx, ocaml→ml+mli) are separate
  registry entries sharing one upstream repo/tags.
- 12 languages have minimal (`{}`) profiles → core tools only; css/html/json/regex
  have no `tags.scm` upstream (ingest writes an empty one; they still `check`).
