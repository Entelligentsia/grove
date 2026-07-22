//! `grove init` — make a project one where the agent actually uses grove.
//!
//! Two halves, per VISION §6.4.1: registration (`.mcp.json`) makes the tools
//! *available*; a steering directive (`CLAUDE.md`) makes them *adopted*. Plus a
//! `grove.lock` pinning the grammars the project needs. Idempotent: re-running
//! updates the grove pieces without clobbering anything else.

use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde_json::{json, Value};

use grove_core::config::{GroveConfig, Mode};
use grove_core::harness::{
    self, GROVE_START as CLAUDE_START, GROVE_END as CLAUDE_END, HarnessId, McpFormat, Scope,
    MCP_SERVER_KEY, EXPLORE_SERVER_KEY,
};
use grove_core::init::provision_project;
use grove_core::registry;
use grove_explore_core::ExploreConfig;

/// Which integration `grove init` wires up. Grammar provisioning (fetch +
/// `grove.lock`) happens for every target; this only selects the harness glue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Target {
    /// Register the MCP server (`.mcp.json`) + a CLAUDE.md steering block.
    #[default]
    Mcp,
    /// Grammars + a CLAUDE.md steering block (so a cold agent actually reaches
    /// for grove instead of grep). The skill *artifact* is distributed via the
    /// skills tool (`npx skills add Entelligentsia/grove`); init provisions and
    /// steers — steering to the grove skill / CLI rather than MCP tools.
    Skill,
    /// Both of the above.
    Both,
    /// Register BOTH `grove` and `grove-explore` in `.mcp.json` + dual-surface
    /// steering blocks in `CLAUDE.md` and `AGENTS.md` naming `mcp__grove__*` and
    /// `mcp__grove__explore`. Shells out to `grove-explore config` for the
    /// first-run TUI (when `.grove/config.json`'s explore section is absent);
    /// skips it on re-runs and `--dry-run`.
    McpLlm,
    /// Grammars + `grove.lock` only — no `.mcp.json`, no CLAUDE.md steering.
    /// For embedding hosts (e.g. an editor or agent runtime) that register
    /// grove's tools themselves and supply their own steering; the project's
    /// own files are left untouched.
    Grammars,
}

impl Target {
    fn writes_mcp(self) -> bool {
        matches!(self, Target::Mcp | Target::Both)
    }
    fn is_skill(self) -> bool {
        matches!(self, Target::Skill | Target::Both)
    }
    /// Whether this target writes a CLAUDE.md steering block. Every target but
    /// `Grammars` does — `Grammars` provisions grammars + the lock and nothing
    /// else, leaving project-owned files (CLAUDE.md, `.mcp.json`) untouched.
    fn writes_steering(self) -> bool {
        !matches!(self, Target::Grammars)
    }

    /// Bridge from the CLI `Target` enum (clap-facing) to the store `Mode`
    /// enum (config-facing). 1-to-1 mapping.
    pub fn to_mode(self) -> Mode {
        match self {
            Target::Mcp => Mode::Mcp,
            Target::Skill => Mode::Skill,
            Target::Both => Mode::Both,
            Target::McpLlm => Mode::McpLlm,
            Target::Grammars => Mode::Grammars,
        }
    }
}


pub fn run(root: &Path, target: Target, agents: Option<String>, dry_run: bool) -> Result<()> {
    println!("grove init  scanning {} (as {:?})\n", root.display(), target);

    // Load prior config (old mode) — used for reconcile_harness and explore-config preservation.
    let old_cfg = GroveConfig::load(root).ok();
    let old_mode: Option<Mode> = old_cfg.as_ref().map(|c| c.mode);

    // Resolve which coding agents to wire (see `resolve_harness_set`): the
    // `--agents` override, else a preserved set from a prior run, else auto-detect.
    let harnesses = resolve_harness_set(agents.as_deref(), old_cfg.as_ref(), root)?;
    let new_mode = target.to_mode();
    // Global-scope harnesses (Codex → ~/.codex) resolve under the home dir.
    let home = dirs::home_dir().unwrap_or_else(|| root.to_path_buf());
    println!(
        "  agents      {}\n",
        harnesses.iter().map(|h| h.display_name()).collect::<Vec<_>>().join(", ")
    );

    // PATH-aware first-run guard for McpLlm: the TUI is delegated to
    // `grove-explore config`, which requires both the sibling binary on PATH
    // and an interactive terminal. Re-runs and --dry-run bypass this guard so
    // CI re-runs are never blocked after the first interactive session.
    //
    // "Already configured" means any of: the live config declares mcp-llm, the
    // live config carries an explore section, or the deprecated
    // `.grove/explore.json` is still present (pre-ADR-0002 projects, which
    // GroveConfig::load migrates on read).
    let already_configured = old_mode == Some(Mode::McpLlm)
        || old_cfg.as_ref().is_some_and(|c| c.explore.is_some())
        || root.join(".grove").join("explore.json").exists();
    let explore_on_path = explore_binary_on_path().is_some();
    if target == Target::McpLlm
        && !dry_run
        && !already_configured
        && explore_on_path
        && !std::io::stdout().is_terminal()
    {
        anyhow::bail!(
            "`grove init --as mcp-llm` requires an interactive terminal for the \
             first-run configuration. Run it in a real terminal, or pre-create \
             `.grove/config.json` with an `explore` section to skip the TUI."
        );
    }
    // When grove-explore is absent from PATH we degrade: continue to
    // register both servers and print a setup hint at the end.

    // Provision grammars + the lock (clap-free core). An empty return is the
    // contract for "nothing provisioned" — a dry-run / no-files / no-cached-
    // grammars short-circuit; core already printed its terminal line, so stop.
    let provisioned = provision_project(root, dry_run)?;

    // Dry-run: preview every harness file that would be written (per selected
    // agent + shared steering), flagging user-global targets, BEFORE the
    // is_empty() early-return that provision_project triggers on --dry-run.
    if dry_run {
        for t in harness_targets(root, &home, new_mode, &harnesses) {
            println!("  would write  {t}");
        }
    }

    if provisioned.is_empty() {
        return Ok(());
    }

    // First-run TUI: delegate to `grove-explore config` when the sibling is on
    // PATH and stdout is a TTY. If it is absent from PATH, degrade gracefully:
    // reconcile the harness registrations and tell the user how to finish setup.
    let mut explore_missing_degrade = false;
    // Shares `already_configured` with the guard above — checking a different
    // condition here (it previously tested only for the deprecated
    // `.grove/explore.json`) made the two disagree, so a project configured via
    // `.grove/config.json`'s explore section cleared the first guard and was
    // then rejected by this one.
    if target == Target::McpLlm && !dry_run && !already_configured {
        if let Some(bin) = explore_binary_on_path() {
            if !std::io::stdout().is_terminal() {
                // Defensive: the guard above already catches this for full
                // installs, but re-checking keeps the matrix explicit.
                anyhow::bail!(
                    "`grove init --as mcp-llm` requires an interactive terminal for the \
                     first-run configuration. Run it in a real terminal, or pre-create \
                     `.grove/config.json` with an `explore` section to skip the TUI."
                );
            }
            let status = std::process::Command::new(&bin)
                .arg("config")
                .arg(root)
                .status()
                .with_context(|| format!("launching {} config", bin.display()))?;
            if !status.success() {
                anyhow::bail!("grove-explore config exited with status {:?}", status.code());
            }
        } else {
            explore_missing_degrade = true;
        }
    }

    // Reconcile the harness glue to match new_mode, then extend with provisioning
    // actions — preserving report order: registrations, `CLAUDE.md`, `grove.lock`.
    let mut wrote = reconcile_harness_for(root, &home, old_mode, new_mode, &harnesses)?;
    wrote.extend(provisioned);

    // Save the new config.json so active_mode reflects the chosen target.
    let explore = if target == Target::McpLlm {
        // Prefer config.json (TUI writes here, already a Value); fall back to
        // legacy explore.json (convert the typed ExploreConfig to Value), then
        // to the explore section of the pre-run config.
        GroveConfig::load(root).ok().and_then(|c| c.explore)
            .or_else(|| {
                ExploreConfig::load(root).ok()
                    .and_then(|ec| serde_json::to_value(ec).ok())
            })
            .or_else(|| old_cfg.as_ref().and_then(|c| c.explore.clone()))
    } else {
        // Preserve existing explore config across mode switches.
        old_cfg.as_ref().and_then(|c| c.explore.clone())
    };
    // Persist the harness set resolved above alongside the mode.
    let new_cfg = GroveConfig { version: 1, mode: new_mode, explore, harnesses };
    new_cfg.save(root)?;

    println!("\n  wrote");
    for w in &wrote {
        println!("             ✓ {w}");
    }
    if target == Target::McpLlm {
        println!("\n  ready      one surface registered:");
        println!("             mcp__grove__explore — LLM-backed code locator (file:line citations)");
        println!("\n             Ask it broad 'where is X' questions; read the lines it cites.");
        println!("             For the structural tools instead, use `grove init --as mcp`.");
        if explore_missing_degrade {
            println!("\n  setup      run `grove-explore config` to finish setup");
        }
    } else if target.writes_mcp() {
        println!("\n  ready      your agent now has grove's tools across its loop:");
        println!("             outline · symbols · source · callers · map · definition · check");
        println!("\n  try it     start a fresh Claude Code session here and ask a");
        println!("             \"where/what/who-calls\" question — it routes through grove.");
    }
    if target.is_skill() {
        println!("\n  skill      grammars + steering are ready. Install the cross-harness");
        println!("             skill with:  npx skills add Entelligentsia/grove");
        println!("             (it steers your agent to grove's MCP tools when present,");
        println!("              else the grove CLI — same engine either way.)");
    }
    if !target.writes_steering() {
        println!("\n  grammars   provisioned + pinned (grove.lock). No project files were");
        println!("             modified — the embedding host registers grove's tools and");
        println!("             supplies its own steering.");
    }
    Ok(())
}

