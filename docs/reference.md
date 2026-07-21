# Reference

Formats and files grove reads and writes.

## Symbol id

Every result across the whole surface carries a stable **symbol id** you can pass
from one tool to the next (`outline` / `symbols` / `map` → `source` / `callers` /
`definition`):

```
<lang>:<relpath>#<name>@<line>
```

- `lang` — the grammar name (`rust`, `python`, `typescript`, …).
- `relpath` — the file path relative to the working directory.
- `name` — the symbol's name.
- `line` — the **1-based** line of the name.

Example: `rust:core/src/ops.rs#symbols@153`.

### 1-based lines & columns

Lines and columns are **1-based everywhere** grove reports or accepts them (the
editor / `grep -n` convention) — including `definition --at file:line:col`. This
is a deliberate normalization; tree-sitter's own points are 0-based.

## `.grove/config.json`

The single, versioned source of truth for a project's grove integration, written
by `grove init` and read by `grove serve`, `grove-explore serve`/`config`/`tap`,
and `grove doctor` (see
[ADR 0002](adr/0002-grove-project-config-and-declared-mode.md)).

```json
{
  "version": 1,
  "mode": "mcp-llm",
  "explore": {
    "provider": "ollama",
    "base_url": "http://localhost:11434/v1",
    "model": "qwen2.5-coder:7b",
    "steering": "standard",
    "allowed_tools": ["grove", "rg", "grep", "find"],
    "tap": false,
    "trace_retain": 50
  }
}
```

- **`mode`** — the integration mode (`mcp` · `skill` · `both` · `mcp-llm` ·
  `grammars`), the harness-reconciliation key for `init`/`doctor`. `grove serve`
  is **unconditionally** the 7-tool structural surface regardless of `mode` —
  `mode` no longer selects a serve surface. `mode: mcp-llm` means "`init`
  registers **both** `grove` and `grove-explore` in the harness (`.mcp.json`,
  `CLAUDE.md`, `AGENTS.md`)"; `grove-explore` is its own MCP server binary
  with its own always-on `explore` surface, composable alongside `grove serve`.
- **`explore`** — present when `mode` is `mcp-llm`; read by `grove-explore`:
  - `provider` — `ollama` or `llamacpp` (both speak the OpenAI-compatible wire
    protocol).
  - `base_url`, `model` — the inference endpoint and model id.
  - `steering` — the steering arm: `standard` (merit), `balanced` (plan-first),
    or `strict` (grove-first).
  - `allowed_tools` — the tools the inner explorer may invoke.
  - `tap` — record per-session traces under `.grove/traces/` (browse with
    `grove-explore tap`).
  - `trace_retain` — how many trace sessions to keep (`0` = keep all).

> `grove config` / `grove tap` still work as deprecated forwarding shims to
> `grove-explore config` / `grove-explore tap` — they print a one-line note and
> forward to the sibling binary. Use the `grove-explore` spelling directly.

> The legacy `.grove/explore.json` is **migrated forward** to `config.json` on
> first load; `grove doctor` flags it until you remove it.

## `grove.lock`

Written by `grove lock` (and `grove init`), pinning the grammars a project needs
by version + wasm `sha256`, so a checkout resolves the same grammars every time.
See [Languages & grammars](languages.md).
