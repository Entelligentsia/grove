# grove — developer guide for Claude

grove gives coding agents **structural, byte-precise, token-cheap access to a
codebase** via tree-sitter, instead of reading whole files. It is a single Rust
binary with two faces — a human CLI (`grove <verb>`) and an MCP server
(`grove serve`) — over one engine. Grammars load at runtime from a WASM registry,
so new languages need no recompile.

Read [`VISION.md`](VISION.md) for the product vision and [`README.md`](README.md)
for usage. This file is the orientation for *continuing development*.

## Architecture — one engine, two faces

The engine lives in the `core` crate (published as `grove-cst`); the `cli` crate
holds the two faces, formatting, TUIs, and harness glue:

```
core/src/ops.rs        structural operations — the shared engine BOTH faces call
core/src/engine.rs     wasm load + Query-based tag extraction + check + position helpers
core/src/registry.rs   grammar resolver, cache-location precedence, catalog/index, lockfile
core/src/fetch.rs      `grove fetch` — install grammars from the hosted registry
core/src/ingest.rs     `grove ingest` — build registry artifacts from upstream releases
core/src/init.rs       grammar provisioning half of `grove init`

cli/src/main.rs        CLI dispatch (clap) — every verb; thin, delegates to modules
cli/src/mcp.rs         MCP server — newline-delimited JSON-RPC 2.0 over stdio
cli/src/init.rs        `grove init [--as mcp|skill|both|mcp-llm]` — file-writing harness glue
cli/src/config_tui/    full-screen ratatui TUI (`grove config` verb) — incl. model-list dropdown
cli/src/trace_tui/     full-screen ratatui trace browser (`grove tap` verb): session→call→turn
cli/src/tap.rs         `grove tap` — enable session tracing + launch the trace browser (debug)
skills/grove/          SKILL.md — cross-harness skill, routes to MCP-or-CLI (npx skills add)

core/src/explore/      inner explorer engine (mcp-llm mode — EXPERIMENTAL)
  mod.rs               re-exports; public surface is run_explore[_reporting]()
  config.rs            ExploreConfig — .grove/explore.json serde + atomic save (tap, trace_retain)
  client.rs            ChatClient trait + OpenAiCompatClient + health_probe() + list_models() + Usage
  agent.rs             bounded loop (≤ 6 turns, forced-final-answer, no byte budget); plan-first state machine + progress + trace
  trace.rs             TraceWriter (per-session JSONL under .grove/traces/) + request/response pretty-printers
  toolset.rs           the 4 inner tools (Grove/Read/Glob/Grep) + submit_plan — schemas + dispatch
  steering.rs          per-arm system prompts (standard=merit / balanced=plan-first / aggressive=coerce)
  grounding.rs         <final_answer> citation parse + filesystem validation
  prompts/             system/tool/steering prompt assets, embedded verbatim (include_str!)
```

**mcp-llm mode is experimental** (unreleased) — a direct port of the delegation
study's bench agent. The default CLI + 7-tool `grove serve` are the stable surface.

Data flow: `main`/`mcp` → `ops` → `engine` (+ `registry` for grammar resolution).
For mcp-llm mode: `mcp.rs` → `core::explore::run_explore` → inner loop → answer.
**Never put engine logic in `main` or `mcp`** — they only format. `ops` returns
typed `Symbol`/`Defect`/etc.; the CLI prints tables, the MCP server emits JSON.

## The tool surface (7 tools, the agent loop)

`outline` (file skeleton) · `symbols` (find across a dir) · `source` (one symbol's
code) · `check` (ERROR/MISSING nodes — post-edit verify) · `callers` (call sites +
enclosing fn) · `map` (directory dependency graph — defs + outgoing refs, no bodies) ·
`definition` (go-to-def by name or `--at file:line:col`, 1-based).

