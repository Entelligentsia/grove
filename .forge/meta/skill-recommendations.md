# Forge Skill Recommendations

Maps discovered stack components to verified Claude Code marketplace skills.
Used during `forge:init` (Phase 2) and `forge:health` to recommend skills
that complement Forge's project-specific knowledge.

## Skill Sources

Skills come from two locations — both are checked by `tools/list-skills.js`:

| Source | Location | Install mechanism |
|---|---|---|
| Marketplace plugin | `~/.claude/plugins/installed_plugins.json` | `/plugin install <name>@<marketplace>` |
| Personal skill | `~/.claude/skills/<name>/SKILL.md` | Manual — place SKILL.md in directory |

Marketplace install format: `/plugin install <skill-name>@<marketplace>`

Personal skills cannot be installed via `/plugin install`. If a recommended personal
skill is not present, surface it in the health report with its source noted as "personal skill".

---

## Why This Matters

Forge generates project-specific context: your entities, auth patterns,
business rules, conventions. Marketplace skills provide universal technique
knowledge: LSP intelligence for your language, frontend design patterns, etc.
A Forge-generated persona that invokes a relevant skill gets both layers —
project knowledge AND technique depth — without duplication.

---

## Language Servers (LSP)

High-confidence for any project using the matched language. LSP skills give
agents go-to-definition, find-references, static analysis, and real-time
diagnostics, dramatically reducing hallucinated API usage.

| If stack language includes | Skill | Marketplace | Install |
|---|---|---|---|
| TypeScript, JavaScript | `typescript-lsp` | claude-plugins-official | `/plugin install typescript-lsp@claude-plugins-official` |
| Python | `pyright-lsp` | claude-plugins-official | `/plugin install pyright-lsp@claude-plugins-official` |
| Ruby | `ruby-lsp` | claude-plugins-official | `/plugin install ruby-lsp@claude-plugins-official` |
| Go | `gopls-lsp` | claude-plugins-official | `/plugin install gopls-lsp@claude-plugins-official` |
| Rust | `rust-analyzer-lsp` | claude-plugins-official | `/plugin install rust-analyzer-lsp@claude-plugins-official` |
| Java | `jdtls-lsp` | claude-plugins-official | `/plugin install jdtls-lsp@claude-plugins-official` |
| Kotlin | `kotlin-lsp` | claude-plugins-official | `/plugin install kotlin-lsp@claude-plugins-official` |
| C# / .NET | `csharp-lsp` | claude-plugins-official | `/plugin install csharp-lsp@claude-plugins-official` |
| C, C++ | `clangd-lsp` | claude-plugins-official | `/plugin install clangd-lsp@claude-plugins-official` |
| PHP | `php-lsp` | claude-plugins-official | `/plugin install php-lsp@claude-plugins-official` |
| Swift, iOS | `swift-lsp` | claude-plugins-official | `/plugin install swift-lsp@claude-plugins-official` |
| Lua | `lua-lsp` | claude-plugins-official | `/plugin install lua-lsp@claude-plugins-official` |

---

## Frontend & UI

| If stack contains | Skill | Source | Confidence | Why |
|---|---|---|---|---|
| Any web UI, React, Vue, Svelte | `frontend-design` | `claude-plugins-official` | High | Production-grade UI, distinctive design, avoids generic AI aesthetics |
| Vue, Nuxt, Pinia, Vue Router | `vue-best-practices` | personal (`~/.claude/skills/`) | High | Composition API, `<script setup>`, TypeScript, SSR, Volar, vue-tsc |

---

## 3D Graphics & XR

| If stack contains | Skill | Marketplace | Why |
|---|---|---|---|
| Three.js | `threejs-skills` | agentic-skills | 10 skills: scene, geometry, materials, lighting, textures, animation, shaders, post-processing |
| WebXR, Meta Quest | `meta-webxr-skills` | agentic-skills | 8 skills: XR session lifecycle, rendering, input, passthrough, anchors, PWA packaging |

---

## Security

| Signal | Skill | Confidence | Why |
|---|---|---|---|
| Any project | `security-guidance` | Medium | Hook-based warnings for command injection, XSS, unsafe patterns during file edits |

---

## Payments & Commerce

| If stack contains | Skill | Source | Confidence | Why |
|---|---|---|---|---|
| Stripe, stripe-js, stripe-python | `stripe-integration` | personal (`~/.claude/skills/`) | High | PCI-compliant checkout, subscriptions, webhook handling, idempotency |

---

## Support & CRM

| If stack contains | Skill | Source | Confidence | Why |
|---|---|---|---|---|
| Freshdesk | `freshdesk-api` | personal (`~/.claude/skills/`) | High | Ticket management, contacts, companies, support workflow automation |

---

## MCP & AI Integration

| If stack contains | Skill | Source | Confidence | Why |
|---|---|---|---|---|
| MCP server code | `mcp-server-dev` | `claude-plugins-official` | High | Deployment models, tool design, auth, interactive MCP apps |
| Anthropic SDK, Claude API | `agent-sdk-dev` | `claude-plugins-official` | High | Claude Agent SDK patterns, tool use, multi-agent orchestration |

---

## Workflow Plugins (use with awareness of Forge overlap)

These plugins provide workflow automation. They overlap with Forge's own
Supervisor and Architect workflows. **Do not recommend during init** — let
the user decide after reviewing Forge's generated workflows.

Surface these in `forge:health` only if the relevant Forge workflow is absent
or the user explicitly asks.

| Plugin | What it does | Overlap with Forge |
|---|---|---|
| `code-review` | Multi-agent PR code review with confidence scoring | Duplicates Supervisor review workflow |
| `pr-review-toolkit` | Specialized agents for tests, error handling, type design | Duplicates Supervisor review workflow |
| `commit-commands` | `/commit`, `/push`, `/pr` slash commands | Duplicates `meta-commit` workflow |
| `feature-dev` | Feature development with exploration + architecture agents | Overlaps with Orchestrator pipeline |
| `claude-code-setup` | Analyzes codebase and recommends hooks, skills, MCP servers | Complementary to `forge:init` |
| `claude-md-management` | Audits and improves CLAUDE.md files | Complementary — runs alongside Forge |

---

## Confidence Levels

- **High** — primary language or framework detected; install without hesitation
- **Medium** — relevant but not central to the stack; user should decide
- **Low** — generally useful; surface in health report, not init prompt

---

## Persona Integration Pattern

When a skill is installed, Forge-generated personas should invoke it explicitly
at the relevant workflow step — not mention it in a notes section.

Template (fill in skill name and trigger context):

```
When [doing X], YOU MUST invoke the `<skill-name>` skill before proceeding.
That skill provides universal [domain] technique knowledge; the stack checklist
provides project-specific conventions. Both layers are required. No exceptions.
```

Apply this to whichever workflow step overlaps with the skill's domain:
- Architect reviewing a plan → invoke LSP skill + domain technique skill
- Supervisor reviewing implementation → invoke LSP skill + domain technique skill
- Engineer implementing → invoke LSP skill at task start

LSP skills in particular should be wired into every Engineer workflow that
touches the language, not just review steps.