/// Reconcile all three harness artifacts (`.mcp.json`, `CLAUDE.md`, `AGENTS.md`)
/// toward `new_mode`. The `old_mode` parameter is accepted for forward-compatibility
/// (future skip-if-already-correct optimisations) but the reconciliation always
/// drives from on-disk state to `new_mode`. Stripped artifacts are silently
/// reconciled (no entry in the returned list); only written artifacts are listed.
/// Resolve the harness set to wire, in precedence order:
/// 1. an explicit `--agents` spec (`auto` / `all` / a comma list of slugs);
/// 2. otherwise a set preserved from a prior `grove init` (idempotent re-runs);
/// 3. otherwise auto-detect the agents in use here (falling back to Claude Code).
fn resolve_harness_set(
    spec: Option<&str>,
    old: Option<&GroveConfig>,
    root: &Path,
) -> Result<Vec<HarnessId>> {
    if let Some(s) = spec {
        return parse_agents_spec(s, root);
    }
    if let Some(cfg) = old {
        if !cfg.harnesses.is_empty() {
            return Ok(cfg.harnesses.clone());
        }
    }
    Ok(auto_detect_or_default(root))
}

/// Parse an `--agents` value: `auto` (detect), `all` (every harness), or a
/// comma-separated list of slugs. Unknown slugs error with the legal list.
fn parse_agents_spec(spec: &str, root: &Path) -> Result<Vec<HarnessId>> {
    let s = spec.trim();
    if s.eq_ignore_ascii_case("auto") {
        return Ok(auto_detect_or_default(root));
    }
    if s.eq_ignore_ascii_case("all") {
        return Ok(HarnessId::ALL.to_vec());
    }
    let mut out: Vec<HarnessId> = Vec::new();
    for part in s.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let h = HarnessId::from_slug(p)?;
        if !out.contains(&h) {
            out.push(h);
        }
    }
    if out.is_empty() {
        anyhow::bail!(
            "`--agents` listed no agents; expected `auto`, `all`, or a comma list of {}",
            HarnessId::legal_slugs().join(", ")
        );
    }
    Ok(out)
}

/// Auto-detected harnesses, or Claude Code when none are detected (preserving
/// the historical behavior of a bare `grove init`).
fn auto_detect_or_default(root: &Path) -> Vec<HarnessId> {
    let detected = detect_harnesses(root);
    if detected.is_empty() {
        grove_core::config::default_harnesses()
    } else {
        detected
    }
}

/// Every harness that appears to be in use for `root`: its CLI is on `PATH`
/// (via `which`) or its project marker directory exists (e.g. `.cursor/`).
fn detect_harnesses(root: &Path) -> Vec<HarnessId> {
    HarnessId::ALL
        .iter()
        .copied()
        .filter(|&h| harness_present(h, root))
        .collect()
}

fn harness_present(h: HarnessId, root: &Path) -> bool {
    if let Some(marker) = h.detect_marker(root) {
        if marker.exists() {
            return true;
        }
    }
    h.detect_bins().iter().any(|bin| which::which(bin).is_ok())
}

/// The human-facing list of files `grove init` would write for `mode` +
/// `harnesses` — one line per harness registration (flagging user-global
/// targets) plus the shared steering files. Used by the `--dry-run` preview.
fn harness_targets(root: &Path, home: &Path, mode: Mode, harnesses: &[HarnessId]) -> Vec<String> {
    let mut out = Vec::new();
    if harness::expected_mcp_args(mode).is_some() {
        for &h in harnesses {
            let path = h.mcp_config_path_in(root, Some(home));
            let scope = match h.mcp_scope() {
                Scope::Global => "  (user-global)",
                Scope::Project => "",
            };
            out.push(format!("{} — {} registration{scope}", display_path(root, &path), h.display_name()));
        }
    }
    // Explore-server preview: one line per harness when McpLlm is active.
    if harness::expected_explore_args(mode).is_some() {
        for &h in harnesses {
            let path = h.mcp_config_path_in(root, Some(home));
            out.push(format!("{} — {} grove-explore registration", display_path(root, &path), h.display_name()));
        }
    }
    if harnesses.contains(&HarnessId::ClaudeCode) && mode != Mode::Grammars {
        out.push("CLAUDE.md — steering".to_string());
    }
    if harness::agents_md_expected(mode) {
        out.push("AGENTS.md — steering (cross-agent)".to_string());
    }
    out
}

fn reconcile_harness_for(
    root: &Path,
    home: &Path,
    _old_mode: Option<Mode>,
    new_mode: Mode,
    harnesses: &[HarnessId],
) -> Result<Vec<String>> {
    let mut wrote = Vec::new();

    // Load langs for steering content (from grove.lock written by provision_project).
    let langs = registry::locked_langs(&root.join("grove.lock"))?;

    // ── MCP registration — one file per harness, in its own path + format ───────
    // Loop every known harness (not just the selected ones) so de-selecting an
    // agent strips its stale grove entry. `home` resolves global-scope harnesses
    // (Codex → ~/.codex); project harnesses ignore it and resolve under `root`.
    let wants_entry = harness::expected_mcp_args(new_mode).is_some();
    for &h in HarnessId::ALL {
        let path = h.mcp_config_path_in(root, Some(home));
        if wants_entry && harnesses.contains(&h) {
            wrote.push(write_harness_registration(root, &path, h, new_mode)?);
        } else {
            strip_harness_registration(&path, h)?;
        }
    }

    // ── grove-explore registration (McpLlm only) ─────────────────────────────
    // Second loop: write or strip the locator server entry for every known
    // harness, so transitions away from McpLlm (e.g. McpLlm→Mcp) remove it.
    //
    // The locator registers under the SAME `"grove"` key as the structural
    // server (so its tool is `mcp__grove__explore` — see EXPLORE_SERVER_KEY).
    // That makes the strip here dangerous in mcp/both mode: the first loop just
    // wrote the structural entry under `"grove"`, and an unconditional strip
    // would delete it. Guard on `!wants_entry` — only strip when the structural
    // surface isn't claiming the key. The surfaces are mutually exclusive
    // (ADR 0005), so `wants_entry` and `wants_explore` are never both true, and
    // the first loop runs to completion before this one, so a McpLlm strip in
    // the first loop is always re-written here.
    let wants_explore = harness::expected_explore_args(new_mode).is_some();
    for &h in HarnessId::ALL {
        if wants_explore && harnesses.contains(&h) {
            wrote.push(write_explore_harness_registration(root, home, h)?);
        } else if !wants_entry {
            strip_explore_harness_registration(root, home, h)?;
        }
        // One-time migration: drop any stale two-server-era `grove-explore` key
        // regardless of mode, so an upgraded project doesn't keep a duplicate.
        strip_legacy_explore_registration(root, home, h)?;
    }

    // ── CLAUDE.md ── Claude Code's native, `mcp__grove__`-prefixed steering ─────
    let claude_selected = harnesses.contains(&HarnessId::ClaudeCode);
    if new_mode == Mode::Grammars || !claude_selected {
        strip_steering_block(root, "CLAUDE.md")?;
    } else {
        wrote.push(write_claude_md(root, &langs, mode_to_target(new_mode))?);
    }

    // ── AGENTS.md ── shared, harness-neutral steering for every MCP surface ─────
    // One physical file read directly by Codex/Cursor/Windsurf/VS Code/Gemini
    // (none reliably resolve `@`-imports, so the block is written inline).
    if harness::agents_md_expected(new_mode) {
        wrote.push(write_agents_md(root, &langs, mode_to_target(new_mode))?);
    } else {
        strip_steering_block(root, "AGENTS.md")?;
    }

    Ok(wrote)
}

/// Write (or refresh) grove's MCP registration for one harness at `path`,
/// dispatching on its [`McpFormat`]. Returns a human-facing status line. Only
/// called when [`harness::expected_mcp_args`] is `Some` for `mode`.
fn write_harness_registration(root: &Path, path: &Path, h: HarnessId, mode: Mode) -> Result<String> {
    let args = harness::expected_mcp_args(mode)
        .expect("write_harness_registration is called only when an entry is expected");
    match h.mcp_format() {
        McpFormat::Json { root_key, needs_type_stdio } => {
            write_json_mcp(path, args, root_key, needs_type_stdio)?
        }
        McpFormat::Toml { table } => write_toml_mcp(path, args, table)?,
    }
    Ok(format!("{} ({} registration)", display_path(root, path), h.display_name()))
}

/// Strip grove's MCP registration for one harness from `path`, dispatching on
/// its [`McpFormat`]. No-op when the file or entry is absent.
fn strip_harness_registration(path: &Path, h: HarnessId) -> Result<()> {
    match h.mcp_format() {
        McpFormat::Json { root_key, .. } => strip_json_mcp(path, root_key),
        McpFormat::Toml { table } => strip_toml_mcp(path, table),
    }
}

/// Render `path` for the status list: project-relative when under `root`,
/// otherwise absolute (global-scope harnesses like Codex).
fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).display().to_string()
}

/// Test/back-compat shim: reconcile a Claude-Code-only project, resolving any
/// global-scope harness under `root` so it never touches the real home dir.
#[cfg(test)]
fn reconcile_harness(root: &Path, old_mode: Option<Mode>, new_mode: Mode) -> Result<Vec<String>> {
    reconcile_harness_for(root, root, old_mode, new_mode, &[HarnessId::ClaudeCode])
}

/// Map a `Mode` back to the corresponding `Target` variant for content generation.
/// This is the inverse of `Target::to_mode`.
fn mode_to_target(mode: Mode) -> Target {
    match mode {
        Mode::Mcp => Target::Mcp,
        Mode::Skill => Target::Skill,
        Mode::Both => Target::Both,
        Mode::McpLlm => Target::McpLlm,
        Mode::Grammars => Target::Grammars,
    }
}

/// Remove the `<!-- grove:start -->…<!-- grove:end -->` sentinel block from
/// `filename` (under `root`), preserving all host-authored content outside the
/// block. The blank-line separator that `write_steering_md` inserts before the
/// block is also removed, so repeated A→B→A cycles never accrete blank lines.
/// If the file is absent or has no grove block, returns `Ok(())` (no-op).
/// If the block is the entire file, the file is written empty (not deleted).
/// Ensures a single trailing newline when non-empty content remains.
fn strip_steering_block(root: &Path, filename: &str) -> Result<()> {
    let path = root.join(filename);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Ok(()), // file absent — no-op
    };

    // No grove block present — no-op.
    if !text.contains(CLAUDE_START) || !text.contains(CLAUDE_END) {
        return Ok(());
    }

    let start = text.find(CLAUDE_START).unwrap();
    let end = text.find(CLAUDE_END).unwrap() + CLAUDE_END.len();

    // Compute the content before the block, trimming the \n\n separator that
    // write_steering_md inserts when appending below existing content.
    let before = text[..start].trim_end_matches('\n');
    let after = &text[end..];

    let result = if before.is_empty() && after.trim().is_empty() {
        // Block was the entire file — leave it empty.
        String::new()
    } else if before.is_empty() {
        // Block was at the top; preserve everything after.
        let s = after.trim_start_matches('\n');
        if s.is_empty() {
            String::new()
        } else {
            format!("{s}\n")
        }
    } else {
        // Host content precedes the block; re-join with single trailing newline.
        let after_trimmed = after.trim();
        if after_trimmed.is_empty() {
            format!("{before}\n")
        } else {
            format!("{before}\n\n{after_trimmed}\n")
        }
    };

    std::fs::write(&path, result).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

