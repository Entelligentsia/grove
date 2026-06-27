; from MODULE import NAME
(import_from_statement
  module_name: (_) @import.module
  name: (dotted_name (identifier) @import.name))

; from MODULE import NAME as ALIAS
(import_from_statement
  module_name: (_) @import.module
  (aliased_import
    name: (dotted_name (identifier) @import.source)
    alias: (identifier) @import.name))
