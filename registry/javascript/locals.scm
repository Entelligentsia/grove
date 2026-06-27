; Scopes
(function_declaration) @local.scope
(function_expression) @local.scope
(arrow_function) @local.scope
(statement_block) @local.scope

; Definitions
(formal_parameters (identifier) @local.definition)
(variable_declarator name: (identifier) @local.definition)

; References
(identifier) @local.reference
