# Changelog

All notable changes to grove are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and grove adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added — `grove config` auto-detects local inference engines

The config TUI's provider row is now a **discovery-driven engine picker**. On
launch it concurrently probes the well-known local inference servers — Ollama
(11434), llama.cpp (8080), LM Studio (1234), vLLM (8000) — via each one's
OpenAI-compatible `/models` listing (a short deadline; dead ports return
instantly). Each is shown live (●, with its model count) or not-detected (○).

- **Running-process detection (Linux):** discovery also scans `/proc` for a
  `llama-server` / `llama serve` (the unified router) / `ollama` / `vllm` / LM
  Studio process and probes the port it is *actually* bound to (from `--port`,
  or `OLLAMA_HOST`), so an engine on a non-default port (e.g.
  `llama serve --port 8081`) is found where a fixed-port probe would miss it.

- Selecting an engine **auto-fills the endpoint URL and preloads its model list**,
  so the Model dropdown is instant for a detected engine. A custom endpoint is
  still entered by editing the URL field directly.
- The `provider` config field (Ollama / LlamaCpp) is now **derived from the chosen
  endpoint** rather than picked — it is a cosmetic label (grove speaks
  OpenAI-compat to both and never branches on it at request time), retained for
  on-disk back-compat and the trace header.
- New engine-discovery API in `grove-cst`: `discover_engines()` →
  `Vec<DiscoveredEngine>`, plus the `ENGINE_CANDIDATES` probe table.

## [0.4.0] - 2026-07-15

### Changed — mcp-llm inner harness rewritten to the `base-q4-v2-hf` reference

The opt-in mcp-llm `explore` mode's inner loop is now the **`base-q4-v2-hf`
reference combination** (interim winner in the explore-model experiments; 80.6 on
the 347-case holdout, served on llama.cpp):

- The single flat **v2 system prompt** whose output contract is **bare location
  lines** (`lang:path#symbol@line`, or `path:line` when a point has no enclosing
  symbol) — no prose, numbering, or tags.
- The reference tool vocabulary — base `Glob`/`Grep`/`Read` (Claude schemas) plus
  the six `mcp__grove__{outline,symbols,source,callers,map,definition}` tools.
- The `run_eval.py` harness discipline: single phase, ≤ 12 turns, thrash/token/
  time backstops, nudge + forced-answer (H1/H2), and retry-on-leak.

The `Steering` config field is **retained for back-compat** but no longer selects
a prompt arm (the merit/plan-first/strict arms are gone). The default CLI and the
7-tool `grove serve` structural surface are unchanged. Accordingly, the now-inert
**steering selector was removed from the `grove config` TUI**.

### Fixed

- **Trace files no longer corrupt under concurrent `grove serve` processes.** Two
  servers starting in the same second under the same MCP client derived an
  identical `session_id` (`<epoch>-<client-slug>`) and interleaved their writes
  into one file; the torn header then made `grove tap` drop the whole session
  (rendering real work as "0 calls"). The `session_id` now carries the pid
  (`<epoch>-<client-slug>-<pid>`), and the `grove tap` parser recovers a session
  from its filename when the header line is torn — so its calls still render.

### Documentation

- Overhauled the README with an updated architecture overview, an SVG flow
  diagram (replacing the ASCII art), a symbol-id syntax diagram, and a header
  favicon.

## [0.3.1] - 2026-07-05

### Added — Multi-harness `grove init` (Cursor, Codex, Gemini, Windsurf, VS Code)

`grove init` now wires grove into coding agents beyond Claude Code. A new
**harness** axis (orthogonal to the surface `--as` mode) records which agents a
project targets, persisted as a `harnesses` array in `.grove/config.json`
(defaults to `["claude-code"]`, so existing configs are unchanged).

- **`--agents <list>`** — select which agents to wire: `auto` (the default —
  detect agents in use via `PATH` + project markers like `.cursor/`), `all`, or a
  comma list of `claude-code,cursor,codex,gemini,windsurf,vscode`. Omitting the
  flag preserves a prior run's set (idempotent re-runs), else auto-detects.
- Each harness gets its **own registration file, format, and scope**: Claude Code
  `.mcp.json`, Cursor `.cursor/mcp.json`, Gemini `.gemini/settings.json`, Windsurf
  `.windsurf/mcp.json`, VS Code `.vscode/mcp.json` (note: root key `servers` +
  `"type":"stdio"`), and Codex `~/.codex/config.toml` (**TOML**, and **user-global**
  — merged with `toml_edit`, preserving other servers).
