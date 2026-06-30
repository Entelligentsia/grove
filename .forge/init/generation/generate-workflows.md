# Workflow Generation — Per-Subagent Instructions

You are generating **ONE** workflow file. You have been given three inputs:

1. A **project brief** (`.forge/init-context.md`) — authoritative for all names,
   paths, and placeholder values
2. A **meta-workflow source** — your generation algorithm and Generation Instructions
3. A **persona file** (already generated) — embed verbatim as the opening section

Your job is to produce exactly one file and return a one-line status.

---

## Inputs

Read these three sources before writing anything:

- `.forge/init-context.md` — the project brief (passed inline in your prompt)
- `$FORGE_ROOT/meta/workflows/{meta}` — the meta-workflow for your assigned workflow
- `.forge/personas/{persona}.md` — the persona file for your assigned role

---

## Rules

1. **Write EXACTLY ONE file:** `.forge/workflows/{id}.md`

2. **Opening section** — the generated file must begin with:
   a. The meta-workflow's YAML frontmatter block (`---` … `---`) verbatim,
      if present. This carries `requirements:` fields used for runtime model
      selection and must be preserved exactly.
   b. Immediately after the closing `---`, embed the persona file content
      verbatim as the first section. Do not paraphrase or summarise.

3. **Placeholder substitution** — all `{SYNTAX_CHECK}`, `{TEST_COMMAND}`,
   `{BUILD_COMMAND}`, `{LINT_COMMAND}` values come from the brief's
   `## Commands` section. Do not invent values.

4. **Name vocabulary** — all persona names, template names, architecture doc
   names, and entity names MUST appear in the brief. Do not invent or rename
   anything not listed there.

5. **Required steps** — follow the meta-workflow's Generation Instructions.
   Include the Knowledge Writeback step and the Event Emission step as defined
   in the meta-workflow (unless the meta explicitly omits them, e.g. quiz_agent).

6. **Enforcement quality** — for review workflows, include:
   - A Rationalization Table of common agent excuses and factual rebuttals
   - "YOU MUST" / "No exceptions" gate language at critical checks
   - An announcement pattern: the agent declares intent at workflow start

7. **No free-form additions** — do not add sections, steps, or notes beyond
   what the meta-workflow and project brief prescribe.

---

## Self-check (mandatory last step)

After writing the file, verify before returning:

1. Read back `.forge/workflows/{id}.md`
2. Confirm the persona symbol appears as the **first non-blank line after the
   closing `---`** of the frontmatter block (or as the absolute first non-blank
   line if the meta-workflow had no frontmatter). The symbol is listed in the
   brief's `## Personas` section for this workflow's role.
3. Confirm **no unsubstituted placeholders** remain (no literal `{TEST_COMMAND}`,
   `{BUILD_COMMAND}`, `{SYNTAX_CHECK}`, or `{LINT_COMMAND}` in the file)
4. Record in the manifest:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record ".forge/workflows/{id}.md"
   ```
   (If generation-manifest.cjs is not yet installed, skip silently.)

5. Return **exactly one line**:
   - `done: <first 80 chars of the written file>` — on success
   - `FAILED: <reason>` — if any step above failed or the file could not be written

Do not output anything else after the status line.
