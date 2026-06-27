; Scopes
(function_definition) @local.scope
(lambda) @local.scope

; Definitions
(parameters (identifier) @local.definition)
(default_parameter name: (identifier) @local.definition)
(assignment left: (identifier) @local.definition)

; References
(identifier) @local.reference
