# ADR 0001 — Scope-aware and import-edge name resolution

- **Status:** Accepted
- **Date:** 2026-06-27
- **Deciders:** Boni Gopalan
- **Supersedes:** —
- **Related:** VISION.md §6.3 (`definition` "via `@local.scope` / `@local.definition`
  resolution"), §8 (tool→API mapping), Roadmap Tier 3 ("scope-aware
  `callers`/`definition` via the tags `locals` query")

## Context

Grove's `definition` and `callers` resolve symbols **lexically by name**. Today
`definition_at` (go-to-def from a cursor) finds the identifier under the cursor
and then returns *every* symbol with that name in the directory:

```rust
// ops.rs — definition_at
let name = engine::identifier_at(...)?;  // "parse"
let defs = definition(dir, &name)?;      // every "parse" in the dir
```

This has two precision failures that an LLM-driven agent feels directly:

1. **No scope awareness.** A local variable or parameter named `parse` resolves
   to the module-level function `parse`. Shadowing is invisible. `callers` can
   even count a local binding as a call site (today mitigated only by a lossy
   textual fallback).
2. **No cross-file awareness.** A reference to an imported top-level symbol
   resolves to *all* same-named defs across the directory, not the one the import
   actually binds — and never to a symbol in a file the agent hasn't opened.

The strategic question (see the conversation that motivated this ADR) was whether
to adopt **stack-graphs** (built on tree-sitter-graph) for LSP-grade,
build-tool-free name resolution. That path is real but heavy: per-language `.tsg`
rule sets are large, the upstream `github/stack-graphs` is archived
(2025-09-09), and it forces a persisted, invalidated graph **index** — abandoning
grove's stateless, parse-on-demand model and its "drop a registry dir, no
recompile" wedge.

This ADR records the decision to first capture the high-value, low-risk subset of
that capability **without** an index and **without** per-language `.tsg`, using
machinery tree-sitter already provides and grove already ships partial support
for. It is explicitly the plan VISION promised, not a vision amendment.

## Decision

Add resolution in two independent, separately-shippable steps. Both stay
**stateless** (no persisted index, no watcher) and extend the **registry-dir
data model** rather than the binary, consistent with the WASM-registry spine.

### Step 1 — Scope-aware resolution via the `locals` query

- Add an **optional `locals.scm`** artifact to each registry dir, alongside
  `tags.scm` + `manifest.json` + `grammar.wasm`. It uses tree-sitter's standard
  locals capture vocabulary: `@local.scope`, `@local.definition`,
  `@local.reference`. A grammar without one keeps today's behavior exactly.
- The engine compiles `locals.scm` (lazily, in the per-process `Loaded` cache)
  and gains `resolve_local_at(root, row, col, …)`: find the identifier under the
  cursor, then walk **enclosing `@local.scope` ranges innermost→outermost** for a
  `@local.definition` of the same name contained in that scope. Innermost match
  wins (correct shadowing). Returns a `Symbol` for the binding, or `None`.
- `definition_at` tries local resolution **first**. A hit returns the single
  binding; a miss falls through to today's directory-wide name lookup. Never
  worse than current behavior.

Scope: a single file, one parse. No new IO. Fully testable against the dev stub
(rust/python/javascript), each of which gains a minimal `locals.scm`.

### Step 2 — Import-edge cross-file resolution

- Add an optional **`imports`** block to the manifest `profile` (data, not code),
  describing the import statement node kinds, the fields holding the module path /
  imported names / alias, and a **module-resolution strategy** enum
  (`dotted_package` for Python, `relative_path` for JS/TS, `use_path` for Rust).
  A grammar without it keeps today's behavior.
- `definition_at` decision tree becomes:
  1. local scope resolution (Step 1) — *1 file parsed*;
  2. else, if the name is bound by an import in this file: resolve the module path
     to a file (string transform + a couple of `exists()` probes, **no repo
     scan**), parse **that one target file**, return its matching top-level def —
     *2 files parsed*;
  3. else, today's directory-wide name lookup.

Resolution cost is bounded by **import-chain depth, not repo size**. No index, no
invalidation. This is the increment that delivers the headline "cross-file
go-to-def" for the common case (imported top-level symbols).

## Scope boundary — what this deliberately does NOT do

These are the stack-graphs frontier and are out of scope by construction. When
hit, resolution degrades to returning the **candidate list** (today's behavior),
never a confident wrong answer:

- **Method/receiver typing** — `foo.bar()` → which `bar`? Needs `foo`'s type.
- **Multi-hop re-export / barrel chains** — Step 2 follows one import hop well;
  cyclic/transitive re-exports need partial-path stitching.
- **Wildcard, dynamic, conditional imports; monkey-patching.**

An LLM brain consuming grove is good at picking from a short candidate list with
surrounding context, so candidate-list degradation is an acceptable floor.

## Alternatives considered

- **Adopt stack-graphs / tree-sitter-graph now.** Rejected for the near term:
  heavy per-language `.tsg`, archived upstream, and a persisted index that breaks
  the stateless model and the broad-language wedge. Kept as a possible later
  "deep tier on a few languages" bet; Step 2's measured hit-rate on the testbench
  becomes the go/no-go evidence for it. (LLM-authored `.tsg` inside a
  test-driven oracle loop could lower its cost — a separate investigation.)
- **tree-sitter-graph for richer single-file graphs only.** Doesn't deliver
  cross-file linkage; rejected as a primary direction.
- **Do nothing / keep candidate lists.** Rejected: scope and import precision are
  cheap and high-value, and already promised in VISION.

## Consequences

**Positive**

- `definition` returns one correct binding for locals and imported symbols in the
  common case; `callers` precision improves (locals no longer counted as calls).
- Stateless model, trust story, and zero-recompile language onboarding preserved.
- Cross-file resolution arrives without an index — a genuinely cheap win.
- Generates the evidence to decide the larger stack-graphs question.

**Negative / costs**

- Two new optional registry artifacts/fields to author per language (`locals.scm`,
  `imports`), but they degrade gracefully when absent.
- Step 2 re-parses target files per query (bounded by import depth) — acceptable
  under grove's existing stateless tradeoff.
- Not LSP-complete (see scope boundary); the tail still needs the agent's help or
  a future stack-graphs tier.

## Status of implementation

- Step 1: implemented (engine `resolve_local_at`, `locals.scm` for
  rust/python/javascript, `definition_at` wiring, unit + integration tests).
- Step 2: implemented (engine `extract_imports` + `imports.scm`, ops
  `resolve_import_at` / `import_candidate_paths` with `dotted_package` and
  `relative_path` strategies, `import_resolution` manifest field, `definition_at`
  decision tree local→import→dir-wide, unit + integration tests). Shipped for
  python/javascript; rust import resolution (`use_path`) is deferred — rust falls
  back to directory-wide lookup. Next: validate the cross-file hit-rate on
  grove-testbench before any stack-graphs decision; widen the per-language
  `locals.scm`/`imports.scm` (match arms, destructuring, `import *`, re-exports).
