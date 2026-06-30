# Template Generation — Per-Subagent Instructions

You are generating **ONE** template file. You have been given two inputs:

1. A **project brief** (`.forge/init-context.md`) — authoritative for all names,
   paths, command values, and entity names
2. A **meta-template source** — the document structure and Generation Instructions

Your job is to produce exactly one file and return a one-line status.

---

## Inputs

Read these two sources before writing anything:

- `$FORGE_ROOT/meta/templates/{meta}` — the meta-template for your assigned document
- `.forge/init-context.md` — the project brief (passed inline in your prompt)

---

## Rules

1. **Write EXACTLY ONE file:** `.forge/templates/{filename}.md`

2. **Stack-specific sections** — add framework-specific subsections based on the
   stack information in the brief. Use the detected languages and frameworks to
   decide which optional sections to include.

3. **Project substitution** — replace all `{Project}`, `{PREFIX}`, entity name
   placeholders, and ID format examples with actual values from the brief.
   All entity names and command values MUST come from the brief. Do not invent.

4. **Test output formats** — reference the project's actual test runner output
   format in any template sections that involve test results.

5. **Follow Generation Instructions** — each meta-template has a
   `## Generation Instructions` section. Follow it fully. Do not add sections
   or notes beyond what it prescribes.

---

## Self-check (mandatory last step)

After writing the file, verify before returning:

1. Read back `.forge/templates/{filename}.md`
2. Confirm **no unsubstituted placeholders** remain (no literal `{Project}`,
   `{PREFIX}`, `{TEST_COMMAND}`, etc.)
3. Record in the manifest:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record ".forge/templates/{filename}.md"
   ```
   (If generation-manifest.cjs is not yet installed, skip silently.)

4. Return **exactly one line**:
   - `done: <first 80 chars of the written file>` — on success
   - `FAILED: <reason>` — if any step above failed or the file could not be written

Do not output anything else after the status line.
