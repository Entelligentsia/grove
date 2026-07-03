# CODE_REVIEW — GROVE-S02-T04 (standalone review)

**Verdict:** Approved

Health-gated explore mode for `grove serve`. Every acceptance criterion was
verified against actual source (not the PROGRESS report), and all build gates
were run independently.

## Independent verification

- **API contracts** (grove-navigated, not assumed):
  - `run_explore(question:&str, root:&Path, cfg:&ExploreConfig, client:&dyn ChatClient) -> Result<ExploreAnswer, ExploreError>` — matches call site `run_explore(&question, root, cfg, &client)`.
  - `ExploreError` has exactly `ProviderDown{url,detail}` and `Client(String)` — the `call_explore_tool` match is exhaustive with no catch-all.
  - `ExploreAnswer.text`, `ExploreConfig::config_path`/`::load`, pub `base_url`/`model`, `OpenAiCompatClient::new(&ExploreConfig)`, `health_probe(&ExploreConfig)->Result<(),HealthError>` (Unreachable | ModelMissing) — all confirmed present and correctly used.
- **Build gates** (run, not trusted):
  - `cargo clippy --all-targets -- -D warnings` → clean.
  - `cargo test --release --locked` → 178 pass (126 core + 32 cli-unit + 19 integration + 1 doc), 0 fail. Matches PROGRESS claim.
  - `grep -rin fastcontext cli/src core/src` → none (AC-7).

## AC-by-AC

1. **Mode selection (AC-1)** — `determine_surface` precedence `force_standard` > (`force_explore` ∥ config-exists) > `health_probe`. Clap flags `--explore`/`--standard` with `path` default `.` follow existing conventions. mcp.rs stays thin; `core::explore` executes. ✓
2. **Startup gate (AC-2)** — probe runs once before the request loop. Healthy → single `explore` tool, plain object schema `{properties:{question:string}, required:["question"]}`, no top-level anyOf/oneOf; `explore_instructions` is delegation-oriented. Unhealthy (Unreachable/ModelMissing) & load-failure → Standard 7-tool surface + one-line stderr note; `initialize` always answered; stdout protocol-only. ✓
3. **Explore dispatch (AC-3)** — real `OpenAiCompatClient`; bad-args and all `ExploreError` paths return `isError:true` results, never JSON-RPC errors. ✓
4. **Mid-session loss (AC-4)** — `ProviderDown` → `isError` with actionable message (endpoint / `grove config` / restart). No crash/exit. ✓
5. **Default byte-identical (AC-5)** — Standard branch reuses `tool_specs()`/`instructions()`/`call_tool()` verbatim. Existing unit tests only threaded the unavoidable `&Surface::Standard` third arg; their assertions (7 tools, symbol-id instructions) are unchanged, preserving behavioral identity. ✓
6. **Integration test (AC-6)** — `explore_mode_unhealthy_provider_falls_back_to_standard_surface`: port-1 unreachable → 7 tools, "falling back" stderr, initialize OK. Deterministic, no provider. Passing. ✓
7. **Hygiene (AC-7)** — clippy/warning clean, tests green, no "fastcontext". ✓

## Advisory notes (non-blocking)

1. **Deterministic branches lack unit guards.** The two plan-review advisories were
   not adopted: (a) a unit test that `call_explore_tool` with a missing `question`
   returns `isError:true` (unit-testable without a provider), and (b) an assertion
   that `explore_tool_spec()` has no top-level `anyOf`/`oneOf` (locks the AC-2 schema
   shape). Both are currently only exercised transitively. Cheap regression guards to
   add in a future touch — not required for this sprint-final, additive change.
2. **Explore instructions untested.** `explore_instructions` (model/base_url interpolation)
   has no direct test; only Standard instructions are asserted. Low risk.
