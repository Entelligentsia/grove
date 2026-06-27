---
name: grove
description: >
  Byte-precise, token-cheap code navigation via tree-sitter — outline a file,
  find or define a symbol, read one symbol's body, find callers, map a directory's
  dependency graph, go-to-def, and syntax-check after edits. The first move for any
  where-is / what's-in / who-calls question, in any language grove has a grammar
  for — with grep, rg, and read kept for text search and quick facts.
---

# grove

grove is the **canonical procedure for structural code navigation** in this
project. Use grove's MCP tools (`mcp__grove__*`) when they're available this
session; otherwise use the `grove` CLI (`--json` for machine-readable output).
Same engine — every `grove <verb>` below is the `mcp__grove__<verb>` tool.

**If the `mcp__grove__*` tools are deferred** (listed by name, schemas not yet
loaded), load all seven in a **single** ToolSearch before you start, not one at a
time:
`select:mcp__grove__outline,mcp__grove__symbols,mcp__grove__source,mcp__grove__check,mcp__grove__callers,mcp__grove__map,mcp__grove__definition`.
The procedure chains (e.g. `symbols` → `source`), so fetching schemas on demand
forces serial round-trips — you won't know you need `source` until `symbols`
returns. Batch them up front and the whole navigation runs without a stall.

## grove for structure, shell for the rest

grove and your shell tools (`grep`, `rg`, `read`, `cat`, `ls`, `find`) are
**partners** — use each for what it does best, and combine them when that is the
shortest path to a grounded answer.

**Reach for grove when the target is a named symbol or a structural relationship.**
If the prompt names a file, a function / type / struct / macro, or asks "where is",
"who calls", "what's in", "how does this connect" — grove answers it precisely and
token-cheap, and returns a stable `symbol-id` to pass forward. Start there instead
of grepping for the symbol or reading the whole file.

**Reach for the shell when grove can't see the target — it's the right tool, not a
fallback:**
- **Text, not a symbol** — a string literal, a log / error message, a config key, a
  macro's *value*, a constant, a flag, a `TODO` → `grep -rn` / `rg`. grove finds
  *named definitions*, not arbitrary text, and (today) has no text-search tool.
- **Non-code / unparsed files** — Makefiles, `*.conf`, YAML / JSON data, docs →
  `grep` / `read`. (css / html / json grammars `check` but yield no symbols.)
- **A quick fact** — does a path exist, list a dir (`ls`), count lines (`wc -l`),
  find files by name (`find`, `git ls-files`), read a genuinely small file → shell.
  A grove round-trip to confirm one line is wasted motion.

**Combine — often the shortest path** (grove and the shell share 1-based lines and
the same bytes):
- `grep -rn '<text>'` to find the line a literal or call site sits on → `grove
  definition --at <file:line:col>` to resolve the enclosing / target symbol.
- `grove outline <file>` for the shape → a **bounded** `read` (`offset`/`limit` from
  the outline) to grab a contiguous block of small adjacent symbols when that beats
  N `source` calls.
- `grove symbols` / `map` to locate the subsystem → `grep` to pin a constant inside.

## Procedure (symbol / structure questions)

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
read in full. Don't fetch many sources in sequence to build a picture that
`map` gives in one call.

## For symbol work: Don't / Do

When the target *is* a named symbol, grove beats grep+read — it returns the exact
body with a stable id, no whole-file read, no false hits from comments or strings:

| ❌ Don't (for a symbol) | ✅ Do |
|---|---|
| `grep -n 'cmd_struct' git.c` then `read git.c` | `grove outline git.c` → `grove source <id>` |
| `grep -rn 'refs_be_files' refs/` | `grove symbols refs/ --name refs_be_files` → `grove source <id>` |
| `read builtin/commit.c` (whole 1700-line file) | `grove outline builtin/commit.c` → `grove source <id>` for the one symbol |
| `grep -rn 'struct ref_storage_be' .` | `grove symbols . --name ref_storage_be` → `grove source <id>` |
| 7× `source` calls to understand a subsystem | `grove map <dir>` — one call, definitions + references, no bodies |

