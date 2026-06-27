; Scopes
(function_item) @local.scope
(closure_expression) @local.scope
(block) @local.scope

; Definitions (subtyped captures — the engine prefix-matches `local.definition*`)
(parameter pattern: (identifier) @local.definition.parameter)
(let_declaration pattern: (identifier) @local.definition.var)

; References
(identifier) @local.reference
