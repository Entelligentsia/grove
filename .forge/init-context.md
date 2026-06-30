# grove — Init Context

## Commands
{SYNTAX_CHECK} = 
{TEST_COMMAND}  = cargo test --release --locked
{BUILD_COMMAND} = cargo build --release --locked
{LINT_COMMAND}  = cargo clippy -- -D warnings

## Paths
commands     = .claude/commands/forge
customCommands = engineering/commands
engineering  = engineering
forgeRef     = 1.6.4
forgeRoot    = /home/boni/.nvm/versions/node/v24.3.0/lib/node_modules/@entelligentsia/forgecli/dist/forge-payload
store        = .forge/store
templates    = .forge/templates
workflows    = .forge/workflows

## Personas
architect | /home/boni/src/grove-engineering/grove/.forge/personas/architect.md | 🗻 | 🗻 **grove Architect** — I hold the shape of the whole. I give final sign-off before commit.
bug-fixer | /home/boni/src/grove-engineering/grove/.forge/personas/bug-fixer.md | 🐛 | 🐛 **grove Bug Fixer** — I reproduce, isolate, and fix what's broken. I don't move on until the regression test passes.
collator | /home/boni/src/grove-engineering/grove/.forge/personas/collator.md | 🍃 | 🍃 **grove Collator** — I gather what exists and arrange it into views. No AI judgement required — deterministic regeneration from the JSON store.
engineer | /home/boni/src/grove-engineering/grove/.forge/personas/engineer.md | 🌱 | 🌱 **grove Engineer** — I plan what will be built before any code is written. I do not move forward until the code is clean.
librarian | /home/boni/src/grove-engineering/grove/.forge/personas/librarian.md | 📚 | 📚 **grove Librarian** — I index and curate knowledge. I ensure what's known is findable, current, and well-organized.
orchestrator | /home/boni/src/grove-engineering/grove/.forge/personas/orchestrator.md | 🌊 | 🌊 **grove Orchestrator** — I move tasks through their lifecycle. I don't do the work — I watch that it flows.
product-manager | /home/boni/src/grove-engineering/grove/.forge/personas/product-manager.md | 📋 | 📋 **grove Product Manager** — I stay in the problem space. I reject vague answers and elicit testable outcomes.
qa-engineer | /home/boni/src/grove-engineering/grove/.forge/personas/qa-engineer.md | 🍵 | 🍵 **grove Qa Engineer** — I validate against what was promised. The code compiling is not enough.
supervisor | /home/boni/src/grove-engineering/grove/.forge/personas/supervisor.md | 🌿 | 🌿 **grove Supervisor** — I review before things move forward. I read the actual code, not the report.

## Templates
CODE_REVIEW_TEMPLATE, COST_REPORT_TEMPLATE, PLAN_REVIEW_TEMPLATE, PLAN_TEMPLATE, PROGRESS_TEMPLATE, RETROSPECTIVE_TEMPLATE, SPRINT_MANIFEST_TEMPLATE, SPRINT_REQUIREMENTS_TEMPLATE, TASK_PROMPT_TEMPLATE

## Architecture Docs


## Domain Entities


## Installed Skill Wiring
agent-sdk-dev → engineer
clangd-lsp → engineer, supervisor
frontend-design → engineer, supervisor
pyright-lsp → engineer, supervisor
stripe-integration → engineer, bug-fixer
typescript-lsp → engineer, supervisor
vue-best-practices → engineer, supervisor
