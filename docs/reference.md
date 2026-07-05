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
by `grove init` and read by `grove serve` / `grove config` / `grove doctor` (see
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
  `grammars`). `grove serve` runs the explore surface only when `mode` is
  `mcp-llm`; otherwise it serves the 7 structural tools.
- **`explore`** — present when `mode` is `mcp-llm`:
  - `provider` — `ollama` or `llamacpp` (both speak the OpenAI-compatible wire
    protocol).
  - `base_url`, `model` — the inference endpoint and model id.
  - `steering` — the steering arm: `standard` (merit), `balanced` (plan-first),
    or `strict` (grove-first).
  - `allowed_tools` — the tools the inner explorer may invoke.
  - `tap` — record per-session traces under `.grove/traces/` (browse with
    `grove tap`).
  - `trace_retain` — how many trace sessions to keep (`0` = keep all).

> The legacy `.grove/explore.json` is **migrated forward** to `config.json` on
> first load; `grove doctor` flags it until you remove it.

## `grove.lock`

Written by `grove lock` (and `grove init`), pinning the grammars a project needs
by version + wasm `sha256`, so a checkout resolves the same grammars every time.
See [Languages & grammars](languages.md).
