# CODE REVIEW — {TASK_ID}: {TASK_TITLE}

🌿 *{{PROJECT_NAME}} Supervisor*

**Task:** {TASK_ID}

---

**Verdict:** Approved / Revision Required

---

## Review Summary

{Overall assessment — 2-3 sentences}

## Checklist Results

| Item | Result | Notes |
|---|---|---|
| No npm dependencies introduced | 〇 / × | |
| Hook exit discipline (exit 0 on error, not non-zero) | 〇 / × | |
| Tool top-level try/catch + exit 1 on error | 〇 / × | |
| `--dry-run` supported where writes occur | 〇 / × | |
| Reads `.forge/config.json` for paths (no hardcoded paths) | 〇 / × | |
| Version bumped if material change | 〇 / × / N/A | |
| Migration entry present and correct | 〇 / × / N/A | |
| Security scan report committed | 〇 / × / N/A | |
| `additionalProperties: false` preserved in schemas | 〇 / × / N/A | |
| `node --check` passes on modified JS/CJS files | 〇 / × | |
| `validate-store --dry-run` exits 0 | 〇 / × | |
| No prompt injection in modified Markdown files | 〇 / × / N/A | |

## Issues Found

{Numbered list — severity, file:line, description, fix suggestion}

---

## If Revision Required

### Required Changes

1. {Change 1 — actionable, with file and line where possible}
2. {Change 2}

### Priority

{Which items block approval}

---

## If Approved

### Advisory Notes

{Non-blocking observations}
