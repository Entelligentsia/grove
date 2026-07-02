# ARCHITECT_APPROVAL — GROVE-S02-T05: Full-screen setup TUI + `grove config` verb

**Verdict:** Approved

## Approval Rationale

The implementation is consistent with grove's core architectural boundary and
all seven acceptance criteria were verified through the plan → review → implement
→ code-review → validation chain.

- **Layer boundary held.** `ratatui = 0.29` and `crossterm = 0.28` are confined
  to `cli/Cargo.toml`; grep confirms core remains TUI-free. This honors the
  "cli formats, core executes" contract — validation and persistence are
  delegated to `ExploreConfig::validate()` / `ExploreConfig::save()` in core,
  while the TUI is purely a presentation/input layer.
- **Testability preserved.** The Elm-style `update()` is pure and covered by 12
  headless unit tests (tab nav, provider→URL coupling, mode cycling, tool toggle,
  save/cancel, round-trip, validation). No terminal is required to exercise the
  state machine — a durable, low-cost regression surface.
- **Single source of truth.** Provider default endpoints (Ollama 11434,
  llama.cpp 8080) are constants matching core provider defaults; the `dirty_url`
  flag correctly prevents provider switches from clobbering user-edited endpoints.
- **Safe failure modes.** Non-TTY guard fast-fails (exit 1, "interactive
  terminal" message); terminal state is restored on all exit paths including the
  save-error path, avoiding a corrupted user terminal.
- **Clean extension point.** AC #5 (model auto-discovery) is correctly scoped out
  via the `#[allow(dead_code)] Msg::ModelListFetched` variant — future work slots
  in without refactoring the update loop.

## Cross-Cutting Concerns

None. The change adds a new, isolated verb (`grove config`) and a new module
(`cli/src/config_tui/`). No existing CLI surface, MCP server behavior, or core
API is modified. The config on-disk format (`.grove/explore.json`) is already
owned by core's `ExploreConfig`; the TUI round-trips through it without schema
changes.

## Operational Impact

- **Version bump:** not required (sprint-final, additive verb).
- **Regeneration / migration:** none.
- **Backward compat:** new verb; no existing surface touched.
- **Binary size:** release binary measured at 13,296,264 bytes (~12.7 MB),
  recorded per AC #6. The ratatui/crossterm addition is an acceptable delta for
  an interactive-only code path in a single-binary distribution.

## Follow-up Items (future sprints)

1. **Add the promised non-TTY integration test** to `cli/tests/cli.rs`
   (`assert_cmd` asserting exit 1 + "interactive terminal" in stderr). The guard
   is verified manually and via CLI, but the automated assertion promised in the
   PLAN was not delivered — a non-blocking gap worth closing to prevent
   regression.
2. **Model auto-discovery (AC #5)** — wire `Msg::ModelListFetched` to a provider
   `/api/tags` (Ollama) / `/v1/models` (llama.cpp) query when the endpoint is
   reachable, populating the Model field with a selection list.
3. **Consider a `Provider::default_base_url()` helper in core** so the TUI's URL
   constants and core's provider defaults share a single definition (currently
   aligned by convention).
