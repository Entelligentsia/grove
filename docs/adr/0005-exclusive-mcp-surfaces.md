# ADR 0005 — `grove` and `grove-explore` are mutually exclusive in the outer harness

- **Status:** Accepted
- **Date:** 2026-07-21
- **Deciders:** Boni Gopalan
- **Supersedes:** ADR 0004 §2 (`mcp-llm` means "register *both* servers in
  `.mcp.json`"). The mode key, the reconciliation design, and every other part
  of ADR 0004 stand unchanged.
- **Related:** `core/src/harness.rs` (`expected_mcp_args`, the shared writer /
  verifier source of truth), `cli/src/init.rs` (registration + steering block),
  `grove-explore/src/main.rs` (startup gate, server instructions),
  ADR 0002 (declared mode), ADR 0004 (the crate + surface split)

## Context

ADR 0004 split the explore delegate into its own binary and defined `mcp-llm`
as "register both servers." The two surfaces were held to compose: ask the
locator where something is, then use the structural tools on the citations it
returns. `grove init --as mcp-llm` wrote both registrations and a steering block
teaching that three-step flow.

In practice the composition has costs that were not visible when ADR 0004 was
written:

1. **Routing ambiguity.** The outer agent sees eight tools across two servers
   and must choose between them per question, then execute a multi-step protocol
   from prose instructions. Protocols expressed as prose degrade — the agent
   skips the follow-up, or skips the locator and navigates structurally itself,
   at which point the delegate's cost-saving premise is gone.

2. **The dereference argument does not hold.** The composition was justified by
   `explore` returning bare location lines with no way to read them cheaply.
   But a Claude Code-class outer harness has `Read` with `offset`/`limit`.
   Given `appcommon.js#configureSessionStore@228` it reads a window at line 228.
   `mcp__grove__source` knows the symbol's exact end line, which is a real but
   small advantage over guessing a window — not enough to justify a second
   server in the outer context.

3. **The capability is already present twice.** The inner harness carries nine
   tools — `Glob`, `Grep`, `Read`, and six `mcp__grove__*` — in-process, since
   `grove-explore` links `grove-cst` directly. The outer harness independently
   has its own `Grep`/`Glob`/`Read`. Registering grove's structural tools
   alongside `explore` adds a third path to capability already reachable two
   other ways.

4. **The flag name stopped matching its behaviour.** Pre-0.4.0 `mcp-llm` meant
   "switch `grove serve` into explore mode" — one server, and the name was
   accurate. ADR 0004 changed the meaning to "both" while keeping the spelling.

## Decision

**`mcp-llm` registers `grove-explore` alone.** The structural `grove` entry is
absent in that mode, and is stripped on transition into it.

The `--as` modes become properly exclusive:

| Mode | Registers | Outer agent |
|---|---|---|
| `--as mcp` (default) | `grove` | navigates structurally itself; no model in the loop |
| `--as mcp-llm` | `grove-explore` | delegates location; dereferences citations with its own `Read` |

This is expressed in one place — `harness::expected_mcp_args`, which returns
`None` for `McpLlm`. That function is the shared source of truth for `init`
(writer) and `doctor` (verifier), so both agree by construction.

The `mcp-llm` steering block is rewritten to describe the locator alone. It
must **not** name `mcp__grove__*` tools: in this mode they are not in the
agent's context, and steering toward absent tools is worse than silence.

### Failure behaviour

A down provider does **not** degrade to the structural surface. That fallback
was deleted in GROVE-S04-T03 and stays deleted. But the failure must be
*legible*, which is a separate question from whether it is *recoverable*:

- Startup config load and explore-section deserialize remain hard `exit(1)`.
  Without a valid config there is no endpoint to name in an error, so there is
  nothing useful to say in-band.
- The **health probe no longer exits.** Exiting killed the process before it
  answered `initialize`; the client rendered only a synthesized transport error
  (`Failed to reconnect to grove-explore: -32000`) and the agent never learned
  the tool existed. It degraded silently to grep. The probe now warns to stderr
  and serves anyway; an `explore` call against a down provider returns an
  actionable `isError` that reaches the model in-band.
- When the startup probe fails, the server's `initialize` **instructions** lead
  with `PROVIDER UNAVAILABLE` and the endpoint. Without this the model learns
  the provider is down only by spending calls on it — observed in the wild
  costing two `explore` calls before the agent concluded the backend was
  unreachable.

Serving with a down provider is not a fallback: the only tool is still
`explore`, and it reports that it cannot work. The agent receives an error, not
a substitute.

## Consequences

- **A down provider in `mcp-llm` means no grove tools at all.** Previously the
  structural server survived and could answer the question anyway. This is
  accepted: `mcp-llm` is opt-in and presumes an engine. Observed in the wild —
  an agent asked to find a login handler reported the provider was down, looked
  for the structural tools, found them unregistered, and used ripgrep.
- **`/mcp` shows `grove-explore` connected when it cannot work.** Mitigated by
  the instructions warning, which reaches the model even though the client's
  connection state looks healthy.
- **Existing `mcp-llm` projects keep their two-server layout** until re-inited.
  Nothing strips a registration that is not rewritten. Re-running
  `grove init --as mcp-llm` converges them.
- **Pre-split projects converge correctly.** A `.mcp.json` carrying
  `grove: ["serve", "--explore"]` has that entry *stripped* rather than
  rewritten — its `--explore` arg is a hard error post-T03, so a rewritten
  entry would fail at launch.
- **`--as mcp` is unchanged**, and remains the default and the recommended
  surface for most projects.

## Alternatives considered

- **Keep both, weaken the steering.** Register both but describe the structural
  tools as secondary. Retains the eight-tool routing surface and relies on
  prose to prevent the behaviour it describes — the failure mode this ADR
  exists to remove.
- **Make `explore` self-sufficient.** Have `grove-explore` expose `source` (and
  perhaps `map`) beside `explore`, giving one server with locate-and-read. This
  is coherent and was the leading option until the `Read(offset)` argument made
  it unnecessary. It remains available if outer harnesses without offset reads
  become a target.
- **Degrade at init.** Probe the provider during `grove init --as mcp-llm` and
  write the `mcp` registration instead when unreachable. Rejected: it makes the
  registration depend on a transient condition at setup time, so an engine that
  is merely not started yet silently produces the wrong mode.

## Notes

This ADR is a step toward ADR 0004 **Stage 3** (repo/release split). Once the
two products no longer appear in the same harness context, what remains to
sever is packaging and config ownership. Stage 3's stated triggers are
release-cadence divergence; the scope argument — a local-LLM code reader is not
an AST/CST tool and does not belong in grove's product surface — is independent
of those triggers and may justify Stage 3 on its own.

`grove-explore`'s in-process dependency on `grove-cst` is expected to survive
extraction: it becomes an ordinary crates.io dependency, the way any consumer
uses grove. It is not residual entanglement to be removed.
