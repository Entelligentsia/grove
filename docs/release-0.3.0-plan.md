# Release plan: grove 0.3.0 — graduate `mcp-llm`, ship the docs site

**Status:** planned (not started)
**Date:** 2026-07-05
**Owner:** Boni Gopalan
**Related:** [`RELEASING.md`](../RELEASING.md), [ADR 0002](adr/0002-grove-project-config-and-declared-mode.md)
(declared-mode config, implemented), [doctor proposal](doctor-command-proposal.md)
(implemented), the 2026-07-03 code-quality review.

## Situation

- **`0.3.0` is bumped in every manifest but never tagged.** The last released
  tag is `v0.2.0`; `CHANGELOG.md` still reads `[Unreleased]`. This cuts the first
  0.3.0 release.
- **The S03 hardening has already landed on `main`.** `GroveConfig` / `Mode` /
  `active_mode` (`core/src/config.rs`), `serve` no longer sniffing
  `.grove/explore.json` (`cli/src/mcp.rs:48-50`), the mode-aware config TUI, and
  `grove doctor` (`cli/src/main.rs:312`) are all implemented. The `aggressive →
  strict` steering rename landed (`de487aa`). ADR 0002 and the doctor proposal are
  now the design records for shipped code.
- **One "revisit before stabilize" item remains:** the explore toolset shells out
  to grove's own binary per tool call (`core/src/explore/toolset.rs:438,462`,
  `current_exe()` + `Command::output()`) instead of calling `ops` in-process. The
  code-quality review flagged this as acceptable while experimental but to be
  fixed before `mcp-llm` stabilizes.
- **The site is a hand-rolled static marketing page** (`site/index.html` + `app.js`
  + `styles.css`), deployed verbatim by `.github/workflows/pages.yml` (uploads
  `./site`, no build step). It has no documentation section.

## What "out of experimental" commits us to

Graduating `mcp-llm` is a **stability promise under semver**, not just deleting a
⚠️ badge. Frozen surfaces: the `.grove/config.json` shape (incl. the `explore`
section), the `explore` MCP tool contract, and the auto-fallback behaviour. A
0.3.x patch may not break these. The config format is already versioned
(`"version": 1`) with a migration path, so it is ready to freeze.

## Decisions locked

1. **Toolset refactor gates graduation.** Do the in-process `ops` refactor
   *before* tagging, so 0.3.0's graduated surface carries no known perf debt.
   (Options weighed: fast-follow in 0.3.1, or defer graduation to 0.4.0.)
2. **Docs use mdBook.** Rust-native generator; sidebar + search + theming for
   free; source stays plain markdown reused from `README` / `docs/setup.md` /
   ADRs. One build step added to `pages.yml`. (Options weighed: hand-rolled
   static, or a JS docs framework.)
3. **Library reference comes from docs.rs.** The release already auto-publishes
   both crates to crates.io (`publish-crates` job, RELEASING.md step 5), so
   `docs.rs/grove-cst` hosts the API reference the moment 0.3.0 publishes. The
   docs site links to it rather than re-documenting the library API.

## Four product surfaces → documentation

| Surface | Docs source | Net-new work |
|---|---|---|
| **Library** (`grove-cst` / `use grove_core`) | docs.rs/grove-cst (auto on publish) + a short "use as a library" quickstart | low |
| **CLI** (`grove <verb>`) | verb reference — content in `README.md` + `CLAUDE.md` command surface | organize existing |
| **Standard MCP** (7-tool `grove serve`) | tool contract, symbol-id format, `init --as mcp` | organize existing |
| **Explore MCP** (`mcp-llm`) | `config.json`, `doctor`, steering, fallback — content in `docs/setup.md` + README | organize existing |

Most content already exists as markdown; the docs section is mostly information
architecture + rendering + navigation, not net-new writing.

## Phases

### Phase 0 — In-process explore toolset (graduation gate)

- **Crux:** the toolset lives in `core` but shells out to capture the CLI's *text*
  formatting of 6 verbs (`outline, symbols, source, callers, definition, map`),
  which today lives in `cli/src/main.rs` match arms — and `core` cannot depend on
  `cli`. Going in-process requires **relocating the per-verb text rendering into
  `core`** so both faces call it.
- Extract that rendering into a `core` module (e.g. `core::format`, or
  typed→string helpers on `ops`) returning exactly the strings the CLI prints
  today. Rewire `cli/src/main.rs` to call it (no behaviour change). Replace
  `toolset.rs`'s `run_capture(grove_bin, parts)` with an in-process
  `render_grove_verb(verb, args, root) -> String`.