(Searching for the *text* `refs_be_files` — a log line, a comment, a config value —
is the opposite case: that's what `grep` is for.)

## Cross-file / "used across the codebase"

- `grove symbols <root> --name <symbol>` — definitions tree-wide, token-cheap.
- `grove callers <name> -d <root>` — use sites.
- `grove source <id>` per definition you care about.
- "every file that defines a backend/registry of type T": `grove outline
  <header>` to read the struct, then `grove symbols <dir> --kind <kind>` for the
  concrete instances.
- For a type's *definition*, prefer `grove symbols --name <type>` over
  `grep -rn '<typename>' .` — grep returns string matches (comments, casts, every
  mention), grove returns the semantic definition. (Want every textual mention?
  Then grep is the right tool.)

## Trace a parameter's shape (dynamically-typed languages)

"What is the type/shape of the `batch` argument inside `panouploaddb.createBatch`?"
On a **dynamically-typed** codebase grove has no type system, so you reconstruct
the shape by a short backward slice — **~3 hops, mostly grove tools**. (On a
statically-typed language this is usually one hop: `grove source <id>` — the type
is already in the signature grove slices.)

Procedure:

1. `grove symbols <dir> --name createBatch` (exact match) → take the id; then
   `grove source <id>` to read the signature and name the parameter of interest.
2. `grove callers createBatch -d <root>` → every call site plus its enclosing
   function (`callers` returns the enclosing fn so you don't grep for it).
3. For each call site: `grove source <enclosing-fn-id>` and read **how the
   argument is constructed**. If it's a constructor / factory / `require(...)` /
   `import` binding, resolve that name exactly — `grove symbols <dir> --name
   <Constructor>` → `grove source <id>` — and merge its field assignments with
   any caller-side mutations. `grove map <dir>` can fold several of these hops
   into one call when the construction is local to one directory.

Per-idiom table (the construction site to chase at step 3):

| Language | module/binding | instance shape |
|---|---|---|
| JS/TS | `require('./x')`, `import x` | `this.foo =`, `Object.assign`, literal `{...}` |
| Python | `import x`, `from x import Y` | `self.foo =`, `__init__`, `@dataclass` fields |
| Ruby | `require 'x'`, module include | `@foo =`, `attr_accessor`, `initialize` |
| Go | `import "pkg"` | `&Foo{...}` literal, `NewFoo(...)` constructor |
| PHP | `use Namespace\Cls` | `$this->foo =`, `new Cls(...)` |

**Profile gate.** `callers` and the enclosing-function field only exist for the
~15 **full-profile** languages (the ones whose `manifest.json` declares
`function_kinds`/`identifier_kinds`). On the **minimal-profile** languages this
procedure degrades to: `grove outline <file>` → `grove source <id>` for each
candidate, i.e. the model reads the few relevant symbols by id. It earns its
keep on the **dynamically-typed, full-profile** subset (JS/Python/Ruby/PHP/…);
on statically-typed full-profile languages (Rust/Go/…) `source` already carries
the type and the slice is unnecessary.

**Resolve names with grove.** Use `grove symbols --name <exact>` (exact by
default; `--name-contains` only for deliberate fuzzy exploration) and read bodies
with `grove source <id>` — that's cheaper and more precise than `grep -rn
'<constructor>'` + a whole-file `read` to chase a `require` to its definition.

## Recovery — grove returned partial/empty output

- Empty `outline`/`symbols` for a file grove should cover → confirm the language
  has a grammar (`grove languages`); a language with no tags query (e.g. css,
  html, json) still `check`s but yields no symbols.
- Genuinely partial body → `read <file>` with `offset=<start-row>` and
  `limit=<next-symbol-row − start-row>` from `grove outline --detail 2`.
- For continued *symbol* questions, recover with grove (re-run `source` by id)
  rather than dropping to grep — but if the target turns out to be text or a
  non-code file, the shell was the right tool all along.

## When grove, when shell

`read` on a 1700-line file floods context with ~50 KB you don't need; `grep`
returns string matches that miss struct/function boundaries and conflate
same-named symbols. For a **symbol**, grove returns its exact bytes with a stable
id you pass forward (`callers`/`definition` are name-based, not receiver-type
resolved). For **text, a non-code file, or a quick fact**, the shell is faster and
correct. Most real questions want grove to navigate and the shell to confirm — use
both.

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
