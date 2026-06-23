# Languages & grammars

A grammar is a `registry/<lang>/` directory holding `grammar.wasm` + `tags.scm` +
`manifest.json`. They load at runtime — `grove init` fetches what your project
needs, or fetch explicitly:

```bash
grove languages      # what's installed locally
grove fetch          # install all grammars from the hosted registry
grove lock           # write grove.lock pinning version + wasm sha256
```

The hosted registry ([Entelligentsia/grove-registry](https://github.com/Entelligentsia/grove-registry))
carries **all 27 official tree-sitter grammars** (c, cpp, c#, go, java, js, ts, tsx,
python, ruby, rust, php, scala, ocaml, ql, and more); `grove fetch` installs them.
The grove repo itself ships a small 3-language dev stub. Adding a language is
dropping a `registry/<lang>/` directory in — the binary doesn't change, and that
includes the *full* surface: the `manifest.json` carries a `profile` (container /
function / identifier node kinds) that drives `parent` grouping, `callers`'
enclosing-function, and go-to-def, so nothing language-specific is compiled in.

```jsonc
// registry/<lang>/manifest.json
{ "name": "go", "version": "…", "extensions": ["go"],
  "profile": {
    "function_kinds": ["function_declaration", "method_declaration"],
    "containers": [["type_declaration", "name"]],
    "identifier_kinds": ["identifier", "field_identifier"]
  } }
```

Build a grammar's wasm with `tree-sitter build --wasm` (emits the `dylink.0`
module the native runtime needs).

## Where grammars live

Grammars are a **cache** — reconstructible from the hosted registry and
content-addressed by `grove.lock` — so the standard home is the OS-native cache
location. grove resolves the registry root by precedence (first existing wins);
`grove registry` shows it:

1. **`GROVE_REGISTRY`** env var — explicit override (CI, tests, air-gapped).
2. **`<project>/.grove/grammars/`** — project-vendored grammars (commit them for
   hermetic / offline builds), found by walking up from the cwd.
3. **OS user cache** — the default shared store:
   - Linux: `~/.cache/grove/grammars` (honors `$XDG_CACHE_HOME`)
   - macOS: `~/Library/Caches/grove/grammars`
   - Windows: `%LOCALAPPDATA%\grove/grammars`
4. **dev source tree** (`registry/` next to this crate) — only in a checkout.

Layout under the root is `<lang>/{grammar.wasm, tags.scm, manifest.json}`.

## Fetching grammars

`grove fetch` pulls grammars from the hosted registry into the OS cache:

```bash
grove fetch                 # all languages in the catalog
grove fetch python rust     # just these
grove fetch python --force  # re-download
```

**grove owns the artifacts it serves.** Rather than redirecting to upstream URLs
at fetch time, grove ingests each grammar (official release wasm where it exists,
else built from source), normalizes it into the shape grove needs — `grammar.wasm`
(native `dylink.0`) + `tags.scm` + a `manifest.json` carrying the node-kind
`profile` — and hosts those bytes content-addressed, recording provenance
(`source.repo` / `source.rev`) for auditability. This guarantees the three travel
as one co-versioned unit and that `grove.lock` always resolves.

The host is the **[Entelligentsia/grove-registry](https://github.com/Entelligentsia/grove-registry)**
repo, split for efficiency: the small text files (`index.json`, per-language
`tags.scm` + `manifest.json`) live in the repo (served via
`raw.githubusercontent.com`), and the heavy `grammar.wasm` binaries are **GitHub
Release assets** (GitHub's CDN). The catalog's `release_base` + per-file `asset`
fields tell `fetch` where each file lives. **Every** file's sha256 is verified
against the catalog before it's written (download-verify-then-write, atomically),
so a corrupted or tampered artifact is rejected. Override the host with
`GROVE_REGISTRY_URL` (self-host, fork, or a local mirror).

## Profiles (why some languages do more)

A grammar's `manifest.json` `profile` decides which tools work fully:

- **Full profile** (~15 languages) — declares `function_kinds` /
  `identifier_kinds`, so `callers` (enclosing function), `definition` from a
  position, and `parent` grouping all work.
- **Minimal profile** (12 languages) — core tools (`outline`, `symbols`,
  `source`, `check`, `map`) work; `callers`/`definition` degrade.
- css/html/json/regex have no upstream `tags.scm` — they still `check` but yield
  no symbols.

Nothing language-specific is compiled into the binary; the profile is data.

## Building the registry (maintainer)

`grove ingest` builds the registry from a curated spec (`registry-sources.json`):
for each grammar it pulls the **official tree-sitter release wasm** + the repo's
`tags.scm` at a pinned rev, attaches grove's curated `profile`/`extensions`, writes
`registry/<lang>/`, and regenerates the catalog.

```bash
grove ingest                 # all grammars in registry-sources.json
grove ingest python rust     # just these
grove index registry         # (re)build index.json with per-file hashes
```

The spec records identity + provenance + the grove-authored profile; the wasm and
tags come from upstream and the version/`source` are pinned. `grove index` then
emits the `index.json` catalog (per language: version, provenance, content hash of
every served file) — the publish step for registry CI.

---

Next: [Tools](tools.md) · [Registry repo →](https://github.com/Entelligentsia/grove-registry)