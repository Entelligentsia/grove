---
name: grove
description: >
  Byte-precise, token-cheap code navigation via tree-sitter — outline a file,
  find or define a symbol, read one symbol's body, find callers, map a directory's
  dependency graph, go-to-def, and syntax-check after edits. The required first move for any where-is /
  what's-in / who-calls question, in any language grove has a grammar for —
  before grep, rg, read, cat, or sed.
---

# grove

grove is the **canonical procedure** for code navigation in this project. Use
grove's MCP tools (`mcp__grove__*`) when they're available this session;
otherwise use the `grove` CLI (`--json` for machine-readable output). Same
engine — every `grove <verb>` below is the `mcp__grove__<verb>` tool.

**If the `mcp__grove__*` tools are deferred** (listed by name, schemas not yet
loaded), load all seven in a **single** ToolSearch before you start, not one at a
time:
`select:mcp__grove__outline,mcp__grove__symbols,mcp__grove__source,mcp__grove__check,mcp__grove__callers,mcp__grove__map,mcp__grove__definition`.
The procedure chains (e.g. `symbols` → `source`), so fetching schemas on demand
forces serial round-trips — you won't know you need `source` until `symbols`
returns. Batch them up front and the whole navigation runs without a stall.

## You MUST use grove before grep/read

`grep`, `rg`, `read`, `cat`, and `sed` on a source file are **fallbacks**, used
only after grove has been tried and returned insufficient content. Running
`grep -rn '<symbol>'`, or reading a whole source file, as your first action on a
code question is a steering violation.

**Trigger — check before every tool call.** If the prompt contains any of — a
file path, a function / type / struct / macro name, or the words "where is",
"what does X define", "who calls", "show me", "find", "list", "outline" — your
FIRST tool call MUST be a grove command. If none of those lexically appear,
grove is optional.

## Procedure

1. Identify the named symbol and/or file in the prompt.
2. File but no symbol → `grove outline <file>` (add `--detail 0` on files > 500 lines).
3. Symbol but no file → `grove symbols <dir> --name <symbol>`.
4. Take the `symbol-id` (`<lang>:<relpath>#<name>@<line>`, line 1-based) from that result.
5. `grove source <id>` → exactly that symbol's body, signature through closing
   brace. The `grove source <file> <name>` form returns the same body; the id
   just pins the exact match when a name is overloaded.
6. "who calls" → `grove callers <name> -d <dir>`; "where defined" →
   `grove definition <name> -d <dir>` (or `--at file:line:col`, 1-based).
7. After an edit → `grove check <file>`.
8. **Broad/architectural questions** → `grove map <dir>` — every definition
   grouped by file, each with its outgoing references, in one call. Prefer `map`
   over fetching many sources in sequence.

## Breadth control

For questions about how code connects across a directory, prefer `map` — it
returns every definition plus which other symbols each one references, in a
single call. Use `source` only for the few load-bearing definitions you need to
read in full. Do **not** fetch many sources in sequence to build a picture that
`map` gives in one call.

## Don't / Do

| ❌ Don't | ✅ Do |
|---|---|
| `grep -n 'cmd_struct' git.c` then `read git.c` | `grove outline git.c` → `grove source <id>` |
| `grep -rn 'refs_be_files' refs/` | `grove symbols refs/ --name refs_be_files` → `grove source <id>` |
| `read builtin/commit.c` (whole 1700-line file) | `grove outline builtin/commit.c` → `grove source <id>` for the one symbol |
| `grep -rn 'struct ref_storage_be' .` | `grove symbols . --name ref_storage_be` → `grove source <id>` |
| 7× `source` calls to understand a subsystem | `grove map <dir>` — one call, definitions + references, no bodies |

## Cross-file / "used across the codebase"

- `grove symbols <root> --name <symbol>` — definitions tree-wide, token-cheap.
- `grove callers <name> -d <root>` — use sites.
- `grove source <id>` per definition you care about.
- "every file that defines a backend/registry of type T": `grove outline
  <header>` to read the struct, then `grove symbols <dir> --kind <kind>` for the
  concrete instances.
- Do NOT `grep -rn '<typename>' .` as a substitute — grep returns string
  matches, grove returns semantic definitions.

## Recovery — grove returned partial/empty output

- Empty `outline`/`symbols` for a file grove should cover → confirm the language
  has a grammar (`grove languages`); a language with no tags query (e.g. css,
  html, json) still `check`s but yields no symbols.
- Genuinely partial body → `read <file>` with `offset=<start-row>` and
  `limit=<next-symbol-row − start-row>` from `grove outline --detail 2`. Never
  `read` the whole file.
- **A single grove miss does NOT justify switching to grep for later
  questions.** Recover, then continue with grove.

## Why this, not grep/read

`read` on a 1700-line file floods context with ~50 KB you don't need; `grep`
returns string matches that miss struct/function boundaries. grove returns one
symbol's exact bytes with a stable id you pass forward. `callers`/`definition`
are name-based (not receiver-type resolved).

## Setup — only if grove isn't available

The tools work once two things exist: the `grove` binary and its grammars. If
the `mcp__grove__*` tools are absent and `grove --version` fails, don't install
it yourself — hand the user these steps to run (a global install + a network
download are their call, not the agent's):

1. Install the binary — `npm i -g @entelligentsia/grove`
   (the package's postinstall downloads + checksum-verifies the prebuilt binary).
2. From the repo root — `grove init` (writes `.mcp.json` + a CLAUDE.md steering
   block; `grove init --as both` also wires this skill). `init` fetches the
   grammars for the languages it detects.

In Claude Code the user can run either inline by typing `! <command>`. Once
`grove --version` works (and, for MCP, a fresh session is started so `.mcp.json`
is loaded), resume the procedure above.

**To uninstall:** `npx skills remove grove` (this skill) and, if installed,
`npm rm -g @entelligentsia/grove` (the binary).