- **Steering:** Claude Code keeps its native `mcp__grove__`-prefixed `CLAUDE.md`
  block; every other agent reads a shared, harness-neutral (bare tool names)
  `AGENTS.md` block — now written for the standard `mcp`/`both` surfaces too, not
  just `mcp-llm`. (`@`-imports are unreliable across agents — notably Codex can't
  resolve them — so the block is written inline into each file.)
- De-selecting an agent strips its stale grove entry on the next `init`.
- **`grove doctor`** verifies each configured harness's registration at its own
  path/format (`harness_mcp_<agent>` checks), not just Claude Code's `.mcp.json`.
- `--dry-run` previews every target file per selected agent, flagging user-global
  ones (Codex).

## [0.3.0] - 2026-07-05

### Added — Delegated local-LLM mode (`grove serve --explore` / `--as mcp-llm`)

Grove gains a second MCP face: a **delegated local-LLM ("explore") mode**, opt-in
alongside the stable CLI and 7-tool `grove serve`. It is configured project-locally
in `.grove/config.json` and its config format and `explore` tool contract are now
covered by semantic versioning. Architecture and setup are in the README's
*Delegated local-LLM mode* section and the docs site.

- **`grove init --as mcp-llm`** — init target that provisions the delegated
  local-LLM harness: writes `.mcp.json` (a `grove serve --explore` entry), seeds
  idempotent steering blocks in `CLAUDE.md` and `AGENTS.md`, and launches an
  interactive TUI to configure the inference backend on first run.
- **Declared project mode — `.grove/config.json`** — a single, versioned source of
  truth for a project's grove integration (`mode` + an `explore` section), so
  `grove serve` and `grove init` agree on the active surface. `serve` selects
  explore mode from `config.mode`, not the presence of a config file, so switching
  modes with `grove init --as …` reliably reconciles `.mcp.json` and the steering
  blocks. Legacy `.grove/explore.json` is **migrated forward** on first load
  (mode = `mcp-llm`, with a one-time deprecation notice); `grove doctor` flags it
  until removed.
- **`grove doctor`** — a pre-flight health check (in the spirit of `brew doctor`):
  validates the registry, grammar cache, project languages, `grove.lock`, and — in
  explore mode — the config, provider reachability, and that the configured model
  is served. Reports declared-mode-vs-on-disk drift. `--json` for CI; non-zero exit
  on any hard failure.
- **`grove config`** — TUI verb to view and edit the explore config in
  `.grove/config.json` at any time. Mode-aware (read-only on `mode`; the explore
  fields are inert when the project is not in `mcp-llm` mode). Requires a TTY.
- **`grove serve --explore`** — a second serve face that exposes a single
  `mcp__grove__explore` tool instead of the 7 structural tools. A startup health
  probe (`/models`) decides the surface: **healthy** → explore-only; **unhealthy
  → transparent fallback to the 7 structural tools** (never a dead server).
  Mid-session provider loss returns a recoverable `isError` with a restart hint.
- **Inner explorer engine (`core::explore`)** — a pure-Rust agent loop: turn-bounded
  (≤ 6 turns) with a forced-final-answer at the cap; a four-tool inner set
  (`Grove` command-tool · `Read` · `Glob` · `Grep`); `<final_answer>` citation
  grounding with filesystem validation; and an OpenAI-compatible client (Ollama
  default, llama.cpp) using bench sampling (`max_completion_tokens` 1024,
  `temperature` 0; qwen `enable_thinking=false`). The `Grove` tool runs the
  structural verbs **in-process** (`ops` + `core::render`, ADR 0003) — no
  subprocess spawn or reparse per call.
- **Three steering arms** — `standard` (merit: the model picks tools freely),
  `balanced` (plan-first: recon → commit a plan → execute, plan cached per repo),
  and `strict` (grove-first steering).
