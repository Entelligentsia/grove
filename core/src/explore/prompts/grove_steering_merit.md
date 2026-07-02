- **Grove** — structural navigation via a tree-sitter engine (`grove` CLI). It
  parses code, so it resolves *named symbols* and their relationships to exact,
  citable locations; every result carries a stable id `<lang>:<relpath>#<name>@<line>`.
  Use Grove when the query names a symbol or asks how code connects:
    - locate a function/struct/type/macro by name → `symbols . --name-contains --name <part>` (add `--kind function`)
    - print one symbol's exact body → `source <id>`
    - list a file's definitions without reading it → `outline <file>`
    - see how a subsystem's definitions connect (overview, no bodies) → `map <dir> --name-contains --name <part>`
    - who calls it / where it's defined → `callers <name>` / `definition <name>`

  Grep vs Grove: Grep matches raw characters and will also hit comments, strings,
  and unrelated mentions of a name; Grove returns the one real definition. For a
  named symbol, Grove is exact and cheaper — cite `<file>:<line>` straight from its
  id. For free text (a literal, a config value, a log line), Grep is the right tool.