All carry a stable `symbol-id` (`<lang>:<relpath>#<name>@<line>`, line 1-based —
lines/cols are 1-based across the whole surface, `grep -n` convention). `outline` is
tiered (`--kind`, `--detail 0|1|2`) so big files stay cheap. `map` is the
breadth-control tool: it returns a directory's definitions grouped by file, each
with its outgoing references (which other symbols it calls/uses), replacing many
`symbols`+`source` round-trips with one call. MCP results are compact JSON; tool
errors come back as `isError: true` so the model can recover.

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
grove map     <dir> [--kind K] [--name SUB]
grove definition <name> [-d <dir>] | --at <file:line:col>   # line/col 1-based
grove serve                         # MCP server over stdio

# setup / registry
grove init [path] [--as mcp|skill|both|mcp-llm] [--dry-run]  # provision grammars + chosen harness glue
grove config [path]                 # open the explore config TUI (requires TTY); Tap toggle + model dropdown
grove serve [path] [--explore] [--standard]  # MCP server; mode flags override config
grove tap [path] [--no-enable]      # enable session tracing (.grove/traces/) + browse it in a TUI
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
- **Test beds:** `../git` (the git source tree) is the local C bed for hands-on
  MCP poking — its `.mcp.json` registers grove. For systematic regression + eval
  across languages use the **grove-testbench** harness (`../grove-testbench`):
  Tier-1 zero-token probes (`scripts/run-probes.sh`) and Tier-2 baseline-vs-grove
  agent races. The Rust def-count regression anchor (formerly ripgrep's **3317
  definitions** in the retired `../grove-test` bed) now rides on tokio there
  (`probes/def-count.tsv`).
- **Local agent testing:** `scripts/setup-local-test.sh [lang ...]` builds the
  release binary, installs it over the npm-vendored one the local bed's `.mcp.json`
  points at, regenerates the requested grammars in the OS cache via the real
  `ingest` pipeline (so `registry-sources.json` `extra_tags` are applied), and
  verifies against `../git`. Re-run after a change, then start a fresh agent
  session in the bed so its MCP server reloads.
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
  assert the 3-language counts). Coverage: `cargo llvm-cov --workspace
  --summary-only` (~81% lines / ~83% functions; the gaps are network paths in
  `fetch`/`ingest` and the `serve` stdio loop). Network-dependent paths are
  tested up to the error-before-fetch boundary, not mocked.
- **Releasing:** see [`RELEASING.md`](RELEASING.md). In short: bump the version in
  `Cargo.toml` + `Cargo.lock` + `dist/npm/package.json` and add a dated
  `CHANGELOG.md` section, PR to `main`, then push a `vX.Y.Z` tag — that triggers
  `release.yml` to build the 5 platform binaries and cut the GitHub Release.
  Afterwards: `npm publish` from `dist/npm/`, and regenerate the Homebrew tap via
  `dist/homebrew/update-formula.sh vX.Y.Z`.

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
Tier 3 — depth: scope-aware `definition --at` via the `locals` query (ADR 0001
Step 1, `engine::resolve_local_at`) and import-edge cross-file resolution (ADR
0001 Step 2, `engine::extract_imports` + `ops::resolve_import_at`, python/js) are
**done**; next is rust `use_path` import resolution, scope-aware `callers`, and
wider `locals.scm`/`imports.scm` coverage;
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

<!-- forge-kb-links: managed by Forge — do not edit manually -->
## Forge Knowledge Base

| Index | Contents |
|-------|----------|
| [MASTER_INDEX](engineering/MASTER_INDEX.md) | All sprints, tasks, bugs, and features |
| [Architecture](engineering/architecture/INDEX.md) | Stack, processes, database, routing, deployment |
| [Business Domain](engineering/business-domain/INDEX.md) | Entity model and domain concepts |

Personas live in `.forge/personas/`.
<!-- /forge-kb-links -->

<!-- forge-workflow-links: managed by Forge — do not edit manually -->
## Forge Workflows

| Workflow | Purpose |
|----------|---------|
| [Plan](.forge/workflows/plan_task.md) | Research codebase → implementation plan |
| [Implement](.forge/workflows/implement_plan.md) | Execute approved plan → code changes |
| [Sprint plan](.forge/workflows/architect_sprint_plan.md) | Sprint planning and task decomposition |
| [Sprint intake](.forge/workflows/architect_sprint_intake.md) | Sprint intake and requirements elicitation |
<!-- /forge-workflow-links -->
