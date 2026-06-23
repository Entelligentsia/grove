# Tools

grove's surface is the agent loop in miniature — seven tools, each returning one
symbol's worth of structure with a stable id. Add `--json` to any command for the
agent-facing structured shape.

| Phase | Command | What it does |
|---|---|---|
| Read | `grove outline <file> [--kind K] [--detail 0\|1\|2]` | compact definition skeleton: kind · name · parent · signature · id. Filter by kind / dial detail down for big files |
| Find | `grove symbols <dir> [--kind K] [--name SUB] [--name-contains] [--refs]` | repo-wide symbol search (gitignore-aware). `--name` is **exact** (case-insensitive); `--name-contains` (alias `--name-substr`) opts into substring |
| Read | `grove source <id>` or `grove source <file> <name>` | full source of one symbol — no whole-file read |
| Verify | `grove check <file>` | ERROR / MISSING nodes (exit 1 if any) — post-edit syntax check |
| Trace | `grove callers <name> [-d <dir>]` | call sites of a symbol, each with its enclosing function |
| Map | `grove map <dir> [--kind K] [--name SUB] [--name-contains]` | directory dependency graph: definitions + outgoing references, no source bodies |
| Trace | `grove definition <name> [-d <dir>]` or `grove definition --at <file:line:col>` | go-to-def, by name or from a usage position (`line`/`col` are 1-based) |

## Conventions

- Lines and columns are **1-based** everywhere grove reports or accepts them
  (the editor / `grep -n` convention).
- Every result carries a stable **`symbol-id`** (`<lang>:<relpath>#<name>@<line>`,
  line 1-based) usable across turns — pass it from `outline`/`symbols`/`map` into
  `source`, `callers`, `definition`.
- `--name` matches **exactly** (case-insensitive) by default so `--name batch`
  returns `batch`, not `testCreateBatch`. Use `--name-contains` for fuzzy
  substring exploration. ([issue #37](https://github.com/Entelligentsia/grove/issues/37))

## Detail tiers (`outline`)

`--detail` controls field density so big files stay token-cheap:

- `0` — terse: kind · name · parent · line
- `1` — default: adds id · col · signature
- `2` — full: adds byte offsets (for `read` slicing between symbols)

## Examples

```bash
grove languages
grove outline foo.py --kind class        # python, loaded from wasm at runtime
grove outline src/engine.rs --kind function
grove source  src/mcp.rs serve
grove callers extract -d src
grove map     src --kind function         # dependency graph, no source bodies
grove check   src/registry.rs
grove symbols . --name batch              # exact: just `batch`
grove symbols . --name batch --name-contains   # substring: testCreateBatch, …
```

## When to use which

- **One file** → `outline` (skeleton) → `source <id>` (the one symbol's body).
- **Where is X defined across the repo** → `symbols <dir> --name X` → `source <id>`.
- **Who calls X** → `callers <name> -d <dir>`.
- **How does this directory connect** → `map <dir>` (definitions + outgoing
  references, one call, no bodies) — replaces many `symbols`+`source` round-trips.
- **After an edit** → `check <file>` to confirm you didn't break syntax.

The cross-harness [skill](../skills/grove/SKILL.md) encodes these chains as the
agent's default procedure.

---

Next: [MCP server](mcp.md) · [Skill →](../skills/grove/SKILL.md)