- **MCP progress notifications** — long delegated calls emit per-turn
  `notifications/progress` (keyed off the caller's `_meta.progressToken`) so the
  waiting client sees liveness instead of a silent wait.
- **Locator-framed steering** — the tool description and init blocks frame
  `explore` as a *locator* (find WHERE code lives, return `file:line` citations)
  with a recommended locate → read → synthesize flow, so the calling agent
  engages grove rather than bypassing it with a broad grep subagent.
- **`tap` — structured session tracing + a browser.** A `tap` field in the
  `explore` config (toggled in the `grove config` TUI): when on,
  `grove serve --explore` records every session to a per-session JSONL trace under
  `.grove/traces/` — a `session` header (client identity from MCP `clientInfo`,
  model, mode, provider) then a `call_start` / `turn` / `call_end` stream per
  `explore` call, carrying **token usage and wall time**. **`grove tap`** enables
  tracing and opens a full-screen browser that drills session → call → turn: session
  list (agent, model, #calls, total tokens, live marker), per-call metrics, and each
  turn's request/response pretty-printed — the request tiered into a collapsed system
  prompt, collapsed prior context, and the inline prompt the turn acts on. Retention
  keeps the last `trace_retain` sessions (default 50, `0` = keep all).

### Documentation

- **Documentation site** — an mdBook docs section (built into the GitHub Pages
  site under `/docs`) covering install, the CLI, the standard and explore MCP
  modes, and using grove as a library (with the API reference on docs.rs).

## [0.2.0] - 2026-07-01

### Added

- **`grove-cst` — the engine as a standalone library crate.** grove is now a
  Cargo workspace: the tree-sitter AST engine, grammar registry, fetch, and
  ingest live in a reusable library (`core/`, published as `grove-cst`; CST for
  the concrete syntax trees tree-sitter builds) that you can embed in Rust
  directly — `use grove_core::ops` — with no subprocess and no CLI. The `grove`
  binary is now a thin `clap` + MCP shell over it (`cli/`, published as
  `grove-cst-cli`; the installed binary is still named `grove`). A curated public
  surface (`grove_core::ops`, `provision_project`, and the `Symbol`/`Defect`/
  `CallSite`/`FileMap` return types) is documented in `core/README.md`.

### Changed

- **Repo restructured into a Cargo virtual workspace** (`core/` + `cli/`). The
  CLI and MCP behaviour, the 7-tool surface, and grammar/registry handling are
  unchanged; this is a packaging split that makes the engine independently
  consumable and publishable.

### Fixed

- **Dev-tree grammar fallback after the workspace move.** `registry::dev_root`
  resolved the source-tree registry as `CARGO_MANIFEST_DIR/registry`, which became
  `core/registry` once the crate moved into `core/` — a path that does not exist.
  On a checkout with no OS cache and no `GROVE_REGISTRY` (e.g. CI), grammar
  resolution fell through to that broken path and 42 library tests failed. It now
  resolves `../registry` (the workspace root), where the dev stub actually lives.
- Release tooling made workspace- and rename-aware: `scripts/bump-version.sh` and
  `.github/workflows/release.yml` now target the `grove-cst` / `grove-cst-cli`
  package ids; both crates ship a `LICENSE` and a package-local `README`.

## [0.1.11] - 2026-06-27

### Changed

- **Route-by-task steering** (`grove init` CLAUDE.md block and the `grove`
  skill's `SKILL.md`). Both surfaces previously framed code navigation as an
  "INVARIANT — grove or it's a steering violation", relegating
  `grep`/`rg`/`read`/`cat`/`sed` to fallbacks allowed only after grove was
  tried. With grove's current 7-tool surface (no text-search tool yet), that
  over-rigid framing pushed the model into costly one-symbol-at-a-time `source`
  fan-outs for things the shell does cheaply, and gave no guidance for
  text/non-code/quick-fact work or for combining grove with the shell. The
  steering now **routes by task**: grove for named symbols and structural
  relationships (where-defined, who-calls, what's-in-a-file, how-a-dir-connects,
  post-edit check), the shell for text / non-code files / quick facts ("the
  right tool, not a fallback"), and an explicit **combine** path (grep a
  literal's line → `definition --at`; `outline` → bounded read; `map`/`symbols`
  → grep a constant inside). The useful procedure (outline→source chains, stable
  symbol-ids, `map` breadth control, the shape-slice, profile gate, recovery,
  setup) is preserved.

### Notes

- Validated in the `nav-3way` testbench: on `L4-grove-redis` the reworked
  steering cut context ~41% (560k → 329k tokens) with no loss of answer
  completeness — the model combined 20 grove calls with 3 greps + 2 bounded
  reads instead of 36 `source` calls.

## [0.1.10] - 2026-06-27

### Fixed

- **Supertype-guard no longer false-positives on `/` in query comments.** The
  guard added in 0.1.9 (which disables `locals.scm`/`imports.scm` using
  crash-prone tree-sitter supertype syntax `(a/b)`) skipped string literals but
  not `;` comments, so a comment like `; if/else` or `; try/catch` wrongly
  disabled an otherwise-valid query. It now tracks `;`-to-end-of-line comments.
  This unblocks scope-aware resolution for hosted `locals.scm` whose comments
  contain `/` (e.g. java, c, cpp).

### Notes

- With this fix, the hosted registry's newly added `locals.scm` for **python,
  go, java, c#, c, cpp, rust** take effect (`grove fetch` + `definition --at`),
  bringing scope-aware go-to-def to 11 languages total (with the existing ruby,
  scala, julia, javascript). Each was verified to resolve a shadowed local
  line-exact against the pinned grammar.

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
