; import { NAME } from "MODULE"
(import_statement
  (import_clause
    (named_imports
      (import_specifier name: (identifier) @import.name !alias)))
  source: (string (string_fragment) @import.module))

; import { NAME as ALIAS } from "MODULE"
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @import.source
        alias: (identifier) @import.name)))
  source: (string (string_fragment) @import.module))
