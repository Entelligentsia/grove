; Scopes
(function_item) @local.scope
(closure_expression) @local.scope
(block) @local.scope

; Definitions
(parameter pattern: (identifier) @local.definition)
(let_declaration pattern: (identifier) @local.definition)

; References
(identifier) @local.reference
