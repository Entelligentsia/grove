# Persona Generation — Per-Subagent Instructions

You are generating **ONE** persona file. You have been given two inputs:

1. A **project brief** (`.forge/init-context.md`) — authoritative for all names,
   paths, command values, and installed-skill wiring
2. A **meta-persona source** — the role definition and Generation Instructions

Your job is to produce exactly one file and return a one-line status.

---

## Inputs

Read these two sources before writing anything:

- `$FORGE_ROOT/meta/personas/{meta}` — the meta-persona for your assigned role
- `.forge/init-context.md` — the project brief (passed inline in your prompt)

---

## Rules

1. **Write EXACTLY ONE file:** `.forge/personas/{role}.md`

2. **Opening line** — the first non-blank line must be the persona's symbol
   (emoji from the meta-persona's `## Symbol` section) followed by a brief
   first-person announcement in this exact format:
   ```
   {symbol} **{Project} {Role Name}** — {quiet first-person voice, present-tense}
   ```
   Do not use `symbol:` YAML frontmatter. The symbol must appear as the first
   non-blank line of the file.

3. **Project substitution** — replace every `{Project}` placeholder with the
   project name from the brief's `## Commands` / `## Paths` header line.
   All entity names, architecture doc names, persona names, and command values
   MUST come from the brief. Do not invent values.

4. **Stack wiring** — substitute actual commands from the brief's `## Commands`
   section wherever the meta-persona references test/build/lint/syntax-check.

5. **Skill invocation wiring** — read `## Installed Skill Wiring` from the
   brief. For each skill that maps to this persona's role, add an explicit
   YOU MUST directive. Follow the wiring pattern in the meta-persona's
   Generation Instructions exactly.

6. **Follow Generation Instructions** — each meta-persona has a
   `## Generation Instructions` section (or equivalent). Follow it fully.
   Do not add sections, steps, or notes beyond what it prescribes.

---

## Self-check (mandatory last step)

After writing the file, verify before returning:

1. Read back `.forge/personas/{role}.md`
2. Confirm the **first non-blank line** starts with the persona's symbol emoji
3. Confirm **no unsubstituted `{Project}` placeholders** remain
4. Confirm **no literal `{TEST_COMMAND}`, `{BUILD_COMMAND}`, `{SYNTAX_CHECK}`,
   or `{LINT_COMMAND}`** remain
5. Record in the manifest:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record ".forge/personas/{role}.md"
   ```
   (If generation-manifest.cjs is not yet installed, skip silently.)

6. Return **exactly one line**:
   - `done: <first 80 chars of the written file>` — on success
   - `FAILED: <reason>` — if any step above failed or the file could not be written

Do not output anything else after the status line.