// ── Format-aware MCP registration primitives ─────────────────────────────
//
// Each harness stores its MCP registration in a different file, format, and
// scope (see `grove_core::harness`). These four primitives are the format
// layer — JSON (most harnesses) and TOML (Codex) — over an explicit path.
// The claude-code wrappers and the per-harness reconcile loop both build on
// them, so grove's registration is written/stripped identically everywhere.

/// Add (or refresh) grove's server entry in a JSON MCP config at `path`, under
/// `root_key` (`"mcpServers"` for most, `"servers"` for VS Code), preserving
/// any other servers. Creates parent directories. When `needs_type_stdio`, the
/// entry also carries `"type": "stdio"` (VS Code requires it). Atomic-ish: a
/// single `write` of the pretty-printed document.
fn write_json_mcp(path: &Path, args: &[&str], root_key: &str, needs_type_stdio: bool) -> Result<()> {
    let mut doc: Value = match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("{} is not valid JSON", path.display()))?,
        Err(_) => json!({}),
    };
    if !doc.is_object() {
        doc = json!({});
    }
    let exe = std::env::current_exe().context("locating the grove binary")?;
    let mut entry = json!({ "command": exe.to_string_lossy(), "args": args });
    if needs_type_stdio {
        entry["type"] = json!("stdio");
    }
    doc[root_key][MCP_SERVER_KEY] = entry;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&doc)?))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Remove grove's server entry from a JSON MCP config at `path`, under
/// `root_key`, preserving all other servers. No-op if the file is absent, has
/// no such object, or has no grove entry. Errors on malformed JSON.
fn strip_json_mcp(path: &Path, root_key: &str) -> Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Ok(()), // file absent — no-op
    };
    let mut doc: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not valid JSON", path.display()))?;
    if doc.get(root_key).and_then(|s| s.get(MCP_SERVER_KEY)).is_none() {
        return Ok(()); // no grove entry — no-op
    }
    if let Some(servers) = doc.get_mut(root_key).and_then(|v| v.as_object_mut()) {
        servers.remove(MCP_SERVER_KEY);
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&doc)?))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Add (or refresh) grove's server table in a TOML MCP config at `path`, under
/// `[<table>.grove]` (Codex: `mcp_servers`), preserving every other table and
/// any comments (via `toml_edit`). Creates parent directories.
fn write_toml_mcp(path: &Path, args: &[&str], table: &str) -> Result<()> {
    use toml_edit::{Array, DocumentMut, Item, Table};
    let mut doc: DocumentMut = match std::fs::read_to_string(path) {
        Ok(text) => text
            .parse()
            .with_context(|| format!("{} is not valid TOML", path.display()))?,
        Err(_) => DocumentMut::new(),
    };
    let exe = std::env::current_exe().context("locating the grove binary")?;
    if doc.get(table).and_then(|i| i.as_table()).is_none() {
        let mut t = Table::new();
        t.set_implicit(true); // render `[mcp_servers.grove]`, not an empty `[mcp_servers]`
        doc[table] = Item::Table(t);
    }
    let servers = doc[table]
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("{}: `{}` is not a table", path.display(), table))?;
    let mut grove = Table::new();
    grove["command"] = toml_edit::value(exe.to_string_lossy().into_owned());
    let mut arr = Array::new();
    for a in args {
        arr.push(*a);
    }
    grove["args"] = toml_edit::value(arr);
    servers[MCP_SERVER_KEY] = Item::Table(grove);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    std::fs::write(path, doc.to_string())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Remove grove's server table from a TOML MCP config at `path`, under
/// `[<table>.grove]`, preserving everything else. No-op if the file is absent
/// or has no grove table. Errors on malformed TOML.
fn strip_toml_mcp(path: &Path, table: &str) -> Result<()> {
    use toml_edit::DocumentMut;
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Ok(()), // file absent — no-op
    };
    let mut doc: DocumentMut = text
        .parse()
        .with_context(|| format!("{} is not valid TOML", path.display()))?;
    let had = doc
        .get(table)
        .and_then(|i| i.as_table())
        .map(|t| t.contains_key(MCP_SERVER_KEY))
        .unwrap_or(false);
    if !had {
        return Ok(()); // no grove table — no-op
    }
    if let Some(servers) = doc.get_mut(table).and_then(|i| i.as_table_mut()) {
        servers.remove(MCP_SERVER_KEY);
    }
    std::fs::write(path, doc.to_string())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Probe PATH for the `grove-explore` binary.
fn explore_binary_on_path() -> Option<std::path::PathBuf> {
    which::which("grove-explore").ok()
}

/// Value to use as the MCP registration `command` for `grove-explore`: the
/// absolute PATH-resolved binary when available, or the bare name as a graceful
/// fallback for PATH-only installs.
fn explore_command_value() -> std::path::PathBuf {
    explore_binary_on_path().unwrap_or_else(|| std::path::PathBuf::from("grove-explore"))
}

/// Add (or refresh) the `grove-explore` server entry in a JSON MCP config at
/// `path`, under `root_key`/[`EXPLORE_SERVER_KEY`]. The command value is derived
/// via [`explore_command_value`] and takes no args. Preserves any other servers.
/// Creates parent directories. When `needs_type_stdio`, adds `"type": "stdio"`.
fn write_json_mcp_explore_server(
    path: &Path,
    root_key: &str,
    needs_type_stdio: bool,
) -> Result<()> {
    let mut doc: Value = match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("{} is not valid JSON", path.display()))?,
        Err(_) => json!({}),
    };
    if !doc.is_object() {
        doc = json!({});
    }
    let exe = explore_command_value();
    let mut entry = json!({ "command": exe.to_string_lossy(), "args": [] });
    if needs_type_stdio {
        entry["type"] = json!("stdio");
    }
    doc[root_key][EXPLORE_SERVER_KEY] = entry;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&doc)?))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Remove the `grove-explore` server entry from a JSON MCP config at `path`,
/// under `root_key`. No-op when the file is absent, has no such object, or has
/// no `grove-explore` entry. Errors on malformed JSON.
fn strip_json_mcp_explore_server(path: &Path, root_key: &str, server_key: &str) -> Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Ok(()), // file absent — no-op
    };
    let mut doc: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not valid JSON", path.display()))?;
    if doc.get(root_key).and_then(|s| s.get(server_key)).is_none() {
        return Ok(()); // no such entry — no-op
    }
    if let Some(servers) = doc.get_mut(root_key).and_then(|v| v.as_object_mut()) {
        servers.remove(server_key);
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&doc)?))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Add (or refresh) the `grove-explore` server table in a TOML MCP config at
/// `path`, under `[<table>.grove-explore]`. The command value is derived via
/// [`explore_command_value`] and takes no args. Preserves every other table.
fn write_toml_mcp_explore_server(path: &Path, table: &str) -> Result<()> {
    use toml_edit::{Array, DocumentMut, Item, Table};
    let mut doc: DocumentMut = match std::fs::read_to_string(path) {
        Ok(text) => text
            .parse()
            .with_context(|| format!("{} is not valid TOML", path.display()))?,
        Err(_) => DocumentMut::new(),
    };
    let exe = explore_command_value();
    if doc.get(table).and_then(|i| i.as_table()).is_none() {
        let mut t = Table::new();
        t.set_implicit(true);
        doc[table] = Item::Table(t);
    }
    let servers = doc[table]
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("{}: `{}` is not a table", path.display(), table))?;
    let mut explore = Table::new();
    explore["command"] = toml_edit::value(exe.to_string_lossy().into_owned());
    explore["args"] = toml_edit::value(Array::new());
    servers[EXPLORE_SERVER_KEY] = Item::Table(explore);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    std::fs::write(path, doc.to_string())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Remove the `grove-explore` server table from a TOML MCP config at `path`,
/// under `[<table>.grove-explore]`. No-op when the file is absent or has no
/// `grove-explore` table. Errors on malformed TOML.
fn strip_toml_mcp_explore_server(path: &Path, table: &str, server_key: &str) -> Result<()> {
    use toml_edit::DocumentMut;
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Ok(()), // file absent — no-op
    };
    let mut doc: DocumentMut = text
        .parse()
        .with_context(|| format!("{} is not valid TOML", path.display()))?;
    let had = doc
        .get(table)
        .and_then(|i| i.as_table())
        .map(|t| t.contains_key(server_key))
        .unwrap_or(false);
    if !had {
        return Ok(()); // no grove-explore table — no-op
    }
    if let Some(servers) = doc.get_mut(table).and_then(|i| i.as_table_mut()) {
        servers.remove(server_key);
    }
    std::fs::write(path, doc.to_string())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Write (or refresh) the `grove-explore` server entry for one harness,
/// dispatching on its [`McpFormat`]. Returns a human-facing status line.
/// Only called when [`harness::expected_explore_args`] is `Some` for the mode.
fn write_explore_harness_registration(root: &Path, home: &Path, h: HarnessId) -> Result<String> {
    let path = h.mcp_config_path_in(root, Some(home));
    match h.mcp_format() {
        McpFormat::Json { root_key, needs_type_stdio } => {
            write_json_mcp_explore_server(&path, root_key, needs_type_stdio)?
        }
        McpFormat::Toml { table } => write_toml_mcp_explore_server(&path, table)?,
    }
    Ok(format!(
        "{} ({} explore-server registration)",
        display_path(root, &path),
        h.display_name()
    ))
}