- Keep the verb allow-list (`ALLOWED_VERBS` / `RECON_VERBS`) and the
  workspace-relative path sandbox; drop the `GROVE_BIN` / `current_exe()`
  subprocess path.
- **ADR 0003** records the `cli → core` formatting relocation (an altitude change
  to the "faces only format" rule).
- **Parity test:** in-process output byte-equals the prior CLI output for
  representative calls; the existing explore agent tests (`DownClient`, fallback)
  stay green.

### Phase 1 — Freeze the contract & de-experimentalize

- Rewrite the `CHANGELOG.md` `[Unreleased]` block → `## [0.3.0] - 2026-07-05`:
  drop the "may change without a major bump" caveat; fold in the S03 additions
  (declared-mode `.grove/config.json`, `grove doctor`, migration, `strict`
  rename, in-process toolset).
- Sweep the ⚠️ experimental markers: `README.md:128`, `docs/setup.md:36`,
  `CLAUDE.md:33` + `:45`, and the `Target::McpLlm` doc comment in
  `cli/src/init.rs`. Reframe `mcp-llm` as a supported opt-in mode.
- Confirm the `explore.json → config.json` migration is covered by a test (the
  one-way door for existing users).

### Phase D — Documentation site (mdBook) — parallel; lands with the tag

- New mdBook at `docs/` (`book.toml`, `src/SUMMARY.md`) sourcing existing
  markdown. Chapters:
  - Getting started / install
  - CLI reference (`grove <verb>`)
  - MCP: standard 7-tool server
  - MCP: explore (`mcp-llm`) — `config.json`, `doctor`, steering, fallback
  - Use as a library (quickstart + link to docs.rs/grove-cst)
  - Registry & grammars
  - Reference (symbol-id format, config schema)
  - Link out to the ADRs and `CHANGELOG`.
- `pages.yml`: add a step to build mdBook into `site/docs/` before the existing
  `./site` upload; add `site/docs/` to `.gitignore`. Optionally add an `mdbook
  build` link-check to `ci.yml`.
- `index.html`: add a **Docs** nav link → `/docs/`. Marketing landing otherwise
  unchanged.
- The docs' library link resolves same-day as the tag (docs.rs goes live when
  `publish-crates` runs). Not a hard tag-blocker.

### Phase 2 — Release mechanics (`RELEASING.md`)

Branch `release/v0.3.0` → `scripts/bump-version.sh 0.3.0` (effectively CHANGELOG
date + `Cargo.lock` refresh; versions already 0.3.0) → `cargo build --release
--locked` → `cargo test` → clippy gate (`cargo clippy --all-targets --workspace
--locked -- -D warnings`) → PR to `main` → merge (triggers `pages.yml`, docs
deploy) → push tag `v0.3.0` (triggers `release.yml`: 5 platform binaries +
`publish-crates` → crates.io → docs.rs) → `npm publish` from `dist/npm/` →
`dist/homebrew/update-formula.sh v0.3.0`.

### Phase 3 — Post-release smoke

Published-binary check (`grove init --as mcp-llm`, `grove doctor`, a mode switch
`mcp-llm → mcp` confirming no drift), brew + npm installs, docs.rs page renders,
`/docs` site live.

## Critical path & sequencing

```
Phase 0 (code + ADR 0003) ─┐
                           ├─→ Phase 2 (tag v0.3.0) ─→ Phase 3 (smoke)
Phase 1 (labels/CHANGELOG)─┘
Phase D (mdBook) ───── parallel ───── deploys on release-PR merge
```

Phase 0 is the bulk of the work and carries the one architectural decision
(formatting → `core`, ADR 0003). Recommended order: draft ADR 0003 → Phase 0 →
Phase 1 → Phase 2; Phase D proceeds independently and lands automatically when the
release PR merges.

## Risks & notes

- **Formatting relocation scope.** Phase 0 is "an afternoon" only if the 6 verbs'
  formatting extracts cleanly; if the CLI match arms are tangled with clap/table
  glue, budget more. ADR 0003 should settle the target module shape first.
- **Contract freeze is real.** After 0.3.0, `config.json` and the `explore` tool
  contract are semver-frozen. Do a final review of both before tagging.
- **Version bump is a near-noop.** Manifests are already `0.3.0`; the "bump" is
  mostly the CHANGELOG date and lockfile refresh. Do not skip the lockfile
  refresh — `--locked` builds will fail otherwise.
- **Docs and the tag are coupled but not blocking.** Library reference (docs.rs)
  only exists after `publish-crates`; the docs site can deploy earlier, its
  library link resolving once the tag's publish job completes.
