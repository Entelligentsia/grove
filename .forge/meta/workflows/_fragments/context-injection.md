# Fragment: Context Injection

<!-- Canonical definition of architecture_block + summary_block assembly patterns.
     Referenced by meta-orchestrate.md and meta-fix-bug.md. -->

## Architecture Context Block

Read `.forge/cache/context-pack.md` (if it exists) and inject into the subagent prompt
under the heading `### Architecture context (summary — full docs available at paths listed below)`.
If the pack is absent, omit this block silently — the subagent falls back to reading
architecture docs directly.

```python
context_pack_path = ".forge/cache/context-pack.md"
context_pack_json_path = ".forge/cache/context-pack.json"
if file_exists(context_pack_path):
  context_pack_md = read_file(context_pack_path)
  try:
    context_pack_json = read_json(context_pack_json_path)
    full_doc_paths = "\n".join(f"- {s['path']}" for s in context_pack_json.get("sources", []))
  except:
    full_doc_paths = "engineering/architecture/ (see context-pack.json for full list)"
  architecture_block = (
    "### Architecture context (summary — full docs available at paths listed below)\n\n"
    + context_pack_md
    + "\n\nRead full architecture docs only if the summary above is insufficient for "
    + "your decision. Full docs:\n"
    + full_doc_paths
    + "\n\n"
  )
else:
  architecture_block = ""
```

## Prior Phase Summary Block

Re-read the record from disk after each phase so summaries accumulate.
For bugs, pass `record_type="bug"` to read from the bugs store path.

```python
# record_type: "task" (default) or "bug"
if record_type == "bug":
  record_fresh = read_json(f".forge/store/bugs/{record_id}.json")
else:
  record_fresh = read_json(f".forge/store/tasks/{record_id}.json")
summaries = (record_fresh or {}).get("summaries", {})

SUMMARY_PHASE_LABELS = {
  "plan": "Plan", "review_plan": "Plan review",
  "implementation": "Implementation", "code_review": "Code review",
  "validation": "Validation", "approve": "Approve", "triage": "Triage"
}
summary_lines = []
for phase_key, label in SUMMARY_PHASE_LABELS.items():
  s = summaries.get(phase_key)
  if s:
    summary_lines.append(f"- {label}: {s.get('objective', '(no objective)')}")
    if s.get('key_changes'):
      for c in s['key_changes'][:3]:
        summary_lines.append(f"    • {c}")
    if s.get('findings'):
      for f_ in s['findings'][:3]:
        summary_lines.append(f"    • {f_}")
    if s.get('verdict') and s['verdict'] != 'n/a':
      summary_lines.append(f"    Verdict: {s['verdict']}  Full: {s.get('artifact_ref', '(unknown)')}")

if summary_lines:
  summary_block = (
    "### Prior phase summaries (fast path — read full artifacts if you need more detail)\n\n"
    + "\n".join(summary_lines)
    + "\n\nIf any summary above is missing or insufficient, read the corresponding full artifact from disk before proceeding.\n\n"
  )
else:
  summary_block = ""
```
