# PROGRESS — GROVE-S04-T02
## grove-explore binary — own MCP server identity, explore tool only, startup health gate

## Summary of Changes

Implemented the `grove-explore` binary: a dedicated stdio MCP server that exposes exactly
one tool (`explore`), carries its own server identity (`grove-explore`), and enforces a
hard startup health gate — the server never enters the JSON-RPC loop if provider config
loading, explore-section deserialization, or the `health_probe` fails.

### Files Changed

| File | Action | Description |
|------|--------|-------------|
| `cli/src/bin/grove_explore.rs` | **Created** | ~260 LOC — self-contained MCP server binary: clap args, config load gate, health gate, stdio loop, explore-only dispatch |
| `cli/Cargo.toml` | Modified | Added `[[bin]] name = "grove-explore" path = "src/bin/grove_explore.rs"` |
| `cli/tests/cli.rs` | Modified | Added two integration tests: smoke test and startup-failure test |

### Design Decisions

- **No `Surface` enum**: the binary is always explore mode; the enum is unnecessary overhead.
- **Mode gate intentionally omitted**: `grove-explore` is always in explore mode by design
  (separate binary expresses this better than a flag check would).
- **Hard startup gate**: each of the three startup steps (config load, explore deser, health
  probe) calls `process::exit(1)` on failure with a clear stderr hint. No fallback.
- **Code duplication vs. T03**: ~130 lines from `mcp.rs` are duplicated here, intentionally.
  T03 will factor shared logic into a module. Order: T02→T03 tight per plan advisory.
- **Trace support preserved**: `TraceWriter` opened on `initialize` when `cfg.tap` is set,
  identical to the existing explore path in `grove serve`.
- **Mid-session provider loss**: `ExploreError::ProviderDown` → `isError:true` tool result
  (AC3), identical to `call_explore_tool` in `mcp.rs`.

## Test Evidence

```
$ cargo test --release --locked
...
     Running unittests src/bin/grove_explore.rs (target/release/deps/grove_explore-e69295eae1d9ac66)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/cli.rs (target/release/deps/cli-ee67d28fdd7533ad)
running 35 tests
test grove_explore_startup_fails_on_unhealthy_provider ... ok
test grove_explore_single_tool_surface_and_identity ... ok
... (33 other tests)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

   Total: 301 passed; 0 failed (all crates)
```

```
$ cargo clippy --all-targets --workspace --locked -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s
```

Zero errors, zero warnings.

## Acceptance Criteria

| AC | Status |
|----|--------|
| AC1 — `grove-explore` binary, stdio MCP, single `explore` tool | ✅ `[[bin]]` added; smoke test asserts single tool + identity |
| AC2 — unhealthy provider is startup error, server never starts | ✅ Hard-fail health gate; startup-failure test asserts non-zero exit + empty stdout |
| AC3 — mid-session provider loss returns `isError: true` | ✅ `call_explore_tool` ProviderDown path preserved, identical to `mcp.rs` |
| AC4 — inner explorer via direct in-process Rust calls | ✅ `run_explore_reporting` called directly in `call_explore_tool` |
| AC5 — config from `.grove/config.json` explore section | ✅ `GroveConfig::load` + `grove_cfg.explore` deserialization |
| AC6 — MCP smoke test + startup-failure test | ✅ Both integration tests pass in `cli/tests/cli.rs` |
| AC7 — workspace green | ✅ `cargo test --release --locked` + clippy clean |

## Files Changed Manifest

- `cli/src/bin/grove_explore.rs` (new)
- `cli/Cargo.toml`
- `cli/tests/cli.rs`
