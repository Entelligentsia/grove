# ADR 0004 — Split the explore delegate out of grove core into `grove-explore`

- **Status:** Accepted
- **Date:** 2026-07-20
- **Deciders:** Boni Gopalan
- **Supersedes:** — (amends ADR 0002 §1: `serve` no longer selects a surface
  from the declared mode, `determine_surface` is deleted; and §3: the config
  TUI's mode-gated inert rendering is moot once no surface switches on mode)
- **Related:** `cli/src/mcp.rs` (`Surface`, `determine_surface`),
  `core/src/explore/` (the entire subsystem), `cli/src/config_tui/`,
  `cli/src/trace_tui/`, `cli/src/tap.rs`, ADR 0002 (declared mode),
  VISION §6.4.1 (availability ≠ adoption), `grove-explore-model` (research
  repo), `grove-models` (model release facade),
  `is-grep-enough/studies/fastcontext-sidebench/` (eval bed)

## Context

`grove serve` is bimodal. At startup, `determine_surface` (`mcp.rs`) resolves
the declared mode plus `--explore`/`--standard` overrides plus a provider
health probe into a `Surface` enum, and the server then presents **either** the
standard 7-tool structural surface **or** a single delegating `explore` tool
backed by a local LLM. One server name, two unrelated products behind it, and
never both at once.

The explore subsystem this switch guards is not small, and it is not
structural code intelligence:

| Piece | Where | Size | What it actually is |
|---|---|---|---|
| Inner explorer engine | `core/src/explore/` (11 modules + embedded prompt) | ~3,900 lines | LLM agent loop (`agent.rs`, a Rust port of the eval bench's `run_question`), OpenAI chat wire model, HTTP client, health probe, local-engine discovery (`/proc` scanning), prompt steering, grounding/leak-retry, JSONL trace persistence |
| Config + trace TUIs, `tap` verb | `cli/src/config_tui/`, `cli/src/trace_tui/`, `cli/src/tap.rs` | ~2,950 lines | Full-screen ratatui apps for managing the explore backend and browsing its traces |
| Radiating coupling | `config.rs` (`Mode::McpLlm`, legacy `explore.json` migration), `doctor.rs` (explore check group), `init.rs` (first-run TUI, `--as mcp-llm`), `mcp.rs` (mode resolution, health fallback) | — | Mode plumbing that exists only because two products share one server identity |

For scale: the structural engine proper (`ops`/`engine`/`registry`/`fetch`/
`ingest`/…) is ~6,400 lines. The explore layer plus its TUIs is roughly the
size of the product it is embedded in.

Five forces make this a problem rather than a footnote:

1. **`grove-cst` is a published library with a false advertisement.** Its
   crates.io description reads "structural code-intelligence library"; VISION.md
   describes the structural-access product and does not mention explore mode at
   all. Yet every library consumer compiles in an LLM orchestration harness, an
   HTTP-to-LLM client, embedded prompts, and trace persistence — none of it
   feature-gated. `toolset.rs` even re-implements Glob/Grep/Read with
   Claude-style schemas, tools with no relation to grove's domain.

2. **The explore product's lifecycle already lives elsewhere.** Its research is
   in `grove-explore-model`, its weights are on HuggingFace/ollama fronted by
   `grove-models`, its eval bed is the fastcontext sidebench in
   `is-grep-enough`. Three of its four lifecycle artifacts are outside this
   repo; the runtime harness is the straggler — and it is the piece with the
   tightest release coupling. Every prompt revision (the system prompt is
   `include_str!`-embedded), harness-discipline fix, or default-model change
   requires the full grove release train: version bump, tag, 5-platform build,
   npm publish, brew formula regen.

3. **The either/or `Surface` is a product limitation, not just a code smell.**
   A project gets *either* the structural tools *or* the locator — never both.
   The arguably best configuration (delegate broad "where is X" sweeps to
   `explore`, use `map`/`source` directly for precision work) is unreachable.

4. **The seam is being fought, and it has already produced bugs.** Bug-1
   (GROVE-S03-T03, documented in `determine_surface`'s own doc comment): a
   stale `explore.json` booted the explore surface against the declared mode.
   The silent health-probe fallback is a standing latent instance of the same
   class: the client's `CLAUDE.md` steering says "use `mcp__grove__explore`"
   while the server quietly served the 7-tool surface because the local model
   was down. Every one of these is downstream of a single server identity whose
   surface depends on local config *and* the health of a background daemon.

5. **Counterweights, honestly stated.** (a) The inner explorer calls grove ops
   as direct in-process Rust calls — no MCP hop — and the delegate's economics
   (a light model making many cheap tool calls) depend on that staying true.
   (b) The runtime is currently *frozen*: the shipped harness is the
   `base-q4-v2-hf` reference combination, and the active research thread
   (quantization floor) does not touch it. The release-cadence pain is
   therefore prospective, not acute — which argues for cutting the seam
   cheaply now, not for standing up a second release train today.

## Decision

Separate the explore delegate into its own product surface — **`grove-explore`**
— with `grove-cst` as the shared library underneath, and make the two MCP
surfaces **composable instead of exclusive**. Executed in three stages; stages
1–2 now, stage 3 gated on an explicit trigger.

```
grove-cst (library)              pure structural engine; zero LLM knowledge
    ├── grove (binary)           CLI + 7-tool MCP server; no modes, no fallback
    └── grove-explore (binary)   the locator product: agent loop, prompts,
                                 config/trace TUIs, tap; its OWN MCP server
                                 identity; links grove-cst for in-process ops
```

Users compose in `.mcp.json`: structural only, explore only, or both. The name
`grove-explore` keeps family branding and aligns with the already-released
model line (`grove-explore-base` in `grove-models`).

### Stage 1 — crate split (now)

Move `core/src/explore/` into a new workspace crate (`explore/`, published as
`grove-explore-core` if/when publishing is warranted). `grove-cst` returns to
being what its description claims. The boundary is already clean — `mod.rs`
exports only `run_explore[_reporting]`, config, and health types — so this is
mechanical. `ExploreConfig` moves with it; `core/src/config.rs` keeps
`GroveConfig` but holds the explore section as an opaque-to-core type owned by
the new crate (or re-exported from it), so `grove-cst` carries no LLM types.

### Stage 2 — surface split (now)

- Add a `grove-explore` bin target serving **only** the `explore` tool under
  its own MCP server name. An unhealthy provider at startup is a startup
  *error* with an actionable message — never a silent morph into a different
  product. Mid-session provider loss keeps today's recoverable `isError`.
- `grove serve` always serves the 7-tool structural surface. Delete `Surface`,
  `determine_surface`, the `--explore`/`--standard` flags, and the health
  fallback; `mcp.rs` loses all `explore::` imports.
- **Amendment to ADR 0002 §1:** the declared `mode` no longer selects a serve
  surface (there is nothing left to select). `mode` remains the
  harness-reconciliation key for `init`/`doctor`: `mcp-llm` now means "register
  *both* servers in `.mcp.json` and write the delegation steering." This keeps
  ADR 0002's single-source-of-truth and `reconcile_harness` design intact —
  only the runtime consumer of `mode` disappears.
- **TUI surfaces.** Both TUIs are pure explore-product UIs — `config_tui`
  imports only `ExploreConfig`/`Provider`/`Steering`, engine discovery, and
  the `/models` fetch; `trace_tui`/`tap` only the trace helpers and the `tap`
  flag; none reference `ops`/`engine`/`registry` — so they move wholesale:
  `grove config` → `grove-explore config`, `grove tap` → `grove-explore tap`,
  with the old `grove` verbs retained for one release as forwarding shims that
  print the new spelling. ratatui/crossterm leave the `grove` binary entirely
  (they are its only TUI dependencies). User-facing strings are swept in the
  same change: `tap`'s "restart `grove serve`" hint becomes "restart the
  grove-explore server", and its error path stops pointing at the
  ADR-0002-deprecated `.grove/explore.json`. ADR 0002 §3's planned
  inert/greyed rendering when `mode != mcp-llm` becomes moot — with no
  mode-switched surface, the explore config is always live for the explore
  server; any badge should reflect whether the `grove-explore` server is
  registered, not the mode.
- **Config ownership (interim).** Through stages 1–2, `config_tui` keeps its
  read-modify-write of `.grove/config.json` — a grove-owned file — preserving
  `mode` and `harnesses` and rewriting only the explore section, exactly as it
  does today. grove-explore is a deliberate *guest writer* in that file until
  stage 3 moves the config out.
- `grove init --as mcp-llm` remains the adoption funnel (VISION §6.4.1 applies
  to the new product too): it registers both servers and writes steering that
  names both surfaces and when to use each. The first-run config TUI is the
  one cross-binary seam: `init` no longer contains the TUI, so it **shells out
  to `grove-explore config`**. In the interim single-repo form both binaries
  ship together, so the sibling is normally present; when it is not on PATH,
  `init` must degrade gracefully — register the server anyway and print "run
  `grove-explore config` to finish setup" rather than failing the init.
- `doctor`'s explore check group moves behind the same boundary: the checks
  run when the harness registers the `grove-explore` server, not when
  `mode == mcp-llm` implies a surface.

### Stage 3 — repo/release split (gated)

Extract `grove-explore` to its own repository, versioning, and distribution
(sibling brew formula / npm package) **when** its release cadence actually
diverges from grove's. Concrete triggers, either sufficient:

- the sid-fork model track graduates from research (its output contract —
  symbol-id `<final_answer>` — forces harness changes), or
- prompt/harness iteration resumes at a cadence where coupled grove releases
  demonstrably delay explore improvements (more than one explore-only release
  forced onto the grove train in a quarter).

Config moves with the product at this stage (own file, with a migration read
of the `.grove/config.json` `explore` section). Until then — stages 1–2 —
the explore section stays where ADR 0002 put it, to avoid user-visible config
churn twice.

## Scope boundary — what this deliberately does NOT do

- **No inner-loop behavior change.** The reference harness (flat v2 prompt,
  bare location lines, H1/H2 backstops, retry-on-leak) ships byte-identical.
  This is a boundary change, not a quality change; the sidebench numbers must
  not move.
- **No renames of user-visible contracts.** The tool is still `explore`; the
  symbol-id and location-line formats are untouched; model branding stays
  `grove-explore-*`.
- **Stage 3 is not scheduled.** It is prepared (the crate boundary makes it
  mechanical) and gated on the triggers above — not started speculatively
  while the harness is frozen.
- **No new tunables or modes.** `Mode::LEGAL` is unchanged; `mcp-llm` changes
  meaning (register-both) but no mode is added or removed in this ADR.

## Alternatives considered

- **Status quo — mode-switched single server.** Rejected: it is the direct
  cause of the exclusivity limitation, the bug-1/health-fallback class, and
  the library-purity violation. Every mitigation (more doctor checks, more
  migration code) grows the mode plumbing that only exists to defend the
  shared identity.
- **Cargo feature flag (`explore`) on `grove-cst`.** Fixes library consumers
  only. Release-cadence coupling, surface exclusivity, and the bimodal server
  identity all survive. Rejected as a half-measure that spends the migration
  budget without buying the composition win.
- **Expose `explore` as an 8th tool on the standard surface.** Solves
  exclusivity cheaply, and was the strongest rival. Rejected because the
  surface would then vary with the health of a background daemon (the same
  steering-vs-reality mismatch in new clothes), the LLM harness stays inside
  the published library, and the release trains stay coupled. It also welds
  the product identities together permanently — the opposite of preparing
  stage 3.
- **Immediate full repo split (skip the gate).** Rejected as speculative
  overhead: the runtime is frozen on the reference combination, so a second
  repo, CI, and distribution channel would today ship zero-delta releases.
  Stages 1–2 capture all current pain; stage 3 is one `git mv` of a crate
  when the trigger fires.
- **`grove-explore` shells out to the grove binary (or its MCP server) instead
  of linking `grove-cst`.** Rejected: it taxes the delegate's core economics —
  many cheap in-process structural calls per question — with subprocess or
  protocol overhead, and adds a runtime version-skew surface between two
  binaries. Library linkage pins the version in `Cargo.lock`.

## Consequences

**Positive**

- `mcp.rs` shrinks to the 7-tool server: no `Surface`, no mode resolution, no
  health fallback, no explore imports. The steering-vs-served-surface mismatch
  class is structurally gone — each server's surface is a constant.
- Structural tools and the locator compose. A session can register both and
  use each for what it is best at; `init --as mcp-llm` writes exactly that.
- `grove-cst` is honest again: a structural-parsing library consumers can take
  without an LLM harness. The explore crate becomes independently testable
  against the sidbench contract.
- The explore product gets an identity that matches its already-external
  lifecycle (research repo, models repo, eval bed), and stage 3 becomes a
  mechanical extraction rather than surgery.
- Provider-down is a visible startup failure of the explore server with the
  structural server unaffected — strictly better than today's silent morph.

**Negative / costs**

- Two `.mcp.json` registrations to write, reconcile, and doctor-check where
  there was one; `reconcile_harness`'s mode→files matrix grows a column.
- A deprecation window: `serve --explore/--standard` flags, the `grove
  config`/`grove tap` spellings, and the old single-registration `mcp-llm`
  projects all need one release of shims/migration plus clear errors after.
- Interim single-repo form still ships the TUIs in the workspace (binary size
  of `grove` itself *drops* — ratatui/crossterm move to `grove-explore` — but
  total install size for both-server users is unchanged).
- Cross-crate versioning discipline: `grove-explore` pins `grove-cst` exactly
  (as `cli` does today); a structural-surface change now touches two crates'
  release notes.
- Until stage 3, `grove-explore` remains a guest writer in grove-owned
  `.grove/config.json` (the TUI's read-modify-write of the explore section),
  and `grove init --as mcp-llm` depends on the `grove-explore` binary being on
  PATH for its first-run TUI — both are accepted interim couplings with a
  defined end state, but each is a spot where the two products can skew if the
  degrade paths are not tested.
- Docs restructure: README's delegated-mode section, `docs/setup.md`,
  `docs/mcp.md`, and the init-written steering templates all change to the
  two-server story.

## Status of implementation

- **Stages 1–2: implemented.** GROVE-S04-T01 through T08 are committed:
  crate split (`explore/`, published as `grove-explore-core`); the
  `grove-explore` binary with its own MCP server identity and a hard startup
  health gate (config load → explore deser → health_probe, each
  `process::exit(1)` before the serve loop — never a fallback); `mcp.rs`'s
  `Surface`/`determine_surface`/health-fallback machinery deleted, `grove
  serve` unconditionally structural; `reconcile_harness`'s both-servers column
  (`mcp-llm` registers `grove` + `grove-explore`) plus forward migration;
  config/trace TUIs and `tap` moved to `grove-explore` with `grove
  config`/`grove tap` retained as deprecated forwarding shims; `grove init
  --as mcp-llm`'s PATH-aware shell-out to `grove-explore config` with graceful
  degrade when the sibling binary is absent; `doctor`'s explore checks re-keyed
  on `grove-explore` registration rather than declared mode; packaging
  (release workflow, npm, Homebrew, install.sh) ships both binaries.
- **Gate test (a)** — the full sidebench reference-number re-run on the split
  binary — is **GROVE-S04-T10**, a user-supervised rig, and remains **open**.
- **Gate test (b)** — the extended transition-matrix test, asserting every
  `A → B` mode switch leaves both server registrations, steering, and served
  surfaces mutually consistent — **shipped with GROVE-S04-T04**.
- **Stage 3** (repo/release split) remains **not scheduled**, gated on the
  triggers stated above (sid-fork model-track graduation, or explore-only
  release cadence divergence).
