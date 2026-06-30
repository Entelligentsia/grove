# Skill Generation — Per-Subagent Instructions

You are generating **ONE** skill file. You have been given two inputs:

1. A **project brief** (`.forge/init-context.md`) — authoritative for all names,
   paths, command values, and installed-skill wiring
2. A **meta-skill source** — the role's universal capability set and Generation Instructions

Your job is to produce exactly one file and return a one-line status.

---

## Inputs

Read these two sources before writing anything:

- `$FORGE_ROOT/meta/skills/{meta}` — the meta-skill for your assigned role
- `.forge/init-context.md` — the project brief (passed inline in your prompt)

---

## Rules

1. **Write EXACTLY ONE file:** `.forge/skills/{role}-skills.md`

2. **Project interpolation** — replace every generic placeholder with the
   project's actual tools, libraries, paths, and conventions from the brief.
   All entity names, stack names, and command values MUST come from the brief.
   Do not invent values.

3. **Stack specificity** — replace abstract references (e.g. "your test runner",
   "your ORM") with the actual tools discovered for this project. Substitute
   actual command values from the brief's `## Commands` section.

4. **Installed skill integration** — read `## Installed Skill Wiring` from the
   brief. For each skill that maps to this role, add an explicit reference
   explaining how the persona should combine the marketplace skill with this
   project-specific skill set.

5. **Follow Generation Instructions** — each meta-skill has a
   `## Generation Instructions` section. Follow it fully. Do not add sections,
   steps, or notes beyond what it prescribes.

6. **No free-form additions** — produce only the sections the meta-skill defines.

---

## Self-check (mandatory last step)

After writing the file, verify before returning:

1. Read back `.forge/skills/{role}-skills.md`
2. Confirm **no unsubstituted placeholders** remain
3. Confirm **no abstract project references** remain (e.g. "your framework",
   "your ORM" — these must be replaced with the actual project values)
4. Record in the manifest:
   ```sh
   node "$FORGE_ROOT/tools/generation-manifest.cjs" record ".forge/skills/{role}-skills.md"
   ```
   (If generation-manifest.cjs is not yet installed, skip silently.)

5. Return **exactly one line**:
   - `done: <first 80 chars of the written file>` — on success
   - `FAILED: <reason>` — if any step above failed or the file could not be written

Do not output anything else after the status line.
