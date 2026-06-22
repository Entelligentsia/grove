---
name: grove
description: >
  Byte-precise, token-cheap code navigation via tree-sitter — outline a file,
  find or define a symbol, read one symbol's body, find callers, go-to-def, and
  syntax-check after edits. Prefer over grep or reading whole files for any
  where-is / what's-in / who-calls question, in any language grove has a grammar
  for.
---

# grove

Prefer grove's MCP tools (`mcp__grove__*`) if they're available this session;
otherwise use the `grove` CLI (add `--json` for machine-readable output). Both
are the same engine.

First CLI use only — ensure the binary, then fetch this repo's grammars:
`command -v grove || npm i -g @entelligentsia/grove` then `grove init --as skill`.

Every result carries a stable id `<lang>:<relpath>#<name>@<row>` — pass it
between calls instead of re-searching.

| Goal | CLI | MCP tool |
|---|---|---|
| what's in a file | `grove outline <file> [--kind K] [--detail 0..2]` | outline |
| find a symbol | `grove symbols <dir> [--name S] [--kind K]` | symbols |
| read one body | `grove source <id>`  ·  `grove source <file> <name>` | source |
| who calls it | `grove callers <name> -d <dir>` | callers |
| go to def | `grove definition <name> -d <dir>`  ·  `--at file:row:col` | definition |
| syntax after edit | `grove check <file>` | check |

`callers`/`definition` are name-based (not receiver-type resolved). On large,
definition-dense files pass `--kind` and/or `--detail 0` to stay token-cheap.