/// Strip the `grove-explore` server entry for one harness from its registration
/// file, dispatching on its [`McpFormat`]. No-op when the file or entry is absent.
fn strip_explore_harness_registration(root: &Path, home: &Path, h: HarnessId) -> Result<()> {
    let path = h.mcp_config_path_in(root, Some(home));
    match h.mcp_format() {
        McpFormat::Json { root_key, .. } => {
            strip_json_mcp_explore_server(&path, root_key, EXPLORE_SERVER_KEY)
        }
        McpFormat::Toml { table } => {
            strip_toml_mcp_explore_server(&path, table, EXPLORE_SERVER_KEY)
        }
    }
}

/// Remove any lingering `grove-explore`-keyed registration from the two-server
/// era (0.4.x). Since the locator now shares the `"grove"` key, an upgraded
/// project would otherwise keep a stale duplicate under the old key. A no-op
/// once the legacy entry is gone.
fn strip_legacy_explore_registration(root: &Path, home: &Path, h: HarnessId) -> Result<()> {
    let path = h.mcp_config_path_in(root, Some(home));
    match h.mcp_format() {
        McpFormat::Json { root_key, .. } => {
            strip_json_mcp_explore_server(&path, root_key, harness::LEGACY_EXPLORE_SERVER_KEY)
        }
        McpFormat::Toml { table } => {
            strip_toml_mcp_explore_server(&path, table, harness::LEGACY_EXPLORE_SERVER_KEY)
        }
    }
}

/// Add (or refresh) the grove MCP server in `.mcp.json`, preserving other servers.
/// Superseded in production by [`write_harness_registration`] (the per-harness
/// dispatch); retained as a focused regression fixture for the claude-code path.
#[cfg(test)]
fn write_mcp_json(root: &Path) -> Result<String> {
    write_json_mcp(&root.join(".mcp.json"), &["serve"], "mcpServers", false)?;
    Ok(".mcp.json (registration — the tools exist)".to_string())
}

/// Write/refresh the grove steering section in `CLAUDE.md`, between markers, so
/// re-running is idempotent and never disturbs the rest of the file.
fn write_claude_md(root: &Path, langs: &[String], target: Target) -> Result<String> {
    write_steering_md(
        root,
        "CLAUDE.md",
        &claude_section(langs, target),
        "CLAUDE.md (steering — the tools get used)",
    )
}

/// Splice `section` (bracketed by `CLAUDE_START`/`CLAUDE_END`) into `filename`
/// between those sentinels — replacing an existing block, appending below other
/// content, or creating the file. Idempotent, and never disturbs the rest of the
/// file. Shared by `write_claude_md` and `write_agents_md`, which differ only in
/// the target file, the section body, and the status message.
fn write_steering_md(
    root: &Path,
    filename: &str,
    section: &str,
    message: &str,
) -> Result<String> {
    let path = root.join(filename);
    let existing = std::fs::read_to_string(&path).ok();
    let updated = match existing {
        Some(text) if text.contains(CLAUDE_START) && text.contains(CLAUDE_END) => {
            let start = text.find(CLAUDE_START).unwrap();
            let end = text.find(CLAUDE_END).unwrap() + CLAUDE_END.len();
            format!("{}{}{}", &text[..start], section, &text[end..])
        }
        Some(text) => format!("{}\n\n{}\n", text.trim_end(), section),
        None => format!("{section}\n"),
    };
    std::fs::write(&path, updated).with_context(|| format!("writing {}", path.display()))?;
    Ok(message.to_string())
}

/// Write/refresh the grove steering section in `AGENTS.md` via `write_steering_md`;
/// uses harness-neutral framing (AGENTS.md is read by non-Claude harnesses too).
/// The status line is mode-aware: explore-mode vs the standard cross-agent block.
fn write_agents_md(root: &Path, langs: &[String], target: Target) -> Result<String> {
    let message = if target == Target::McpLlm {
        "AGENTS.md (explore-mode steering)"
    } else {
        "AGENTS.md (steering — cross-agent)"
    };
    write_steering_md(root, "AGENTS.md", &agents_section(langs, target), message)
}

