# FAQ

## Is grove an LSP?

**No.** grove is not a Language Server. It doesn't speak the LSP protocol, and it
deliberately stops short of the semantic work a language server does.

grove is a **syntactic, tree-sitter-powered structural-access layer for coding
agents** — a "syntactic shell" — exposed as a CLI (`grove <verb>`) and an MCP tool
server (`grove serve`). It returns the exact bytes of one symbol, a file's
definition skeleton, or a directory's definition/reference graph — token-cheap,
with stable ids the agent passes between turns. It does not build a type graph,
resolve scopes, or infer anything.

### The line grove won't cross

grove's [non-goals](../VISION.md#4-goals--non-goals) are explicit: **not a language
server** — no diagnostics, completion, or type inference; and **not a refactoring
engine** — no rename/move/extract. grove locates; the agent edits. The moment you
need *semantics* (what type is this, what does this resolve to, rename every
binding), you're in LSP territory, and grove hands off rather than half-does it.

### How they differ

| | grove | an LSP server |
|---|---|---|
| What it models | the **parse tree** (syntax) | a **semantic index** (types, scopes, imports) |
| go-to-def | name-based, or the identifier at a position (tree-sitter) — not type/receiver resolved | type- and scope-resolved |
| find references | structural (tags) + whole-word textual fallback, by name | scope- and type-aware |
| hover / completion | — | yes |
| rename / refactor | — | yes |
| diagnostics | syntax only (`check` → ERROR / MISSING nodes) | type errors, lints, semantic warnings |
| cross-file type graph | — | yes (the core of a language server) |
| Protocol | **MCP** (grove's own JSON-RPC tool schema) | **LSP** (`textDocument/definition`, `hover`, `references`, …) |
| Consumer | coding agents | IDEs / editors |
| Languages | **one binary**, all 27 grammars load at runtime from a WASM registry | **one server per language**, usually a heavy language-specific toolchain |
| Index | parses on demand (no persistent DB) | a persistent, incrementally-maintained semantic DB |

### Complementary, not competitive

grove is the cheap, always-on, language-agnostic **syntactic** first move; an LSP
is the authoritative, language-specific **semantic** authority. They sit at
different layers:

- **grove** answers "where is this defined, what are its bytes, who calls it by
  name, how does this directory connect" — fast, in any language grove has a
  grammar for, with no toolchain on the consumer.
- **an LSP** answers "what's the type of this, where does this *really* resolve,
  rename it everywhere it's bound" — authoritatively, for one language, with a
  real build.

An agent can use grove for ~90% of navigation and reach for an LSP (if one's
running in the project) only when it truly needs type-resolved semantics. grove
doesn't aspire to replace an LSP; it aspires to make the *syntactic* 90% cheap
enough that the agent rarely needs the heavier 10%.

### Where grove would start to look LSP-ish (and why it's deferred)

The few places grove edges toward semantics — a backward slice to trace a
parameter's shape, require/import resolution, constructor-shape extraction — are
explicitly **Bridge 1** work ([issue #37](https://github.com/Entelligentsia/grove/issues/37)
non-goals): grove's first heuristic, not-faithful-to-the-parse-tree step, i.e. the
first step toward re-implementing an LSP. It is deliberately out of scope for now;
grove stays a syntactic shell and lets the model keep doing the semantics.

### So: is grove an LSP?

No. It's a tree-sitter-powered structural navigation tool for agents — the cheap
syntactic layer **beneath** where an LSP's semantic intelligence begins. It speaks
MCP, not LSP; it parses, it doesn't analyze; it locates, it doesn't refactor.

---

[Back to README](../README.md) · [VISION](../VISION.md) · [Tools](tools.md) · [Roadmap](roadmap.md)