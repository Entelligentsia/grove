# GROVE-S02-T07: Integration tests, naming guard, docs

**Sprint:** GROVE-S02
**Estimate:** M
**Pipeline:** default

---

## Objective

Harden the sprint end-to-end and make it shippable: close the cross-task test
gaps that no single task owns, enforce the naming ban mechanically, and document
the mcp-llm mode for users.

## Acceptance Criteria

1. Cross-cutting `tests/cli.rs` additions (beyond what T04/T06 landed):
   - `grove config` in a non-TTY context fails fast with the descriptive
     message (exit non-zero, no hang — guarded with a timeout).
   - Init idempotency end-to-end: `grove init --as mcp-llm --dry-run` twice is
     stable; a real double-init (with pre-seeded `.grove/explore.json` so the
     TUI is skipped or pre-populated non-interactively per the T05/T06
     contract) duplicates nothing in CLAUDE.md / AGENTS.md / .mcp.json.
   - Allowlist enforcement: a config whose `allowed_tools` omits `find` causes
     the explorer's dispatcher to refuse a `find` invocation (unit-level via
     the fake `ChatClient`, asserting the corrective tool-result).
2. **Naming guard test:** a test walks `core/src/`, `cli/src/`, `README.md`,
   and `skills/` asserting the string "fastcontext" (case-insensitive) appears
   nowhere (D1/D5).
3. Docs:
   - `README.md`: mcp-llm mode section — what it is (delegated local
     exploration, one `mcp__grove__explore` tool), setup (`grove init --as
     mcp-llm`, `grove config`), the three modes with one-line trade-offs, and
     the health semantics (startup probe; provider connection errors shut the
     server down).
   - `CHANGELOG.md`: dated unreleased section covering `--as mcp-llm`,
     `grove config`, serve explore mode, and the shutdown behavior.
   - `CLAUDE.md` (repo): the new commands added to the Commands block; the
     `core::explore` module added to the architecture map.
4. Stack-checklist review pass recorded in the PR description: MCP tool schema
   plainness, stdio hygiene (protocol/stdout vs diagnostics/stderr), exit
   codes, `--json` behavior of any new CLI output, no new unsafe, rustls-only.
5. Full gates green: `cargo build --release --locked --workspace`
   warning-clean, `cargo clippy -- -D warnings`, `cargo test --release
   --locked`, coverage not regressed materially
   (`cargo llvm-cov --summary-only`).

## Context

Depends on **T04** and **T06** (all surfaces exist). D6: functional
correctness only — benchmarking the delegation win is explicitly
post-implementation (grove-testbench), so this task does NOT add eval harness
work. Manual verification against a live Ollama endpoint is expected during
this task (documented as a manual step in the PR, not a CI dependency).

## Artifacts Involved

- Edited: `tests/cli.rs` (or `cli/tests/cli.rs`), `README.md`, `CHANGELOG.md`,
  `CLAUDE.md`.
- New: naming-guard test (in `tests/cli.rs` or a core test).

## Operational Impact

- **Version bump:** YES — this task closes the sprint; bump per RELEASING.md
  (Cargo.toml + Cargo.lock + dist/npm/package.json + CHANGELOG) is prepared
  here or in the release PR that follows.
- **Regeneration:** none.
- **Security scan:** shell-dispatch allowlist behavior is part of the review
  (no interpolation, vector args only).