/// The AGENTS.md steering block — harness-neutral framing (bare grove tool
/// names, no `mcp__grove__` prefix) so it is useful in every non-Claude harness
/// that reads AGENTS.md: Codex, Cursor, Windsurf, VS Code, Gemini. Two bodies:
/// the explore-mode locator framing for `McpLlm`, and the standard 7-tool
/// routing guide for the plain MCP surfaces (`Mcp`/`Both`). Uses the same
/// sentinel markers as CLAUDE.md for idempotent updates.
fn agents_section(langs: &[String], target: Target) -> String {
    if target != Target::McpLlm {
        return agents_section_standard(langs);
    }
    let langs_str = langs.join(", ");
    format!(
        "{CLAUDE_START}
## Code navigation: grove — structural + explore surfaces

**grove** is configured with **two** servers (languages: {langs_str}):

- **Structural tools** (`outline`, `symbols`, `source`, `callers`, `definition`,
  `map`, `check`): byte-precise, token-cheap. Use for precision work on named
  symbols: read one symbol's body, map a directory, check syntax after an edit.

- **`explore`** — LLM-backed code locator: sweeps multiple files with tree-sitter
  + text tools, returns file:line citations. Use for broad \"where is X\" questions
  before you know which file to look in.

**Recommended flow to understand a feature:**
(1) `explore` — one narrow question per call (\"where is X defined\",
    \"which files handle Y\"); iterate broad → specific.
(2) `source` / `map` on the cited file:line locations.
(3) Synthesise the explanation yourself.

Do not delegate large multi-part tasks to `explore`; keep each question
single-focus. For text, configs, and non-code files, use the shell (`grep`, `rg`).
{CLAUDE_END}"
    )
}

/// The harness-neutral standard-mode AGENTS.md body: grove's 7 structural tools
/// referred to by **bare** name (no `mcp__grove__` prefix), since AGENTS.md is
/// read by Codex/Cursor/Windsurf/VS Code/Gemini, each of which surfaces grove's
/// MCP tools under its own naming. Same route-by-task guidance as the CLAUDE.md
/// MCP block, minus Claude-specific framing (no ToolSearch/deferred-tools note).
fn agents_section_standard(langs: &[String]) -> String {
    let langs = langs.join(", ");
    format!(
        "{CLAUDE_START}
## Code navigation: grove for structure, shell for the rest

**grove** is a tree-sitter engine for *structural* code questions — byte-precise,
token-cheap (languages: {langs}). Its MCP server exposes seven tools; reach for them
when a code question lands (don't default to grep or a whole-file read):
`outline`, `symbols`, `source`, `callers`, `definition`, `map`, `check`.

**Use grove for named symbols and relationships** (every result carries a stable
`symbol-id`, `<lang>:<relpath>#<name>@<row>`, to pass forward; lines 1-based):
- What's in a file (skeleton, not the whole file) → `outline` (`detail:0` if > 500 lines).
- Where a fn / type / struct / macro is defined → `symbols` with `name` → `source` with the id.
- One symbol's exact body → `source`.
- Who calls it → `callers`.
- Go-to-def from a usage (scope-aware, follows imports cross-file) → `definition` with `at` (file:line:col).
- How a directory connects → `map` (one call; prefer over many `source`).
- Syntax after an edit → `check`.

**Use the shell — the right tool, not a fallback — when grove can't see the target:**
- Text, not a symbol (a string, log / error message, config key, a macro's *value*,
  a constant, a flag, a TODO) → `grep -rn` / `rg`. grove finds definitions, not text.
- Non-code files (Makefiles, configs, data, docs) → `grep` / `read`.
- A quick fact (path exists, `ls`, `wc -l`, `find`, read a small file) → shell.

Rule of thumb: want a **symbol** → grove first (don't `grep` / `read` for it). Want
**text or a quick fact** → shell. Combining is fine.
{CLAUDE_END}"
    )
}

/// The steering block, tailored to how the agent reaches grove. The MCP target
/// carries the full route-by-task guide inline (there is no skill file to defer
/// to); the skill target is a thin version that defers to SKILL.md as the single
/// source of truth. Both route by task — grove for named symbols / structure, the
/// shell for text / non-code / quick facts, combining freely — rather than banning
/// grep: a cold agent with no block at all defaults to grep / whole-file reads
/// (VISION §6.4.1), and an over-rigid "grove or violation" block overcorrects into
/// costly source fan-outs for things the shell does cheaply.
fn claude_section(langs: &[String], target: Target) -> String {
    let langs = langs.join(", ");
    if target == Target::McpLlm {
        return format!(
            "{CLAUDE_START}
## Code navigation: `mcp__grove__explore` — the code locator

**`mcp__grove__explore`** is a code LOCATOR (languages: {langs}). It runs
a local model over tree-sitter structural tools plus glob/grep/read, and replies
with **location lines only** — one per line, most relevant first, each
`lang:path#symbol@line`. It locates; it does not explain.

**Reach for it first on any \"where is X\" question** — where something is
defined, which files handle a feature, who calls a function — instead of
grepping the tree yourself. It sweeps many files in one call and costs you only
the citations it returns.

**Then read what it cites.** The line numbers are exact (1-based), so read a
window at that offset rather than the whole file.

Keep each question **single-focus** — one narrow question per call, iterating
broad → specific. Do not hand it large multi-part tasks; it is a locator, not a
research agent. For text, configs, and non-code files, use the shell
(`grep`, `rg`).
{CLAUDE_END}"
        );
    }
    if target.writes_mcp() {
        // Claude Code exposes the tools as `mcp__<server-key>__<tool>`; steer with
        // those exact names. A future AGENTS.md/.cursorrules adapter passes its own.
        let p = format!("mcp__{MCP_SERVER_KEY}__");
        format!(
            "{CLAUDE_START}
## Code navigation: grove for structure, shell for the rest

**grove** is a tree-sitter engine for *structural* code questions — byte-precise,
token-cheap (languages: {langs}). Its tools are **deferred** MCP tools; load them in
one ToolSearch when a code question lands (don't default to a search agent or grep):
`{p}outline`, `{p}symbols`, `{p}source`, `{p}callers`, `{p}definition`, `{p}map`, `{p}check`.

**Use grove for named symbols and relationships** (every result carries a stable
`symbol-id`, `<lang>:<relpath>#<name>@<row>`, to pass forward; lines 1-based):
- What's in a file (skeleton, not the whole file) → `{p}outline` (`detail:0` if > 500 lines).
- Where a fn / type / struct / macro is defined → `{p}symbols` with `name` → `{p}source` with the id.
- One symbol's exact body → `{p}source`.
- Who calls it → `{p}callers`.
- Go-to-def from a usage (scope-aware, follows imports cross-file) → `{p}definition` with `at` (file:line:col).
- How a directory connects → `{p}map` (one call; prefer over many `{p}source`).
- Syntax after an edit → `{p}check`.

**Use the shell — the right tool, not a fallback — when grove can't see the target:**
- Text, not a symbol (a string, log / error message, config key, a macro's *value*,
  a constant, a flag, a TODO) → `grep -rn` / `rg`. grove finds definitions, not text.
- Non-code files (Makefiles, configs, data, docs) → `grep` / `read`.
- A quick fact (path exists, `ls`, `wc -l`, `find`, read a small file) → shell.

**Combine** (same 1-based lines, same bytes): `grep` a literal's line → `{p}definition`
`at` to resolve its symbol · `{p}outline` → bounded `read` (`offset`/`limit`) for
adjacent symbols · `{p}map` / `{p}symbols` to locate → `grep` a constant inside.

Rule of thumb: want a **symbol** → grove first (don't `grep` / `read` for it). Want
**text or a quick fact** → shell. Combining is fine.
{CLAUDE_END}"
        )
    } else {
        format!(
            "{CLAUDE_START}
## Code navigation: grove for structure, shell for the rest

**grove** (tree-sitter backed; languages: {langs}) answers *structural* code
questions — where a symbol is defined, who calls it, what's in a file, how a
directory connects, syntax-check after edits — byte-precise and token-cheap. For a
**named symbol or a structural relationship**, reach for grove (the **grove skill**,
or the `grove` CLI — same engine) before `grep` or a whole-file `read`.

Use the shell where grove can't see the target — it's the right tool, not a fallback:
**text** (a string, log / error message, config key, constant, flag, TODO) → `grep` / `rg`;
**non-code files** (Makefiles, configs, data, docs) → `grep` / `read`; a **quick fact**
(path exists, `ls`, `wc -l`, `find`) → shell. Combining is fine — grove to navigate,
the shell to pin a literal or read a small region.

**The procedure, recovery rule, and full command surface live in the grove skill —
read it before answering a structural question.** It is the single source of truth;
don't rely on remembered flags.
{CLAUDE_END}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grove_core::config::{GroveConfig, Mode, ModeChoice};

    fn tmp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("grove_init_test_{}_{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn claude_section_mcp_is_delimited_and_names_langs_and_prefix() {
        let s = claude_section(&["rust".into(), "python".into()], Target::Mcp);
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        assert!(s.contains("rust, python"));
        assert!(s.contains("mcp__grove__outline"));
        assert!(s.contains("mcp__grove__callers"));
    }

    #[test]
    fn claude_section_skill_is_thin_and_defers_to_skill() {
        let s = claude_section(&["rust".into()], Target::Skill);
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        assert!(s.contains("grove skill"), "skill steering defers to the skill");
        assert!(s.contains("single source of truth"), "defers to SKILL.md, not remembered flags");
        assert!(s.contains("grep"), "routes text search to the shell");
        assert!(!s.contains("mcp__grove__"), "skill steering must not name MCP tools");
    }

    #[test]
    fn claude_section_mcp_routes_grove_and_shell() {
        let s = claude_section(&["rust".into()], Target::Mcp);
        // grove for structure: tools named, with the symbol-first rule of thumb.
        assert!(s.contains("mcp__grove__source"), "names the grove tools");
        assert!(s.contains("symbol"), "routes symbol work to grove");
        // shell is a first-class partner, not a banned fallback.
        assert!(s.contains("grep"), "routes text/quick-fact work to the shell");
        assert!(s.contains("Combine"), "blesses combining grove + shell");
        assert!(!s.contains("steering violation"), "no prohibitive framing");
    }

    #[test]
    fn write_claude_md_creates_then_updates_idempotently() {
        let dir = tmp("claude_idem");
        let path = dir.join("CLAUDE.md");

        write_claude_md(&dir, &["rust".into()], Target::Mcp).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        assert_eq!(first.matches(CLAUDE_START).count(), 1);
        assert!(first.contains("rust"));

        // Re-run with a different language set: section is replaced in place, not
        // duplicated.
        write_claude_md(&dir, &["python".into()], Target::Mcp).unwrap();
        let second = std::fs::read_to_string(&path).unwrap();
        assert_eq!(second.matches(CLAUDE_START).count(), 1, "exactly one grove section");
        assert_eq!(second.matches(CLAUDE_END).count(), 1);
        assert!(second.contains("python"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_claude_md_appends_below_existing_content() {
        let dir = tmp("claude_append");
        let path = dir.join("CLAUDE.md");
        std::fs::write(&path, "# My project\n\nHand-written notes.\n").unwrap();

        write_claude_md(&dir, &["rust".into()], Target::Mcp).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Hand-written notes."), "existing content preserved");
        assert!(text.contains(CLAUDE_START), "grove section appended");
        assert!(text.find("Hand-written").unwrap() < text.find(CLAUDE_START).unwrap());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_mcp_json_registers_grove_and_preserves_other_servers() {
        let dir = tmp("mcp_preserve");
        let path = dir.join(".mcp.json");
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();

        write_mcp_json(&dir).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["mcpServers"]["other"]["command"], json!("x"), "existing server kept");
        assert_eq!(doc["mcpServers"]["grove"]["args"], json!(["serve"]), "grove registered");
        assert!(doc["mcpServers"]["grove"]["command"].is_string());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_mcp_json_creates_file_when_absent() {
        let dir = tmp("mcp_fresh");
        write_mcp_json(&dir).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
        assert!(doc["mcpServers"]["grove"].is_object());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_mcp_json_errors_on_invalid_existing_json() {
        let dir = tmp("mcp_bad");
        std::fs::write(dir.join(".mcp.json"), "{not json").unwrap();
        let err = write_mcp_json(&dir).unwrap_err();
        assert!(err.to_string().contains("not valid JSON"), "got: {err}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn target_default_is_mcp_and_flags_route_correctly() {
        assert_eq!(Target::default(), Target::Mcp);
        assert!(Target::Mcp.writes_mcp() && !Target::Mcp.is_skill());
        assert!(!Target::Skill.writes_mcp() && Target::Skill.is_skill());
        assert!(Target::Both.writes_mcp() && Target::Both.is_skill());
        // Grammars wires nothing — no mcp, no skill, no steering.
        assert!(!Target::Grammars.writes_mcp() && !Target::Grammars.is_skill());
        assert!(!Target::Grammars.writes_steering());
        assert!(Target::Mcp.writes_steering() && Target::Skill.writes_steering());
    }

    /// `reconcile_harness` is harness-only: grammar provisioning + the
    /// `grove.lock` write moved to `grove_core::init::provision_project`, so
    /// these tests seed a lock first (the source of steering's language list)
    /// and assert only the harness files (`.mcp.json` / `CLAUDE.md` / `AGENTS.md`).
    fn seed_lock(dir: &std::path::Path) {
        std::fs::write(
            dir.join("grove.lock"),
            r#"{"version":1,"grammars":[{"name":"rust","version":"1","wasm":"x"}]}"#,
        )
        .unwrap();
    }

    // ─── strip_steering_block tests ───────────────────────────────────────────

    #[test]
    fn strip_steering_block_removes_block_leaves_host_content() {
        let dir = tmp("strip_leaves_host");
        let path = dir.join("CLAUDE.md");
        // Host content before the block (write_steering_md appends \n\n before the block)
        std::fs::write(
            &path,
            format!("# My project\n\nHand-written notes.\n\n{CLAUDE_START}\ngrove block\n{CLAUDE_END}\n"),
        )
        .unwrap();

        strip_steering_block(&dir, "CLAUDE.md").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Hand-written notes."), "host content preserved");
        assert!(!text.contains(CLAUDE_START), "grove block removed");
        assert!(!text.contains(CLAUDE_END), "grove block removed");
        // Should end with a single newline
        assert!(text.ends_with('\n'), "single trailing newline");
        // No double blank lines accreted
        assert!(!text.contains("\n\n\n"), "no triple newlines: {text:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_steering_block_empties_file_when_block_only() {
        let dir = tmp("strip_block_only");
        let path = dir.join("CLAUDE.md");
        std::fs::write(
            &path,
            format!("{CLAUDE_START}\ngrove block\n{CLAUDE_END}\n"),
        )
        .unwrap();

        strip_steering_block(&dir, "CLAUDE.md").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.is_empty(), "file written empty when block-only: {text:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_steering_block_noop_when_no_block() {
        let dir = tmp("strip_noop_no_block");
        let path = dir.join("CLAUDE.md");
        let original = "# My project\n\nNo grove block here.\n";
        std::fs::write(&path, original).unwrap();

        strip_steering_block(&dir, "CLAUDE.md").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, original, "file unchanged when no grove block");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_steering_block_noop_when_absent() {
        let dir = tmp("strip_noop_absent");
        // File doesn't exist — should not error
        strip_steering_block(&dir, "CLAUDE.md").unwrap();
        assert!(!dir.join("CLAUDE.md").exists(), "file not created");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── format-aware primitive tests (write_json_mcp / write_toml_mcp) ───────

    #[test]
    fn write_json_mcp_vscode_uses_servers_key_and_type_stdio() {
        let dir = tmp("json_vscode");
        let path = dir.join(".vscode").join("mcp.json");
        // Parent dir does not exist yet — writer must create it.
        write_json_mcp(&path, &["serve"], "servers", true).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["servers"]["grove"]["args"], json!(["serve"]), "under `servers`, not `mcpServers`");
        assert_eq!(doc["servers"]["grove"]["type"], json!("stdio"), "VS Code needs type:stdio");
        assert!(doc.get("mcpServers").is_none(), "must not use mcpServers key");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_json_mcp_preserves_siblings_no_type_stdio_by_default() {
        let dir = tmp("json_siblings");
        let path = dir.join("mcp.json");
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();
        write_json_mcp(&path, &["serve"], "mcpServers", false).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["mcpServers"]["other"]["command"], json!("x"), "sibling kept");
        assert_eq!(doc["mcpServers"]["grove"]["args"], json!(["serve"]));
        assert!(doc["mcpServers"]["grove"].get("type").is_none(), "no type:stdio unless requested");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_toml_mcp_creates_grove_table_and_preserves_others() {
        let dir = tmp("toml_create");
        let path = dir.join("config.toml");
        // Pre-seed an unrelated server + a top-level key that must survive.
        std::fs::write(
            &path,
            "model = \"gpt-5\"\n\n[mcp_servers.other]\ncommand = \"x\"\nargs = [\"a\"]\n",
        )
        .unwrap();
        write_toml_mcp(&path, &["serve"], "mcp_servers").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let doc: toml_edit::DocumentMut = text.parse().unwrap();
        assert_eq!(doc["mcp_servers"]["grove"]["args"].as_array().unwrap().len(), 1);
        assert!(doc["mcp_servers"]["grove"]["command"].as_str().is_some(), "grove command written");
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("x"), "other server preserved");
        assert_eq!(doc["model"].as_str(), Some("gpt-5"), "unrelated top-level key preserved");
        // Renders as [mcp_servers.grove], not an empty [mcp_servers] header.
        assert!(text.contains("[mcp_servers.grove]"), "nested table header: {text}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_toml_mcp_removes_grove_keeps_others() {
        let dir = tmp("toml_strip");
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "[mcp_servers.grove]\ncommand = \"g\"\nargs = [\"serve\"]\n\n[mcp_servers.other]\ncommand = \"x\"\n",
        )
        .unwrap();
        strip_toml_mcp(&path, "mcp_servers").unwrap();
        let doc: toml_edit::DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert!(doc["mcp_servers"].get("grove").is_none(), "grove removed");
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("x"), "other kept");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_toml_mcp_noop_when_absent_or_no_grove() {
        let dir = tmp("toml_strip_noop");
        // absent file
        strip_toml_mcp(&dir.join("config.toml"), "mcp_servers").unwrap();
        // present, but no grove table
        let path = dir.join("config.toml");
        std::fs::write(&path, "[mcp_servers.other]\ncommand = \"x\"\n").unwrap();
        strip_toml_mcp(&path, "mcp_servers").unwrap();
        let doc: toml_edit::DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("x"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn agents_section_standard_uses_bare_tool_names() {
        let s = agents_section(&["rust".into()], Target::Mcp);
        assert!(s.starts_with(CLAUDE_START) && s.trim_end().ends_with(CLAUDE_END));
        assert!(s.contains("rust"), "names the language");
        assert!(s.contains("`outline`") && s.contains("`callers`"), "names bare tools");
        assert!(!s.contains("mcp__grove__"), "AGENTS.md is harness-neutral (no MCP prefix)");
        assert!(!s.contains("explore"), "standard body, not the explore-mode body");
    }

    // ─── strip_grove_entry_from_mcp_json tests ────────────────────────────────

    #[test]
    fn strip_grove_entry_from_mcp_json_removes_grove_key() {
        let dir = tmp("strip_mcp_removes_grove");
        let path = dir.join(".mcp.json");
        std::fs::write(
            &path,
            r#"{"mcpServers":{"grove":{"command":"grove","args":["serve"]},"other":{"command":"x"}}}"#,
        )
        .unwrap();

        strip_json_mcp(&dir.join(".mcp.json"), "mcpServers").unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(doc["mcpServers"]["grove"].is_null(), "grove entry removed");
        assert_eq!(doc["mcpServers"]["other"]["command"], json!("x"), "other server kept");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_grove_entry_from_mcp_json_noop_when_absent() {
        let dir = tmp("strip_mcp_noop_absent");
        strip_json_mcp(&dir.join(".mcp.json"), "mcpServers").unwrap();
        assert!(!dir.join(".mcp.json").exists(), "file not created");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_grove_entry_from_mcp_json_noop_when_no_grove_key() {
        let dir = tmp("strip_mcp_noop_no_grove");
        let path = dir.join(".mcp.json");
        let original = r#"{"mcpServers":{"other":{"command":"x"}}}"#;
        std::fs::write(&path, original).unwrap();

        strip_json_mcp(&dir.join(".mcp.json"), "mcpServers").unwrap();
        // File should not be modified (same content, but formatting may differ after parse+write).
        // The important thing is that "other" is still there and grove is not.
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["mcpServers"]["other"]["command"], json!("x"), "other server intact");
        assert!(doc["mcpServers"]["grove"].is_null(), "grove not added");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── reconcile_harness helper assertions ──────────────────────────────────

    fn assert_mcp_json_consistent(dir: &std::path::Path, mode: Mode) {
        // One key, `grove`, holds whichever surface the mode selects — structural
        // (args `["serve"]`) in mcp/both, the locator (args `[]`, grove-explore
        // binary) in mcp-llm, absent otherwise. The surfaces are exclusive, so
        // they share the key and are told apart by args, not by a distinct key.
        let path = dir.join(".mcp.json");
        match mode {
            Mode::Mcp | Mode::Both => {
                let doc: Value = serde_json::from_str(
                    &std::fs::read_to_string(&path)
                        .unwrap_or_else(|_| panic!("missing .mcp.json for mode {mode:?}")),
                )
                .unwrap();
                assert_eq!(
                    doc["mcpServers"]["grove"]["args"],
                    json!(["serve"]),
                    "mode {mode:?}: grove must be the structural registration (args [serve])"
                );
            }
            Mode::McpLlm => {
                let doc: Value = serde_json::from_str(
                    &std::fs::read_to_string(&path)
                        .unwrap_or_else(|_| panic!("missing .mcp.json for mode {mode:?}")),
                )
                .unwrap();
                // `grove` is present but is the LOCATOR, not structural: empty
                // args, command pointing at the grove-explore binary.
                assert!(
                    !doc["mcpServers"]["grove"].is_null(),
                    "mode McpLlm: grove (locator) entry must be present"
                );
                assert_eq!(
                    doc["mcpServers"]["grove"]["args"],
                    json!([]),
                    "mode McpLlm: the grove entry must be the locator (empty args)"
                );
                let cmd = doc["mcpServers"]["grove"]["command"].as_str().unwrap_or("");
                assert!(
                    cmd.ends_with("grove-explore"),
                    "mode McpLlm: grove entry must run the grove-explore binary; got {cmd}"
                );
            }
            Mode::Skill | Mode::Grammars => {
                // File may not exist; if it does, the grove entry must be absent.
                if path.exists() {
                    let doc: Value = serde_json::from_str(
                        &std::fs::read_to_string(&path).unwrap(),
                    )
                    .unwrap();
                    assert!(
                        doc.get("mcpServers")
                            .and_then(|s| s.get("grove"))
                            .is_none(),
                        "mode {mode:?}: grove entry should be absent in .mcp.json"
                    );
                }
            }
        }
    }

    fn assert_claude_md_consistent(dir: &std::path::Path, mode: Mode) {
        let path = dir.join("CLAUDE.md");
        match mode {
            Mode::Grammars => {
                // File either absent or has no grove block.
                if path.exists() {
                    let text = std::fs::read_to_string(&path).unwrap();
                    assert!(
                        !text.contains(CLAUDE_START),
                        "mode Grammars: CLAUDE.md must not contain grove block, got: {text:?}"
                    );
                }
            }
            Mode::McpLlm => {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("missing CLAUDE.md for mode {mode:?}"));
                assert!(
                    text.contains(CLAUDE_START),
                    "mode McpLlm: CLAUDE.md must have grove block"
                );
                assert!(
                    text.contains("mcp__grove__explore"),
                    "mode McpLlm: CLAUDE.md must reference explore tool in grove-explore server"
                );
            }
            Mode::Mcp | Mode::Both => {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("missing CLAUDE.md for mode {mode:?}"));
                assert!(
                    text.contains(CLAUDE_START),
                    "mode {mode:?}: CLAUDE.md must have grove block"
                );
                assert!(
                    text.contains("mcp__grove__outline"),
                    "mode {mode:?}: CLAUDE.md must reference outline tool"
                );
            }
            Mode::Skill => {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("missing CLAUDE.md for mode {mode:?}"));
                assert!(
                    text.contains(CLAUDE_START),
                    "mode Skill: CLAUDE.md must have grove block"
                );
                assert!(
                    text.contains("grove skill"),
                    "mode Skill: CLAUDE.md must reference grove skill"
                );
            }
        }
    }

    fn assert_agents_md_consistent(dir: &std::path::Path, mode: Mode) {
        let path = dir.join("AGENTS.md");
        // Every MCP surface (Mcp/Both/McpLlm) writes the shared AGENTS.md block;
        // Skill/Grammars write none. Mirrors `harness::agents_md_expected`.
        match mode {
            Mode::Mcp | Mode::Both | Mode::McpLlm => {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("missing AGENTS.md for mode {mode:?}"));
                assert!(
                    text.contains(CLAUDE_START),
                    "mode {mode:?}: AGENTS.md must have grove block"
                );
            }
            Mode::Skill | Mode::Grammars => {
                if path.exists() {
                    let text = std::fs::read_to_string(&path).unwrap();
                    assert!(
                        !text.contains(CLAUDE_START),
                        "mode {mode:?}: AGENTS.md must not have grove block, got: {text:?}"
                    );
                }
            }
        }
    }

    // ─── reconcile_harness transition-matrix test (20 ordered A→B pairs) ─────

    #[test]
    fn reconcile_harness_transition_matrix() {
        let modes = [Mode::Mcp, Mode::Skill, Mode::Both, Mode::McpLlm, Mode::Grammars];

        for &old in &modes {
            for &new in &modes {
                if old == new {
                    continue;
                }
                let dir = tmp(&format!("matrix_{old:?}_{new:?}"));
                seed_lock(&dir);

                // Seed the initial harness for mode A
                reconcile_harness(&dir, None, old)
                    .unwrap_or_else(|e| panic!("seed {old:?}: {e}"));

                // Transition to mode B
                reconcile_harness(&dir, Some(old), new)
                    .unwrap_or_else(|e| panic!("transition {old:?}→{new:?}: {e}"));

                // Assert harness is consistent with B
                assert_mcp_json_consistent(&dir, new);
                assert_claude_md_consistent(&dir, new);
                assert_agents_md_consistent(&dir, new);

                std::fs::remove_dir_all(&dir).ok();
            }
        }
    }

    // ─── host-content-preservation test ──────────────────────────────────────

    #[test]
    fn reconcile_harness_preserves_host_content() {
        let dir = tmp("preserves_host");
        seed_lock(&dir);

        // 1. Seed CLAUDE.md with host content + Mcp block.
        write_claude_md(&dir, &["rust".into()], Target::Mcp).unwrap();
        // Prepend host content (simulate a file that had content before grove ran)
        let existing = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
        std::fs::write(
            dir.join("CLAUDE.md"),
            format!("# My project\n\nHost notes.\n\n{existing}"),
        )
        .unwrap();

        // Transition Mcp → Grammars: CLAUDE.md block stripped, host content intact.
        reconcile_harness(&dir, Some(Mode::Mcp), Mode::Grammars).unwrap();
        let claude = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
        assert!(claude.contains("Host notes."), "host content in CLAUDE.md preserved");
        assert!(!claude.contains(CLAUDE_START), "grove block removed from CLAUDE.md");

        // 2. Seed .mcp.json with grove + grove-explore + other server; transition to Skill
        // → both grove and grove-explore removed, other preserved.
        let mcp_path = dir.join(".mcp.json");
        std::fs::write(
            &mcp_path,
            r#"{"mcpServers":{"grove":{"command":"g","args":["serve"]},"grove-explore":{"command":"ge","args":[]},"other":{"command":"x"}}}"#,
        )
        .unwrap();
        reconcile_harness(&dir, Some(Mode::Grammars), Mode::Skill).unwrap();
        let doc: Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
        assert!(doc["mcpServers"]["grove"].is_null(), "grove removed from .mcp.json");
        assert!(doc["mcpServers"]["grove-explore"].is_null(), "grove-explore removed from .mcp.json");
        assert_eq!(doc["mcpServers"]["other"]["command"], json!("x"), "other server preserved");

        // 3. Seed AGENTS.md with host content + an MCP block; transition to a mode
        // that strips AGENTS.md (Skill) → block removed, host content intact.
        // First get to McpLlm to seed AGENTS.md.
        reconcile_harness(&dir, Some(Mode::Skill), Mode::McpLlm).unwrap();
        let existing_agents = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        std::fs::write(
            dir.join("AGENTS.md"),
            format!("# AGENTS\n\nHost agent notes.\n\n{existing_agents}"),
        )
        .unwrap();
        // Transition McpLlm → Skill: AGENTS.md block stripped, host content intact.
        reconcile_harness(&dir, Some(Mode::McpLlm), Mode::Skill).unwrap();
        let agents = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        assert!(agents.contains("Host agent notes."), "host content in AGENTS.md preserved");
        assert!(!agents.contains(CLAUDE_START), "grove block removed from AGENTS.md");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── config round-trip test ───────────────────────────────────────────────

    #[test]
    fn reconcile_harness_then_save_config_active_mode() {
        let dir = tmp("config_round_trip");
        seed_lock(&dir);

        // Reconcile to Mcp mode, then save config.
        reconcile_harness(&dir, None, Mode::Mcp).unwrap();
        let cfg = GroveConfig { version: 1, mode: Mode::Mcp, ..Default::default() };
        cfg.save(&dir).unwrap();

        // active_mode should report Mcp.
        let loaded = GroveConfig::load(&dir).unwrap();
        assert_eq!(loaded.mode, Mode::Mcp, "config.json round-trips Mcp mode");
        let am = grove_core::config::active_mode(&dir, ModeChoice::None);
        assert_eq!(am, Mode::Mcp, "active_mode returns Mcp");

        // Transition to McpLlm mode, save config.
        reconcile_harness(&dir, Some(Mode::Mcp), Mode::McpLlm).unwrap();
        let cfg2 = GroveConfig { version: 1, mode: Mode::McpLlm, ..Default::default() };
        cfg2.save(&dir).unwrap();

        let loaded2 = GroveConfig::load(&dir).unwrap();
        assert_eq!(loaded2.mode, Mode::McpLlm, "config.json round-trips McpLlm mode");
        let am2 = grove_core::config::active_mode(&dir, ModeChoice::None);
        assert_eq!(am2, Mode::McpLlm, "active_mode returns McpLlm");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── existing McpLlm unit tests ───────────────────────────────────────────

    #[test]
    fn claude_section_mcp_llm_routes_explore_not_individual_tools() {
        let s = claude_section(&["rust".into()], Target::McpLlm);
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        // Locator-framed steering names the one tool the mode registers.
        assert!(s.contains("mcp__grove__explore"), "names the explore locator tool: {s}");
        // The seven structural tools are NOT registered in this mode, so the
        // block must not steer the agent at them — only `explore` exists here.
        for tool in ["outline", "symbols", "source", "callers", "definition", "map", "check"] {
            assert!(
                !s.contains(&format!("mcp__grove__{tool}")),
                "must not name the structural tool mcp__grove__{tool} in mcp-llm steering: {s}"
            );
        }
    }

    #[test]
    fn agents_section_mcp_llm_routes_explore_not_individual_tools() {
        let s = agents_section(&["rust".into()], Target::McpLlm);
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        // New dual-surface framing: explore tool named by bare name (harness-neutral).
        assert!(s.contains("explore"), "names explore tool: {s}");
        // AGENTS.md must not use Claude-specific MCP prefixes.
        assert!(!s.contains("mcp__grove__"), "must not use Claude-specific prefix: {s}");
        // Structural tools also mentioned by bare name.
        assert!(s.contains("`source`") || s.contains("`map`"), "names structural tools: {s}");
        // No automatic fallback framing in the new dual-surface content.
        assert!(!s.contains("fallback"), "no fallback framing when both servers always present: {s}");
    }

    #[test]
    fn reconcile_harness_mcp_llm_writes_mcp_json_explore_and_steering() {
        let dir = tmp("harness_mcp_llm");
        seed_lock(&dir);
        let wrote = reconcile_harness(&dir, None, Mode::McpLlm).unwrap();

        // .mcp.json must carry the LOCATOR under the `grove` key — empty args,
        // grove-explore binary — and no separate `grove-explore` key.
        let mcp_json = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&mcp_json).unwrap();
        assert_eq!(
            doc["mcpServers"]["grove"]["args"],
            serde_json::json!([]),
            "grove must be the locator registration (empty args) in McpLlm"
        );
        assert!(
            doc["mcpServers"]["grove"]["command"]
                .as_str()
                .unwrap_or("")
                .ends_with("grove-explore"),
            "the grove entry must run the grove-explore binary"
        );
        assert!(
            doc["mcpServers"]["grove-explore"].is_null(),
            "no separate grove-explore key — the locator shares the grove key"
        );

        assert!(dir.join("CLAUDE.md").exists(), "CLAUDE.md written");
        assert!(dir.join("AGENTS.md").exists(), "AGENTS.md written");
        // 3 harness entries: explore-server registration + CLAUDE.md + AGENTS.md.
        // There is no structural registration in McpLlm (ADR 0005).
        assert_eq!(wrote.len(), 3, "3 harness entries: {wrote:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// AC3: pre-split projects whose `.mcp.json` has `grove` with args
    /// `["serve", "--explore"]` converge to the explore-only layout on first
    /// `reconcile_harness(_, _, Mode::McpLlm)` call — the stale structural
    /// entry is stripped, not rewritten (ADR 0005).
    #[test]
    fn mcp_llm_forward_migration_strips_structural_entry_for_explore_only() {
        let dir = tmp("mcp_llm_migration");
        seed_lock(&dir);

        // Seed the old (pre-split) layout: one entry with the now-invalid args.
        std::fs::write(
            dir.join(".mcp.json"),
            r#"{"mcpServers":{"grove":{"command":"/usr/bin/grove","args":["serve","--explore"]}}}"#,
        )
        .unwrap();

        reconcile_harness(&dir, None, Mode::McpLlm).unwrap();

        let doc: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join(".mcp.json")).unwrap(),
        )
        .unwrap();
        // The `grove` key must now be the LOCATOR, not the stale structural
        // entry: empty args (the old `serve --explore` is gone — and `--explore`
        // is a hard error post-T03, so a stray copy would fail at launch), and
        // the command pointing at the grove-explore binary.
        assert_eq!(
            doc["mcpServers"]["grove"]["args"],
            serde_json::json!([]),
            "forward migration: grove must become the locator (empty args)"
        );
        assert!(
            doc["mcpServers"]["grove"]["command"]
                .as_str()
                .unwrap_or("")
                .ends_with("grove-explore"),
            "forward migration: grove must run the grove-explore binary"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_agents_md_creates_then_updates_idempotently() {
        let dir = tmp("agents_idem");
        let path = dir.join("AGENTS.md");

        write_agents_md(&dir, &["rust".into()], Target::McpLlm).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        assert_eq!(first.matches(CLAUDE_START).count(), 1);
        assert!(first.contains("rust"));

        // Re-run: section replaced in place, not duplicated.
        write_agents_md(&dir, &["python".into()], Target::McpLlm).unwrap();
        let second = std::fs::read_to_string(&path).unwrap();
        assert_eq!(second.matches(CLAUDE_START).count(), 1, "exactly one grove section");
        assert_eq!(second.matches(CLAUDE_END).count(), 1);
        assert!(second.contains("python"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_agents_md_appends_below_existing_content() {
        let dir = tmp("agents_append");
        let path = dir.join("AGENTS.md");
        std::fs::write(&path, "# My project\n\nHand-written notes.\n").unwrap();

        write_agents_md(&dir, &["rust".into()], Target::McpLlm).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Hand-written notes."), "existing content preserved");
        assert!(text.contains(CLAUDE_START), "grove section appended");
        assert!(text.find("Hand-written").unwrap() < text.find(CLAUDE_START).unwrap());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_explore_server_registers_in_grove_key() {
        let dir = tmp("explore_server_fresh");
        write_json_mcp_explore_server(&dir.join(".mcp.json"), "mcpServers", false).unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
        // The locator registers under the `grove` key with empty args, so its
        // tool resolves as `mcp__grove__explore`.
        assert!(
            !doc["mcpServers"]["grove"].is_null(),
            "grove (locator) entry must be present"
        );
        assert_eq!(
            doc["mcpServers"]["grove"]["args"],
            serde_json::json!([]),
            "locator args must be empty"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_explore_server_preserves_other_servers() {
        let dir = tmp("explore_server_preserve");
        let path = dir.join(".mcp.json");
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();

        write_json_mcp_explore_server(&path, "mcpServers", false).unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["mcpServers"]["other"]["command"], serde_json::json!("x"), "existing server kept");
        // locator added under the `grove` key with empty args.
        assert!(
            !doc["mcpServers"]["grove"].is_null(),
            "grove (locator) entry added"
        );
        assert_eq!(
            doc["mcpServers"]["grove"]["args"],
            serde_json::json!([]),
            "locator args must be empty"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── pre-existing harness tests updated to use reconcile_harness ──────────

    #[test]
    fn grammars_target_writes_no_harness_files() {
        let dir = tmp("harness_grammars");
        seed_lock(&dir);
        let wrote = reconcile_harness(&dir, None, Mode::Grammars).unwrap();

        assert!(!dir.join("CLAUDE.md").exists() || {
            let t = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
            !t.contains(CLAUDE_START)
        }, "grammars mode writes no steering block");
        assert!(!dir.join(".mcp.json").exists() || {
            let doc: Value = serde_json::from_str(
                &std::fs::read_to_string(dir.join(".mcp.json")).unwrap()
            ).unwrap();
            doc.get("mcpServers").and_then(|s| s.get("grove")).is_none()
        }, "grammars mode writes no .mcp.json grove entry");
        // AGENTS.md must not have a grove block
        assert!(!dir.join("AGENTS.md").exists() || {
            let t = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
            !t.contains(CLAUDE_START)
        }, "grammars mode writes no AGENTS.md block");
        assert!(wrote.is_empty(), "no harness writes reported: {wrote:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn skill_target_writes_steering_but_no_mcp_json() {
        let dir = tmp("harness_skill");
        seed_lock(&dir);
        let wrote = reconcile_harness(&dir, None, Mode::Skill).unwrap();

        assert!(dir.join("CLAUDE.md").exists(), "skill mode writes steering");
        assert!(!dir.join(".mcp.json").exists(), "skill mode writes no .mcp.json");
        assert_eq!(wrote.len(), 1, "only steering reported: {wrote:?}");

        // steering must point at the skill/CLI, not MCP tools that don't exist here
        let steer = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
        assert!(steer.contains("rust"), "steering names the locked language");
        assert!(!steer.contains("mcp__grove__"), "skill steering names no MCP tools");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mcp_target_writes_mcp_json_and_steering() {
        let dir = tmp("harness_mcp");
        seed_lock(&dir);
        let wrote = reconcile_harness(&dir, None, Mode::Mcp).unwrap();

        assert!(dir.join(".mcp.json").exists());
        assert!(dir.join("CLAUDE.md").exists());
        assert!(dir.join("AGENTS.md").exists(), "standard MCP mode also writes AGENTS.md");
        // .mcp.json registration + CLAUDE.md + AGENTS.md.
        assert_eq!(wrote.len(), 3, "registration + CLAUDE.md + AGENTS.md: {wrote:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn both_target_writes_mcp_json_and_steering() {
        let dir = tmp("harness_both");
        seed_lock(&dir);
        reconcile_harness(&dir, None, Mode::Both).unwrap();
        assert!(dir.join(".mcp.json").exists());
        assert!(dir.join("CLAUDE.md").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── --agents parsing + detection tests ──────────────────────────────────

    #[test]
    fn parse_agents_all_and_explicit_list_dedup() {
        let root = std::path::Path::new(".");
        assert_eq!(parse_agents_spec("all", root).unwrap(), HarnessId::ALL.to_vec());
        // Whitespace-tolerant, order-preserving, de-duplicated.
        assert_eq!(
            parse_agents_spec(" cursor, codex ,cursor ", root).unwrap(),
            vec![HarnessId::Cursor, HarnessId::Codex]
        );
    }

    #[test]
    fn parse_agents_rejects_unknown_and_empty() {
        let root = std::path::Path::new(".");
        let err = parse_agents_spec("emacs", root).unwrap_err().to_string();
        assert!(err.contains("emacs"), "names the bad slug: {err}");
        assert!(parse_agents_spec(" , ", root).is_err(), "no valid agents → error");
    }

    #[test]
    fn resolve_harness_set_prefers_spec_then_preserves_old() {
        let root = std::path::Path::new(".");
        // Explicit spec wins.
        assert_eq!(
            resolve_harness_set(Some("all"), None, root).unwrap(),
            HarnessId::ALL.to_vec()
        );
        // No spec + an existing config → preserve its set (idempotent re-runs).
        let old = GroveConfig { mode: Mode::Mcp, harnesses: vec![HarnessId::Cursor], ..Default::default() };
        assert_eq!(
            resolve_harness_set(None, Some(&old), root).unwrap(),
            vec![HarnessId::Cursor]
        );
    }

    #[test]
    fn harness_present_detects_project_marker() {
        let dir = tmp("detect_marker");
        std::fs::create_dir_all(dir.join(".cursor")).unwrap();
        assert!(harness_present(HarnessId::Cursor, &dir), "detects .cursor/ marker");
        // Windsurf marker absent here.
        assert!(!harness_present(HarnessId::Windsurf, &dir) || which::which("windsurf").is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn harness_targets_flags_codex_global_and_lists_steering() {
        let root = std::path::Path::new("/proj");
        let home = std::path::Path::new("/home/u");
        let targets = harness_targets(root, home, Mode::Mcp, &[HarnessId::ClaudeCode, HarnessId::Codex]);
        assert!(targets.iter().any(|t| t.contains(".mcp.json") && t.contains("Claude Code")));
        assert!(
            targets.iter().any(|t| t.contains("user-global") && t.contains("Codex")),
            "Codex flagged user-global: {targets:?}"
        );
        assert!(targets.iter().any(|t| t.starts_with("CLAUDE.md")), "claude steering listed");
        assert!(targets.iter().any(|t| t.starts_with("AGENTS.md")), "agents steering listed");
    }

    // ─── multi-harness reconcile tests (project-scoped harnesses) ─────────────
    // These pass `home = dir` so any global-scope harness would resolve under the
    // temp dir; they exercise only project harnesses (Codex is covered separately).

    #[test]
    fn reconcile_multi_harness_writes_each_registration() {
        let dir = tmp("multi_write");
        seed_lock(&dir);
        let set = [
            HarnessId::ClaudeCode,
            HarnessId::Cursor,
            HarnessId::Gemini,
            HarnessId::Windsurf,
            HarnessId::VsCode,
        ];
        reconcile_harness_for(&dir, &dir, None, Mode::Mcp, &set).unwrap();

        // Each harness's registration lands at its own path/format.
        assert!(dir.join(".mcp.json").exists(), "claude-code");
        assert!(dir.join(".cursor").join("mcp.json").exists(), "cursor");
        assert!(dir.join(".gemini").join("settings.json").exists(), "gemini");
        assert!(dir.join(".windsurf").join("mcp.json").exists(), "windsurf");
        // VS Code: `servers` root key + type:stdio.
        let vscode: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(".vscode").join("mcp.json")).unwrap())
                .unwrap();
        assert_eq!(vscode["servers"]["grove"]["type"], json!("stdio"));
        assert!(vscode.get("mcpServers").is_none(), "VS Code must not use mcpServers");
        // Shared steering: CLAUDE.md (claude selected) + AGENTS.md (any MCP surface).
        assert!(dir.join("CLAUDE.md").exists());
        assert!(dir.join("AGENTS.md").exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reconcile_deselecting_harness_strips_its_registration() {
        let dir = tmp("multi_deselect");
        seed_lock(&dir);
        reconcile_harness_for(&dir, &dir, None, Mode::Mcp, &[HarnessId::ClaudeCode, HarnessId::Cursor])
            .unwrap();
        assert!(dir.join(".cursor").join("mcp.json").exists());

        // Re-run with claude-code only → the cursor grove entry is stripped.
        reconcile_harness_for(&dir, &dir, Some(Mode::Mcp), Mode::Mcp, &[HarnessId::ClaudeCode]).unwrap();
        let cursor: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(".cursor").join("mcp.json")).unwrap())
                .unwrap();
        assert!(
            cursor.get("mcpServers").and_then(|s| s.get("grove")).is_none(),
            "cursor grove entry stripped after de-selection"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reconcile_codex_writes_global_toml_under_injected_home() {
        let dir = tmp("codex_write");
        seed_lock(&dir);
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        // Pre-seed an unrelated Codex server that must survive the merge.
        let codex_cfg = home.join(".codex").join("config.toml");
        std::fs::create_dir_all(codex_cfg.parent().unwrap()).unwrap();
        std::fs::write(&codex_cfg, "[mcp_servers.other]\ncommand = \"x\"\n").unwrap();

        let wrote = reconcile_harness_for(
            &dir,
            &home,
            None,
            Mode::Mcp,
            &[HarnessId::Codex],
        )
        .unwrap();

        // grove registered under the global ~/.codex path, other server preserved.
        let doc: toml_edit::DocumentMut =
            std::fs::read_to_string(&codex_cfg).unwrap().parse().unwrap();
        assert!(doc["mcp_servers"]["grove"]["command"].as_str().is_some(), "grove written");
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("x"), "other preserved");
        // Nothing was written under the project root for Codex.
        assert!(!dir.join(".codex").exists(), "Codex config is global, not project-local");
        // The status line shows the absolute (global) path, flagged by harness name.
        assert!(
            wrote.iter().any(|w| w.contains("Codex") && w.contains(".codex")),
            "status names Codex + its global path: {wrote:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reconcile_without_claude_strips_claude_md_keeps_agents() {
        let dir = tmp("no_claude");
        seed_lock(&dir);
        reconcile_harness_for(&dir, &dir, None, Mode::Mcp, &[HarnessId::Cursor]).unwrap();

        assert!(dir.join(".cursor").join("mcp.json").exists(), "cursor wired");
        assert!(dir.join("AGENTS.md").exists(), "AGENTS.md steers the non-claude harness");
        // No claude registration; CLAUDE.md, if present, carries no grove block.
        assert!(!dir.join(".mcp.json").exists(), "claude not selected → no .mcp.json");
        if dir.join("CLAUDE.md").exists() {
            let c = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
            assert!(!c.contains(CLAUDE_START), "no grove block in CLAUDE.md when claude deselected");
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
