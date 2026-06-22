# grove — structural sight for coding agents

grove gives coding agents **structural, byte-precise, token-cheap access to a
codebase** via tree-sitter, instead of reading whole files. One engine, six
tools, **two faces** — a human CLI (`grove <verb>`) and an MCP server
(`grove serve`) — with grammars loaded **at runtime from a WASM registry**, so
new languages are added with no recompile and no toolchain on the consumer.

See [`VISION.md`](VISION.md) for the product vision.

## Install

Prebuilt binaries are attached to each [GitHub Release](https://github.com/Entelligentsia/grove/releases)
for Linux and macOS (x86_64 + aarch64) and Windows (x86_64).

```bash
# curl | sh — detects your platform, verifies the sha256, installs to ~/.local/bin
curl -fsSL https://raw.githubusercontent.com/Entelligentsia/grove/main/install.sh | sh

# Homebrew (macOS / Linux)
brew install Entelligentsia/grove/grove

# npm (provides the `grove` binary via a downloaded prebuilt)
npm install -g @entelligentsia/grove

# from source (no published crate — install straight from git)
cargo install --git https://github.com/Entelligentsia/grove
```

**As an agent skill** (cross-harness — Claude Code, Cursor, Codex, Cline, …),
via the [agent-skills tool](https://github.com/vercel-labs/skills):

```bash
npx skills add Entelligentsia/grove
```

The skill steers your agent to grove's MCP tools when present, else the `grove`
CLI — and **self-installs the binary on first use** if it's missing (`npm i -g
@entelligentsia/grove`, then `grove init --as skill` to fetch grammars). So this
one command is enough to get grove working in a fresh repo. See
[Setup](#setup--grove-init) for `grove init --as mcp|skill|both`.

## Build

```bash
cargo build --release
```

The first build compiles wasmtime (~30s); after that it's incremental. Single
binary at `target/release/grove`. No grammar is compiled into the binary.

## Setup — `grove init`

One command makes a project one where the agent *uses* grove:

```bash
grove init           # in your project root
```

It detects the project's languages from the hosted catalog (so it sees a
language even before its grammar is installed), **auto-fetches** the grammars the
project needs, then writes three things (idempotently, preserving anything
already there):

- **`.mcp.json`** — registers the grove MCP server (*availability* — the tools exist).
- **`CLAUDE.md`** — a steering directive in a marked section (*adoption* — the agent
  reaches for grove instead of grep/whole-file reads; see VISION §6.4.1).
- **`grove.lock`** — pins the detected grammars' version + wasm sha256.

`grove init --dry-run` detects without writing or fetching. Re-running only
updates grove's own pieces. Offline, it falls back to detecting from grammars
already in the cache.

### MCP, skill, or both — `grove init --as`

grove has one engine behind three faces: the CLI, the MCP server, and a
**cross-harness skill**. `--as` selects which integration `init` wires up
(grammar provisioning + `grove.lock` happens for every target):

```bash
grove init --as mcp     # default — .mcp.json + CLAUDE.md + grove.lock
grove init --as skill   # grammars + grove.lock only; install the skill below
grove init --as both    # MCP wiring and grammars, for skill + MCP side by side
```

The skill is distributed through the [agent-skills tool](https://github.com/vercel-labs/skills),
which works across 70+ harnesses (Claude Code, Cursor, Codex, Cline, …):

```bash
npx skills add Entelligentsia/grove
```

The skill **prefers grove's MCP tools when the host exposes them and falls back
to the `grove` CLI otherwise** — so MCP and the skill are equal partners over the
same engine. On first CLI use it self-bootstraps: if `grove` isn't on `PATH` it
installs the npm package globally, then runs `grove init --as skill` to fetch the
repo's grammars.

## Languages

A grammar is a `registry/<lang>/` directory holding `grammar.wasm` + `tags.scm` +
`manifest.json`. They load at runtime — `grove init` fetches what your project
needs, or fetch explicitly:

```bash
grove languages      # what's installed locally
grove fetch          # install all grammars from the hosted registry
grove lock           # write grove.lock pinning version + wasm sha256
```

The hosted registry ([Entelligentsia/grove-registry](https://github.com/Entelligentsia/grove-registry))
carries **all 27 official tree-sitter grammars** (c, cpp, c#, go, java, js, ts, tsx,
python, ruby, rust, php, scala, ocaml, ql, and more); `grove fetch` installs them.
The grove repo itself ships a small 3-language dev stub. Adding a language is
dropping a `registry/<lang>/` directory in — the binary doesn't change, and that
includes the *full* surface: the `manifest.json` carries a `profile` (container /
function / identifier node kinds) that drives `parent` grouping, `callers`'
enclosing-function, and go-to-def, so nothing language-specific is compiled in.

```jsonc
// registry/<lang>/manifest.json
{ "name": "go", "version": "…", "extensions": ["go"],
  "profile": {
    "function_kinds": ["function_declaration", "method_declaration"],
    "containers": [["type_declaration", "name"]],
    "identifier_kinds": ["identifier", "field_identifier"]
  } }
```

Build a grammar's wasm with `tree-sitter build --wasm` (emits the `dylink.0`
module the native runtime needs).

### Where grammars live

Grammars are a **cache** — reconstructible from the hosted registry and
content-addressed by `grove.lock` — so the standard home is the OS-native cache
location. grove resolves the registry root by precedence (first existing wins);
`grove registry` shows it:

1. **`GROVE_REGISTRY`** env var — explicit override (CI, tests, air-gapped).
2. **`<project>/.grove/grammars/`** — project-vendored grammars (commit them for
   hermetic / offline builds), found by walking up from the cwd.
3. **OS user cache** — the default shared store:
   - Linux: `~/.cache/grove/grammars` (honors `$XDG_CACHE_HOME`)
   - macOS: `~/Library/Caches/grove/grammars`
   - Windows: `%LOCALAPPDATA%\grove\grammars`
4. **dev source tree** (`registry/` next to this crate) — only in a checkout.

Layout under the root is `<lang>/{grammar.wasm, tags.scm, manifest.json}`.

### Fetching grammars

`grove fetch` pulls grammars from the hosted registry into the OS cache:

```bash
grove fetch                 # all languages in the catalog
grove fetch python rust     # just these
grove fetch python --force  # re-download
```

**grove owns the artifacts it serves.** Rather than redirecting to upstream URLs
at fetch time, grove ingests each grammar (official release wasm where it exists,
else built from source), normalizes it into the shape grove needs — `grammar.wasm`
(native `dylink.0`) + `tags.scm` + a `manifest.json` carrying the node-kind
`profile` — and hosts those bytes content-addressed, recording provenance
(`source.repo` / `source.rev`) for auditability. This guarantees the three travel
as one co-versioned unit and that `grove.lock` always resolves.

The host is the **[Entelligentsia/grove-registry](https://github.com/Entelligentsia/grove-registry)**
repo, split for efficiency: the small text files (`index.json`, per-language
`tags.scm` + `manifest.json`) live in the repo (served via
`raw.githubusercontent.com`), and the heavy `grammar.wasm` binaries are **GitHub
Release assets** (GitHub's CDN). The catalog's `release_base` + per-file `asset`
fields tell `fetch` where each file lives. **Every** file's sha256 is verified
against the catalog before it's written (download-verify-then-write, atomically),
so a corrupted or tampered artifact is rejected. Override the host with
`GROVE_REGISTRY_URL` (self-host, fork, or a local mirror).

### Building the registry (maintainer)

`grove ingest` builds the registry from a curated spec (`registry-sources.json`):
for each grammar it pulls the **official tree-sitter release wasm** + the repo's
`tags.scm` at a pinned rev, attaches grove's curated `profile`/`extensions`, writes
`registry/<lang>/`, and regenerates the catalog.

```bash
grove ingest                 # all grammars in registry-sources.json
grove ingest python rust     # just these
grove index registry         # (re)build index.json with per-file hashes
```

The spec records identity + provenance + the grove-authored profile; the wasm and
tags come from upstream and the version/`source` are pinned. `grove index` then
emits the `index.json` catalog (per language: version, provenance, content hash of
every served file) — the publish step for registry CI.

## Tools (the agent loop, in miniature)

| Phase | Command | What it does |
|---|---|---|
| Read | `grove outline <file> [--kind K] [--detail 0\|1\|2]` | compact definition skeleton: kind · name · parent · signature · id. Filter by kind / dial detail down for big files |
| Find | `grove symbols <dir> [--kind K] [--name SUB] [--refs]` | repo-wide symbol search (gitignore-aware) |
| Read | `grove source <id>` or `grove source <file> <name>` | full source of one symbol — no whole-file read |
| Verify | `grove check <file>` | ERROR / MISSING nodes (exit 1 if any) — post-edit syntax check |
| Trace | `grove callers <name> [-d <dir>]` | call sites of a symbol, each with its enclosing function |
| Trace | `grove definition <name> [-d <dir>]` or `grove definition --at <file:row:col>` | go-to-def, by name or from a usage position |

Add `--json` to any command for the agent-facing structured shape. Every result
carries a stable `symbol-id` (`<lang>:<relpath>#<name>@<row>`) usable across turns.

## Examples

```bash
grove languages
grove outline foo.py --kind class        # python, loaded from wasm at runtime
grove outline src/engine.rs --kind function
grove source  src/mcp.rs serve
grove callers extract -d src
grove check   src/registry.rs
```

## The MCP face (for agents)

`grove serve` runs an MCP server over stdio (newline-delimited JSON-RPC 2.0),
exposing the same six tools for every registered language. Register it with a
coding agent and the agent gains structural sight.

Project-scoped registration for Claude Code lives in [`.mcp.json`](.mcp.json):

```json
{ "mcpServers": { "grove": { "command": "…/target/release/grove", "args": ["serve"] } } }
```

A `CLAUDE.md` steering directive (see VISION §6.4.1) is what gets the agent to
actually *use* the tools rather than defaulting to grep/whole-file reads.

Tool results are JSON inside an MCP text block; tool-level failures come back as
`isError: true` with a message so the model can recover.

## Not yet (roadmap)

- **No staleness/incremental reparse** — grove parses on demand; a file watcher +
  `Tree::edit` is ahead.
- **No `map` / `grep` / `ast` tools yet** — repo-orient (`map`) and scope-aware
  `grep` are the next tools in the loop.
- **`callers` / `definition` are name-based** — no receiver-type or local-scope
  resolution (the tags `locals` query is a Tier-3 item).
- **12 languages ship a minimal profile** (core tools only); css/html/json/regex
  have no upstream `tags.scm` (they still `check`).

## Layout

```
registry/<lang>/   grammar.wasm + tags.scm + manifest.json (the registry stub)
skills/grove/      SKILL.md — the cross-harness skill (npx skills add Entelligentsia/grove)
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
```
