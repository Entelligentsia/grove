# Fragment: Progress Reporting

<!-- Canonical progress log format and store-cli commands for orchestrator subagents.
     Referenced by meta-orchestrate.md and meta-fix-bug.md. -->

Each subagent writes progress entries to a transient log file that the
orchestrator monitors in real time.

**Log path:** `.forge/store/events/{sprintId}/progress.log`

**Format per line:**

```
{ISO_TIMESTAMP}|{agent_name}|{banner_key}|{status}|{detail}
```

| Field | Format | Example |
|-------|--------|---------|
| `ISO_TIMESTAMP` | ISO 8601 UTC | `2026-04-16T14:15:23Z` |
| `agent_name` | `{taskId}:{persona_noun}:{phase.role}:{iteration}` | `FORGE-S09-T01:engineer:plan:1` |
| `banner_key` | Banner identity key from BANNER_MAP | `forge` |
| `status` | One of: `start`, `progress`, `done`, `error` | `progress` |
| `detail` | Free text (no pipe characters) | `Reading codebase` |

**Writing entries:** Use `forge_store`:

```
forge_store({ command: "progress", args: ["{sprintId}", "{agentName}", "{bannerKey}", "{status}", "detail text"] })
```

**Monitoring:** The orchestrator starts a Monitor on the progress log before
spawning each subagent and stops it after the subagent returns.

**Clearing:** The orchestrator clears the progress log at task start:

```
forge_store({ command: "progress-clear", args: ["{sprintId}"] })
```
