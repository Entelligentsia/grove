# MCP server

`grove serve` runs an MCP server over stdio (newline-delimited JSON-RPC 2.0),
exposing the same seven [tools](tools.md) for every registered language. Register
it with a coding agent and the agent gains structural sight.

Project-scoped registration for Claude Code lives in [`.mcp.json`](../.mcp.json)
(written by `grove init`):

```json
{ "mcpServers": { "grove": { "command": "…/target/release/grove", "args": ["serve"] } } }
```

## Availability vs. adoption

`.mcp.json` makes the tools *available*; a `CLAUDE.md` steering directive (see
[VISION §6.4.1](../VISION.md)) is what gets the agent to actually *use* them
rather than defaulting to grep / whole-file reads. `grove init --as mcp` writes
both — see [Setup](setup.md).

## Two surfaces, one at a time

`grove` isn't the only MCP server in the family. **`grove-explore`** is a
separate binary with its own MCP server — a single `explore` tool that
delegates broad "where is X" questions to a local LLM and returns validated
`file:line` citations (see [Setup — explore-mode](setup.md)). A project
registers one surface **or** the other, never both (see
[ADR 0005](adr/0005-exclusive-mcp-surfaces.md)) — both together would put eight
tools in front of the agent with no rule for choosing:

| `--as` | Registers under `grove` | Tools |
|---|---|---|
| `mcp` *(default)* | `grove serve` | `mcp__grove__*` — the 7 structural tools |
| `mcp-llm` | `grove-explore serve` | `mcp__grove__explore` — one delegating locator |

Both surfaces register under the **same `grove` key**, so the locator's tool is
`mcp__grove__explore` — the historical name, and one the outer agent reads as
"grove", not as a binary. Switching modes swaps the entry;
`grove init` strips the surface you left. See [Setup](setup.md) for the full
`--as mcp-llm` wiring.

## Tool schemas

Every `inputSchema` is a plain `{type: "object", properties, required?}`. Tool
list is returned by `initialize` → `tools/list`. `outline` is tiered (`--detail`
0|1|2); `symbols`/`map` take `name` (exact, case-insensitive) plus a `nameContains`
boolean for substring matching.

## Result & error model

Tool results are JSON inside an MCP text block. Tool-level failures come back as
`isError: true` with a message so the model can recover (e.g. missing required
arg, unknown language, no identifier at a position). Unknown method →
`-32601 method not found`; unknown tool / bad args → `-32602 invalid params`.

## Same engine as the CLI

`grove serve` and `grove <verb>` call the same `ops` library, so a human at the
shell and an agent over MCP see identical bytes. The [cross-harness skill](../skills/grove/SKILL.md)
prefers the MCP tools when the host exposes them and falls back to the CLI
otherwise — equal partners over one engine.

---

Back: [Tools](tools.md) · [Setup](setup.md) · [Roadmap & layout](roadmap.md)