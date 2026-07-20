# PROGRESS — GROVE-S04-T06: `init --as mcp-llm` funnel — shell-out to `grove-explore config` + graceful PATH degrade

## Summary

Revision fix for the validation finding on the degrade-path integration test. The original implementation correctly wired `grove init --as mcp-llm` to probe PATH for `grove-explore`, shell out when possible, fail fast on PATH+non-TTY, and degrade gracefully when the sibling binary is absent. However, the `init_first_run_grove_explore_absent_degrades` test inherited the parent PATH, so it could accidentally find a globally-installed `grove-explore` and not exercise the degrade path.

This revision replaces the helper that prepended an extra directory to PATH with one that sets the child's PATH to an exact, caller-controlled value. The degrade test now creates an empty temporary directory and passes it as the child's entire PATH, deterministically excluding `grove-explore`. The non-TTY fail-fast test passes only the release `target/` directory containing `grove-explore`, so the sibling is deterministically on PATH.

## Changes made

- `cli/tests/cli.rs`:
  - Renamed `grove_mcp_llm_with_extra_path` to `grove_mcp_llm_with_path` and changed its semantics: when a path is supplied, the child process sees **only** that directory on `PATH`; when `None` is supplied, it inherits the parent `PATH` unchanged.
  - Updated `grove_mcp_llm` to delegate to the renamed helper with `None`.
  - Updated `init_first_run_grove_explore_absent_degrades` to create an empty `empty_path` temp directory and invoke `grove_mcp_llm_with_path(..., Some(&empty_path), ...)`. This guarantees `grove-explore` is not discoverable, so the degrade path is exercised deterministically.
  - Updated `init_first_run_grove_explore_present_non_tty_fails_fast` to use `grove_mcp_llm_with_path(..., Some(&bin_dir), ...)` so PATH is limited to the directory containing the freshly-built `grove-explore` binary.

- No source changes in `cli/src/init.rs`; the runtime behavior already matched the acceptance criteria.

## Test evidence

Targeted PATH-controlled integration tests:

```
running 2 tests
test init_first_run_grove_explore_present_non_tty_fails_fast ... ok
test init_first_run_grove_explore_absent_degrades ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out
```

Full workspace test suite:

```
running 136 tests
test result: ok. 136 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 65 tests
test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 43 tests
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 42 tests
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 54 tests
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Lint gate:

```
$ cargo clippy --all-targets --workspace --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
```

## Files changed

- `cli/tests/cli.rs`
