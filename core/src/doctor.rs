//! `core::doctor` — project health diagnostics.
//!
//! The public entry point is [`diagnose`], which returns a [`Report`]
//! describing the health of a grove project at `root`.  The function is
//! **pure and read-only**: it never writes to disk and performs at most one
//! outbound network request (the explore-mode health probe).

use std::path::Path;

use crate::{
    config::{active_mode, default_harnesses, GroveConfig, Mode, ModeChoice},
    harness::{self, HarnessId, McpFormat},
    registry::{self, LockVerifyStatus},
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Severity of a single diagnostic check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// All good.
    Ok,
    /// Non-fatal issue; should be addressed but does not cause [`Report::ok`]
    /// to return `false`.
    Warn,
    /// Hard failure; [`Report::ok`] returns `false` when any check is `Fail`.
    Fail,
    /// Informational — no pass/fail semantics.
    Info,
}

/// A single diagnostic result.
#[derive(Debug, Clone)]
pub struct Check {
    /// Check group: `"universal"` or `"explore"`.
    pub group: &'static str,
    /// Stable machine-readable name (snake_case).
    pub name: &'static str,
    pub status: Status,
    /// Human-readable detail string.
    pub detail: String,
    /// Optional remediation hint.
    pub hint: Option<String>,
}

/// The full diagnostic report for a project.
#[derive(Debug)]
pub struct Report {
    /// The declared integration mode from `.grove/config.json`
    /// (or `Mode::Mcp` when no config is present).
    pub mode: Mode,
    /// All checks, in emission order.
    pub checks: Vec<Check>,
}

impl Report {
    /// `true` when no check has [`Status::Fail`].  Warnings and Info are
    /// pass-grade; only hard failures drive the exit code to 1.
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| !matches!(c.status, Status::Fail))
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run all applicable diagnostic checks for the grove project at `root`.
///
/// `force` mirrors the `--explore` / `--standard` CLI flags; pass
/// [`ModeChoice::None`] for auto-detection from the project config.
///
/// The function is read-only and pure except for the optional network probe
/// in explore-mode checks.
pub fn diagnose(root: &Path, force: ModeChoice) -> Report {
    let mut checks: Vec<Check> = Vec::new();

    // ── grove_version (Info) ────────────────────────────────────────────────
    checks.push(Check {
        group: "universal",
        name: "grove_version",
        status: Status::Info,
        detail: env!("CARGO_PKG_VERSION").to_string(),
        hint: None,
    });

    // ── config_present (Ok / Fail / Info) ──────────────────────────────────
    let cfg_result = GroveConfig::load(root);
    let declared_mode = match &cfg_result {
        Ok(cfg) => cfg.mode,
        Err(_) => Mode::Mcp,
    };
    // Determine whether any selected harness has registered the explore server.
    // This gates both the explore check group and the reported serve surface.
    let harnesses = cfg_result
        .as_ref()
        .ok()
        .map(|c| c.harnesses.clone())
        .unwrap_or_else(default_harnesses);
    let home = dirs::home_dir().unwrap_or_else(|| root.to_path_buf());
    let has_explore_reg = has_explore_registration(root, &home, &harnesses);

    match &cfg_result {
        Ok(cfg) => {
            checks.push(Check {
                group: "universal",
                name: "config_present",
                status: Status::Ok,
                detail: format!(
                    "{} · mode={}",
                    GroveConfig::config_path(root).display(),
                    mode_name(cfg.mode)
                ),
                hint: None,
            });
        }
        Err(e) => {
            if !GroveConfig::config_path(root).exists() {
                checks.push(Check {
                    group: "universal",
                    name: "config_present",
                    status: Status::Info,
                    detail: "no .grove/config.json — project not yet initialized".to_string(),
                    hint: Some("grove init".to_string()),
                });
            } else {
                checks.push(Check {
                    group: "universal",
                    name: "config_present",
                    status: Status::Fail,
                    detail: format!(
                        "could not load {}: {e}",
                        GroveConfig::config_path(root).display()
                    ),
                    hint: Some("grove init --as <mode>".to_string()),
                });
            }
        }
    }

    // ── legacy_explore_json (Warn) ──────────────────────────────────────────
    let legacy_path = root.join(".grove").join("explore.json");
    let config_path = GroveConfig::config_path(root);
    if legacy_path.exists() && !config_path.exists() {
        checks.push(Check {
            group: "universal",
            name: "legacy_explore_json",
            status: Status::Warn,
            detail: ".grove/explore.json present without .grove/config.json — \
                     run `grove init` to migrate"
                .to_string(),
            hint: Some("grove init --as mcp-llm".to_string()),
        });
    }

    // Resolve the effective mode for harness checks from the declared config.
    let mode = active_mode(root, force);

    // The configured harness set drives per-agent registration checks. Absent a
    // readable config, fall back to the historical Claude-Code-only expectation.
    let claude_selected = harnesses.contains(&HarnessId::ClaudeCode);

    // ── harness sub-checks ──────────────────────────────────────────────────
    // Claude Code keeps its historical `.mcp.json` check (name `harness_mcp_json`);
    // every other selected harness gets a format-aware registration check.
    if claude_selected {
        checks.push(check_harness_mcp_json(root, mode));
    }
    for &h in &harnesses {
        if h != HarnessId::ClaudeCode {
            checks.push(check_harness_registration(root, &home, h, mode));
        }
    }
    checks.push(check_harness_claude_md(root, mode, claude_selected));
    checks.push(check_harness_agents_md(root, mode));
    checks.push(check_harness_serve_surface(mode, has_explore_reg));

    // ── registry_root (Ok / Fail) ───────────────────────────────────────────
    let candidates = registry::search_path();
    if candidates.iter().any(|c| c.exists) {
        let reg_root = registry::root();
        checks.push(Check {
            group: "universal",
            name: "registry_root",
            status: Status::Ok,
            detail: reg_root.display().to_string(),
            hint: None,
        });
    } else {
        checks.push(Check {
            group: "universal",
            name: "registry_root",
            status: Status::Fail,
            detail: "no registry candidate exists on disk".to_string(),
            hint: Some("grove fetch".to_string()),
        });
    }

    // ── grammar_cache (Ok / Warn) ───────────────────────────────────────────
    match registry::cache_root() {
        Some(cache) => checks.push(Check {
            group: "universal",
            name: "grammar_cache",
            status: Status::Ok,
            detail: cache.display().to_string(),
            hint: None,
        }),
        None => checks.push(Check {
            group: "universal",
            name: "grammar_cache",
            status: Status::Warn,
            detail: "no OS cache root resolved".to_string(),
            hint: None,
        }),
    }

    // ── project_languages (Ok / Warn) ───────────────────────────────────────
    {
        let lock_path = root.join("grove.lock");
        if lock_path.exists() {
            match registry::locked_langs(&lock_path) {
                Ok(langs) if !langs.is_empty() => {
                    let available = registry::available();
                    let missing: Vec<_> = langs
                        .iter()
                        .filter(|l| !available.contains(l))
                        .cloned()
                        .collect();
                    if missing.is_empty() {
                        checks.push(Check {
                            group: "universal",
                            name: "project_languages",
                            status: Status::Ok,
                            detail: langs.join(", "),
                            hint: None,
                        });
                    } else {
                        checks.push(Check {
                            group: "universal",
                            name: "project_languages",
                            status: Status::Warn,
                            detail: format!(
                                "locked: {}; missing from registry: {}",
                                langs.join(", "),
                                missing.join(", ")
                            ),
                            hint: Some("grove fetch".to_string()),
                        });
                    }
                }
                Ok(_) => checks.push(Check {
                    group: "universal",
                    name: "project_languages",
                    status: Status::Info,
                    detail: "grove.lock is empty — no languages pinned".to_string(),
                    hint: None,
                }),
                Err(e) => checks.push(Check {
                    group: "universal",
                    name: "project_languages",
                    status: Status::Warn,
                    detail: format!("could not read grove.lock: {e}"),
                    hint: None,
                }),
            }
        } else {
            checks.push(Check {
                group: "universal",
                name: "project_languages",
                status: Status::Info,
                detail: "grove.lock absent — run `grove init` to pin languages".to_string(),
                hint: Some("grove init".to_string()),
            });
        }
    }

    // ── lock_integrity (Ok / Fail / Warn) ───────────────────────────────────
    {
        let lock_path = root.join("grove.lock");
        match registry::verify_lock(&lock_path) {
            Ok(None) => checks.push(Check {
                group: "universal",
                name: "lock_integrity",
                status: Status::Warn,
                detail: "grove.lock absent".to_string(),
                hint: Some("grove init".to_string()),
            }),
            Ok(Some(entries)) => {
                let mut any_fail = false;
                let mut any_warn = false;
                let mut details = Vec::new();
                for e in &entries {
                    match e.status {
                        LockVerifyStatus::Match => {
                            details.push(format!("{}: ok", e.lang));
                        }
                        LockVerifyStatus::Mismatch => {
                            any_fail = true;
                            details.push(format!("{}: hash mismatch", e.lang));
                        }
                        LockVerifyStatus::Missing => {
                            any_warn = true;
                            details.push(format!("{}: wasm not found", e.lang));
                        }
                    }
                }
                let status = if any_fail {
                    Status::Fail
                } else if any_warn {
                    Status::Warn
                } else {
                    Status::Ok
                };
                let hint = if any_fail || any_warn {
                    Some("grove fetch".to_string())
                } else {
                    None
                };
                checks.push(Check {
                    group: "universal",
                    name: "lock_integrity",
                    status,
                    detail: details.join(", "),
                    hint,
                });
            }
            Err(e) => checks.push(Check {
                group: "universal",
                name: "lock_integrity",
                status: Status::Warn,
                detail: format!("could not verify grove.lock: {e}"),
                hint: None,
            }),
        }
    }

    // ── Explore-mode checks (grove-explore registration only) ───────────────
    if has_explore_reg {
        let explore_cfg = cfg_result.ok().and_then(|c| c.explore);
        checks.extend(explore_checks(explore_cfg.as_ref()));
    }

    Report {
        mode: declared_mode,
        checks,
    }
}

// ---------------------------------------------------------------------------
// Harness sub-checks
// ---------------------------------------------------------------------------

fn check_harness_mcp_json(root: &Path, mode: Mode) -> Check {
    let path = root.join(".mcp.json");
    let format = HarnessId::ClaudeCode.mcp_format();

    // Stale pre-split layout: a single `grove` entry still carrying `--explore`.
    if has_stale_explore_arg(&path, format) {
        return Check {
            group: "universal",
            name: "harness_mcp_json",
            status: Status::Fail,
            detail: format!(
                "mode={}: .mcp.json uses the removed `--explore` arg (stale layout)",
                mode_name(mode)
            ),
            hint: Some("grove init --as mcp-llm".to_string()),
        };
    }

    let expected_args = harness::expected_mcp_args(mode);
    let actual_args = read_grove_args(&path, format);
    let expected_explore = harness::expected_explore_args(mode);
    let actual_explore = read_server_args(&path, format, harness::EXPLORE_SERVER_KEY);

    let mut parts = Vec::new();
    let structural_ok = match (expected_args, actual_args) {
        (None, None) => {
            parts.push("no grove entry expected and none present".to_string());
            true
        }
        (None, Some(_)) => {
            parts.push(".mcp.json has a grove entry but none is expected".to_string());
            false
        }
        (Some(expected), None) => {
            parts.push(format!(
                ".mcp.json absent or no grove entry; expected args {:?}",
                expected
            ));
            false
        }
        (Some(expected), Some(actual)) => {
            let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
            if expected == actual_refs.as_slice() {
                parts.push(format!("grove args {:?}", actual));
                true
            } else {
                parts.push(format!(
                    "expected grove args {:?}, found {:?}",
                    expected, actual
                ));
                false
            }
        }
    };

    let explore_ok = match (expected_explore, actual_explore) {
        (None, None) => {
            parts.push("no grove-explore entry expected and none present".to_string());
            true
        }
        (None, Some(_)) => {
            parts.push(".mcp.json has a grove-explore entry but none is expected".to_string());
            false
        }
        (Some(expected), None) => {
            parts.push(format!(
                ".mcp.json absent or no grove-explore entry; expected args {:?}",
                expected
            ));
            false
        }
        (Some(expected), Some(actual)) => {
            let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
            if expected == actual_refs.as_slice() {
                parts.push(format!("grove-explore args {:?}", actual));
                true
            } else {
                parts.push(format!(
                    "expected grove-explore args {:?}, found {:?}",
                    expected, actual
                ));
                false
            }
        }
    };

    if structural_ok && explore_ok {
        Check {
            group: "universal",
            name: "harness_mcp_json",
            status: Status::Ok,
            detail: format!("mode={}: {}", mode_name(mode), parts.join("; ")),
            hint: None,
        }
    } else {
        let hint = if expected_explore.is_some() || mode == Mode::McpLlm {
            "grove init --as mcp-llm".to_string()
        } else {
            format!("grove init --as {}", mode_name(mode))
        };
        Check {
            group: "universal",
            name: "harness_mcp_json",
            status: Status::Fail,
            detail: format!("mode={}: {}", mode_name(mode), parts.join("; ")),
            hint: Some(hint),
        }
    }
}

/// Read a registered server's `args` from a harness MCP config, format-aware:
/// JSON (`<root_key>.<server>.args`) or TOML (`[<table>.<server>] args`).
/// `None` when the file is absent/unparseable or has no entry for `server_key`.
fn read_server_args(path: &Path, format: McpFormat, server_key: &str) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(path).ok()?;
    match format {
        McpFormat::Json { root_key, .. } => {
            let doc: serde_json::Value = serde_json::from_str(&text).ok()?;
            let args = doc[root_key][server_key]["args"].as_array()?;
            Some(args.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        }
        McpFormat::Toml { table } => {
            let doc: toml_edit::DocumentMut = text.parse().ok()?;
            let args = doc.get(table)?.get(server_key)?.get("args")?.as_array()?;
            Some(args.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        }
    }
}

/// Convenience wrapper for the structural `grove` server entry.
fn read_grove_args(path: &Path, format: McpFormat) -> Option<Vec<String>> {
    read_server_args(path, format, harness::MCP_SERVER_KEY)
}

/// `true` if any selected harness registers a `grove-explore` server.
fn has_explore_registration(root: &Path, home: &Path, harnesses: &[HarnessId]) -> bool {
    harnesses.iter().any(|&h| {
        let path = h.mcp_config_path_in(root, Some(home));
        read_server_args(&path, h.mcp_format(), harness::EXPLORE_SERVER_KEY).is_some()
    })
}

/// `true` if the structural `grove` entry in a harness still uses the removed
/// `--explore` argument (pre-split `mcp-llm` layout).
fn has_stale_explore_arg(path: &Path, format: McpFormat) -> bool {
    read_grove_args(path, format)
        .map(|args| args.iter().any(|a| a == "--explore"))
        .unwrap_or(false)
}

/// The stable check name for a harness's registration (`harness_mcp_<slug>`).
fn harness_reg_check_name(h: HarnessId) -> &'static str {
    match h {
        HarnessId::ClaudeCode => "harness_mcp_json",
        HarnessId::Cursor => "harness_mcp_cursor",
        HarnessId::Codex => "harness_mcp_codex",
        HarnessId::Gemini => "harness_mcp_gemini",
        HarnessId::Windsurf => "harness_mcp_windsurf",
        HarnessId::VsCode => "harness_mcp_vscode",
    }
}

/// Verify one non-Claude harness's MCP registration against the expected args
/// for `mode`, reading its own path + format. Mirrors [`check_harness_mcp_json`]
/// but generalized across harnesses (VS Code's `servers` key, Codex's TOML, …).
fn check_harness_registration(root: &Path, home: &Path, h: HarnessId, mode: Mode) -> Check {
    let name = harness_reg_check_name(h);
    let path = h.mcp_config_path_in(root, Some(home));
    let label = h.display_name();
    let format = h.mcp_format();

    // Stale pre-split layout: a single `grove` entry still carrying `--explore`.
    if has_stale_explore_arg(&path, format) {
        return Check {
            group: "universal",
            name,
            status: Status::Fail,
            detail: format!(
                "{label}: registration uses the removed `--explore` arg (stale layout)"
            ),
            hint: Some("grove init --as mcp-llm".to_string()),
        };
    }

    let expected_args = harness::expected_mcp_args(mode);
    let actual_args = read_grove_args(&path, format);
    let expected_explore = harness::expected_explore_args(mode);
    let actual_explore = read_server_args(&path, format, harness::EXPLORE_SERVER_KEY);

    let mut parts = Vec::new();
    let structural_ok = match (expected_args, actual_args) {
        (None, None) => {
            parts.push("no grove entry expected and none present".to_string());
            true
        }
        (None, Some(_)) => {
            parts.push("registration has a grove entry but none is expected".to_string());
            false
        }
        (Some(expected), None) => {
            parts.push(format!(
                "registration absent or no grove entry; expected args {expected:?}"
            ));
            false
        }
        (Some(expected), Some(actual)) => {
            let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
            if expected == actual_refs.as_slice() {
                parts.push(format!("grove args {actual:?}"));
                true
            } else {
                parts.push(format!("expected grove args {expected:?}, found {actual:?}"));
                false
            }
        }
    };

    let explore_ok = match (expected_explore, actual_explore) {
        (None, None) => {
            parts.push("no grove-explore entry expected and none present".to_string());
            true
        }
        (None, Some(_)) => {
            parts.push(
                "registration has a grove-explore entry but none is expected".to_string(),
            );
            false
        }
        (Some(expected), None) => {
            parts.push(format!(
                "registration absent or no grove-explore entry; expected args {expected:?}"
            ));
            false
        }
        (Some(expected), Some(actual)) => {
            let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
            if expected == actual_refs.as_slice() {
                parts.push(format!("grove-explore args {actual:?}"));
                true
            } else {
                parts.push(format!(
                    "expected grove-explore args {expected:?}, found {actual:?}"
                ));
                false
            }
        }
    };

    if structural_ok && explore_ok {
        Check {
            group: "universal",
            name,
            status: Status::Ok,
            detail: format!("{label}: {}", parts.join("; ")),
            hint: None,
        }
    } else {
        let hint = if expected_explore.is_some() || mode == Mode::McpLlm {
            "grove init --as mcp-llm".to_string()
        } else {
            format!("grove init --as {}", mode_name(mode))
        };
        Check {
            group: "universal",
            name,
            status: Status::Fail,
            detail: format!("{label}: {}", parts.join("; ")),
            hint: Some(hint),
        }
    }
}

fn check_harness_claude_md(root: &Path, mode: Mode, claude_selected: bool) -> Check {
    let path = root.join("CLAUDE.md");
    // When Claude Code isn't in the harness set, CLAUDE.md should carry no grove
    // block (same expectation as Grammars mode) — so drop the marker requirement.
    let expected_marker = if claude_selected {
        harness::expected_claude_marker(mode)
    } else {
        None
    };
    let text = std::fs::read_to_string(&path).ok();

    match expected_marker {
        // Grammars mode: grove block must be absent
        None => {
            let has_block = text
                .as_deref()
                .map(|t| t.contains(harness::GROVE_START))
                .unwrap_or(false);
            if has_block {
                Check {
                    group: "universal",
                    name: "harness_claude_md",
                    status: Status::Warn,
                    detail: format!(
                        "mode={}: CLAUDE.md has a grove block but none is expected",
                        mode_name(mode)
                    ),
                    hint: Some(format!("grove init --as {}", mode_name(mode))),
                }
            } else {
                Check {
                    group: "universal",
                    name: "harness_claude_md",
                    status: Status::Ok,
                    detail: format!(
                        "mode={}: no grove block expected and none present",
                        mode_name(mode)
                    ),
                    hint: None,
                }
            }
        }
        // All other modes: file must exist, contain GROVE_START, and contain marker
        Some(marker) => match text.as_deref() {
            None => Check {
                group: "universal",
                name: "harness_claude_md",
                status: Status::Fail,
                detail: format!("mode={}: CLAUDE.md is absent", mode_name(mode)),
                hint: Some(format!("grove init --as {}", mode_name(mode))),
            },
            Some(t) if !t.contains(harness::GROVE_START) => Check {
                group: "universal",
                name: "harness_claude_md",
                status: Status::Fail,
                detail: format!(
                    "mode={}: CLAUDE.md has no grove sentinel block",
                    mode_name(mode)
                ),
                hint: Some(format!("grove init --as {}", mode_name(mode))),
            },
            Some(t) if !t.contains(marker) => Check {
                group: "universal",
                name: "harness_claude_md",
                status: Status::Fail,
                detail: format!(
                    "mode={}: CLAUDE.md block is missing expected marker {:?}",
                    mode_name(mode),
                    marker
                ),
                hint: Some(format!("grove init --as {}", mode_name(mode))),
            },
            Some(_) => Check {
                group: "universal",
                name: "harness_claude_md",
                status: Status::Ok,
                detail: format!(
                    "mode={}: grove block present with {:?}",
                    mode_name(mode),
                    marker
                ),
                hint: None,
            },
        },
    }
}

fn check_harness_agents_md(root: &Path, mode: Mode) -> Check {
    let path = root.join("AGENTS.md");
    let expected = harness::agents_md_expected(mode);
    let text = std::fs::read_to_string(&path).ok();

    if expected {
        // File must exist and contain GROVE_START
        match text.as_deref() {
            None => Check {
                group: "universal",
                name: "harness_agents_md",
                status: Status::Warn,
                detail: format!(
                    "mode={}: AGENTS.md absent (expected for explore mode)",
                    mode_name(mode)
                ),
                hint: Some(format!("grove init --as {}", mode_name(mode))),
            },
            Some(t) if !t.contains(harness::GROVE_START) => Check {
                group: "universal",
                name: "harness_agents_md",
                status: Status::Warn,
                detail: format!(
                    "mode={}: AGENTS.md has no grove sentinel block",
                    mode_name(mode)
                ),
                hint: Some(format!("grove init --as {}", mode_name(mode))),
            },
            Some(_) => Check {
                group: "universal",
                name: "harness_agents_md",
                status: Status::Ok,
                detail: format!("mode={}: grove block present in AGENTS.md", mode_name(mode)),
                hint: None,
            },
        }
    } else {
        // File may exist but must NOT contain a grove block
        let has_block = text
            .as_deref()
            .map(|t| t.contains(harness::GROVE_START))
            .unwrap_or(false);
        if has_block {
            Check {
                group: "universal",
                name: "harness_agents_md",
                status: Status::Warn,
                detail: format!(
                    "mode={}: AGENTS.md has a grove block but none is expected",
                    mode_name(mode)
                ),
                hint: Some(format!("grove init --as {}", mode_name(mode))),
            }
        } else {
            Check {
                group: "universal",
                name: "harness_agents_md",
                status: Status::Ok,
                detail: format!(
                    "mode={}: no grove block in AGENTS.md (correct)",
                    mode_name(mode)
                ),
                hint: None,
            }
        }
    }
}

fn check_harness_serve_surface(mode: Mode, has_explore_reg: bool) -> Check {
    let surface = if has_explore_reg {
        "Explore (when provider is healthy)"
    } else {
        "Standard"
    };

    Check {
        group: "universal",
        name: "harness_serve_surface",
        status: Status::Info,
        detail: format!("{surface} · mode={}", mode_name(mode)),
        hint: None,
    }
}

// ---------------------------------------------------------------------------
// Explore-mode checks
// ---------------------------------------------------------------------------

fn explore_checks(cfg: Option<&serde_json::Value>) -> Vec<Check> {
    let mut checks = Vec::new();

    // ── explore_config_valid ─────────────────────────────────────────────────
    // Config-based checks only (HTTP health probe removed — health_probe lives
    // in grove-explore-core which grove-cst cannot depend on; probe still fires
    // at `grove serve` startup via determine_surface in cli/src/mcp.rs).
    let cfg = match cfg {
        None => {
            checks.push(Check {
                group: "explore",
                name: "explore_config_valid",
                status: Status::Fail,
                detail: "explore section missing from .grove/config.json".to_string(),
                hint: Some("grove config".to_string()),
            });
            return checks;
        }
        Some(v) => {
            // Validate that the required non-empty string fields are present.
            let base_url = v.get("base_url").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
            let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
            if base_url.is_empty() {
                checks.push(Check {
                    group: "explore",
                    name: "explore_config_valid",
                    status: Status::Fail,
                    detail: "`base_url` must not be empty".to_string(),
                    hint: Some("grove config".to_string()),
                });
                return checks;
            }
            if model.is_empty() {
                checks.push(Check {
                    group: "explore",
                    name: "explore_config_valid",
                    status: Status::Fail,
                    detail: "`model` must not be empty".to_string(),
                    hint: Some("grove config".to_string()),
                });
                return checks;
            }
            checks.push(Check {
                group: "explore",
                name: "explore_config_valid",
                status: Status::Ok,
                detail: format!("base_url={base_url} model={model}"),
                hint: None,
            });
            v
        }
    };

    // ── allowed_tools_known (Ok / Warn) ──────────────────────────────────────
    {
        // The inner explorer's base tool names plus the config's tool-family
        // tokens (`allowed_tools` is advisory — grove is exposed as six
        // `mcp__grove__*` tools, not a single `Grove` command tool).
        const KNOWN: &[&str] = &["Read", "Glob", "Grep", "grove", "rg", "grep", "find"];
        let allowed_tools: Vec<String> = cfg
            .get("allowed_tools")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let unknown: Vec<_> = allowed_tools
            .iter()
            .filter(|t| !KNOWN.contains(&t.as_str()))
            .cloned()
            .collect();
        if unknown.is_empty() {
            checks.push(Check {
                group: "explore",
                name: "allowed_tools_known",
                status: Status::Ok,
                detail: format!("tools: {}", allowed_tools.join(", ")),
                hint: None,
            });
        } else {
            checks.push(Check {
                group: "explore",
                name: "allowed_tools_known",
                status: Status::Warn,
                detail: format!("unrecognized tools: {}", unknown.join(", ")),
                hint: Some("check allowed_tools in .grove/config.json".to_string()),
            });
        }
    }

    // ── tap_config (Info) ────────────────────────────────────────────────────
    let tap = cfg.get("tap").and_then(|v| v.as_bool()).unwrap_or(false);
    let trace_retain = cfg.get("trace_retain").and_then(|v| v.as_u64()).unwrap_or(50);
    checks.push(Check {
        group: "explore",
        name: "tap_config",
        status: Status::Info,
        detail: format!("tap={tap} trace_retain={trace_retain}"),
        hint: None,
    });

    checks
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Mcp => "mcp",
        Mode::Skill => "skill",
        Mode::Both => "both",
        Mode::McpLlm => "mcp-llm",
        Mode::Grammars => "grammars",
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("grove_doctor_{}_{tag}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_config(dir: &Path, mode: &str) {
        let grove = dir.join(".grove");
        fs::create_dir_all(&grove).unwrap();
        fs::write(
            grove.join("config.json"),
            format!(r#"{{"version":1,"mode":"{mode}"}}"#),
        )
        .unwrap();
    }

    fn write_mcp_json(dir: &Path, args: &[&str]) {
        let exe = std::env::current_exe().unwrap();
        let args_json: Vec<_> = args.iter().map(|a| format!(r#""{a}""#)).collect();
        let args_str = args_json.join(",");
        fs::write(
            dir.join(".mcp.json"),
            format!(
                r#"{{"mcpServers":{{"grove":{{"command":"{}","args":[{args_str}]}}}}}}"#,
                exe.display()
            ),
        )
        .unwrap();
    }

    /// Seed a `.mcp.json` that includes both the `grove` structural entry AND the
    /// `grove-explore` explore-server entry, reflecting the dual-server McpLlm layout.
    fn write_explore_mcp_json(dir: &Path) {
        let exe = std::env::current_exe().unwrap();
        let exe_path = exe.display();
        // McpLlm registers grove-explore ONLY (args []); the structural `grove`
        // entry must be absent — the surfaces are mutually exclusive (ADR 0005).
        fs::write(
            dir.join(".mcp.json"),
            format!(
                r#"{{"mcpServers":{{"grove-explore":{{"command":"{exe_path}","args":[]}}}}}}"#
            ),
        )
        .unwrap();
    }

    /// Seed a Cursor `.cursor/mcp.json` with optional `grove-explore` registration.
    fn write_cursor_mcp_json(dir: &Path, args: &[&str], explore_args: Option<&[&str]>) {
        let exe = std::env::current_exe().unwrap();
        let exe_path = exe.display();
        let args_json: Vec<_> = args.iter().map(|a| format!(r#""{a}""#)).collect();
        let args_str = args_json.join(",");
        // An empty `args` slice means "no structural entry at all" — McpLlm
        // registers grove-explore alone, so the fixture must omit `grove`
        // rather than write it with empty args.
        let grove_entry = if args.is_empty() {
            String::new()
        } else {
            format!(r#""grove":{{"command":"{exe_path}","args":[{args_str}]}}"#)
        };
        let body = match explore_args {
            Some(exp) => {
                let exp_json: Vec<_> = exp.iter().map(|a| format!(r#""{a}""#)).collect();
                let exp_str = exp_json.join(",");
                let sep = if grove_entry.is_empty() { "" } else { "," };
                format!(
                    r#"{{"mcpServers":{{{grove_entry}{sep}"grove-explore":{{"command":"{exe_path}","args":[{exp_str}]}}}}}}"#
                )
            }
            None => format!(r#"{{"mcpServers":{{{grove_entry}}}}}"#),
        };
        fs::create_dir_all(dir.join(".cursor")).unwrap();
        fs::write(dir.join(".cursor").join("mcp.json"), body).unwrap();
    }

    fn write_claude_md(dir: &Path, marker: &str) {
        fs::write(
            dir.join("CLAUDE.md"),
            format!("{}\n## grove\n{}\n{}\n", harness::GROVE_START, marker, harness::GROVE_END),
        )
        .unwrap();
    }

    fn write_agents_md(dir: &Path) {
        fs::write(
            dir.join("AGENTS.md"),
            format!(
                "{}\n## grove explore\n{}\n",
                harness::GROVE_START,
                harness::GROVE_END
            ),
        )
        .unwrap();
    }

    // ── warn_only_report_exits_zero ───────────────────────────────────────────

    #[test]
    fn warn_only_report_exits_zero() {
        let report = Report {
            mode: Mode::Mcp,
            checks: vec![
                Check {
                    group: "universal",
                    name: "something",
                    status: Status::Warn,
                    detail: "a warning".to_string(),
                    hint: None,
                },
                Check {
                    group: "universal",
                    name: "another",
                    status: Status::Info,
                    detail: "info".to_string(),
                    hint: None,
                },
            ],
        };
        assert!(report.ok(), "warn-only report must return ok=true");
    }

    // ── harness consistency matrix ────────────────────────────────────────────

    fn seed_harness_for_mode(dir: &Path, mode: Mode) {
        match mode {
            Mode::Mcp | Mode::Both => {
                write_mcp_json(dir, &["serve"]);
                write_claude_md(dir, "mcp__grove__outline");
                // Standard MCP surfaces now also carry the shared AGENTS.md block.
                write_agents_md(dir);
            }
            Mode::McpLlm => {
                // Explore server only — no structural `grove` entry (ADR 0005).
                write_explore_mcp_json(dir);
                // CLAUDE.md marker is now in the grove-explore server namespace.
                write_claude_md(dir, "mcp__grove-explore__explore");
                write_agents_md(dir);
            }
            Mode::Skill => {
                write_claude_md(dir, "grove skill");
            }
            Mode::Grammars => {
                // no harness files
            }
        }
    }

    #[test]
    fn harness_matrix_clean_fixtures_all_ok() {
        let modes = [
            ("mcp", Mode::Mcp),
            ("skill", Mode::Skill),
            ("both", Mode::Both),
            ("mcp-llm", Mode::McpLlm),
            ("grammars", Mode::Grammars),
        ];
        for (mode_str, mode) in &modes {
            let dir = tmp(&format!("matrix_{mode_str}"));
            write_config(&dir, mode_str);
            seed_harness_for_mode(&dir, *mode);

            let report = diagnose(&dir, ModeChoice::None);
            let harness_checks: Vec<_> = report
                .checks
                .iter()
                .filter(|c| c.name.starts_with("harness_") && c.name != "harness_serve_surface")
                .collect();
            for chk in &harness_checks {
                assert!(
                    matches!(chk.status, Status::Ok),
                    "mode={mode_str}: check {} was {:?}: {}",
                    chk.name,
                    chk.status,
                    chk.detail
                );
            }
        }
    }

    // ── drift scenarios ────────────────────────────────────────────────────────

    #[test]
    fn multi_harness_config_checks_each_registration() {
        let dir = tmp("multi_doctor_ok");
        std::fs::create_dir_all(dir.join(".grove")).unwrap();
        std::fs::write(
            dir.join(".grove").join("config.json"),
            r#"{"version":1,"mode":"mcp","harnesses":["claude-code","cursor"]}"#,
        )
        .unwrap();
        write_mcp_json(dir.as_path(), &["serve"]);
        write_claude_md(&dir, "mcp__grove__outline");
        write_agents_md(&dir);
        // Seed the cursor registration at its own path.
        std::fs::create_dir_all(dir.join(".cursor")).unwrap();
        std::fs::write(
            dir.join(".cursor").join("mcp.json"),
            r#"{"mcpServers":{"grove":{"command":"g","args":["serve"]}}}"#,
        )
        .unwrap();

        let report = diagnose(&dir, ModeChoice::None);
        let cursor = report.checks.iter().find(|c| c.name == "harness_mcp_cursor").unwrap();
        assert!(matches!(cursor.status, Status::Ok), "cursor OK: {}", cursor.detail);
        let claude = report.checks.iter().find(|c| c.name == "harness_mcp_json").unwrap();
        assert!(matches!(claude.status, Status::Ok), "claude OK: {}", claude.detail);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn multi_harness_missing_cursor_registration_is_fail() {
        let dir = tmp("multi_doctor_fail");
        std::fs::create_dir_all(dir.join(".grove")).unwrap();
        std::fs::write(
            dir.join(".grove").join("config.json"),
            r#"{"version":1,"mode":"mcp","harnesses":["claude-code","cursor"]}"#,
        )
        .unwrap();
        write_mcp_json(dir.as_path(), &["serve"]);
        write_claude_md(&dir, "mcp__grove__outline");
        write_agents_md(&dir);
        // Cursor selected but its registration is missing → Fail.
        let report = diagnose(&dir, ModeChoice::None);
        let cursor = report.checks.iter().find(|c| c.name == "harness_mcp_cursor").unwrap();
        assert!(matches!(cursor.status, Status::Fail), "missing cursor reg → Fail: {}", cursor.detail);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mcp_mode_with_explore_args_in_mcp_json_is_fail() {
        let dir = tmp("drift_mcp_json");
        write_config(&dir, "mcp");
        write_mcp_json(dir.as_path(), &["serve", "--explore"]); // stale layout for mcp mode
        write_claude_md(&dir, "mcp__grove__outline");

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "harness_mcp_json")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Fail),
            "expected Fail for stale --explore arg, got {:?}: {}",
            chk.status,
            chk.detail
        );
        assert!(
            chk.hint.as_deref() == Some("grove init --as mcp-llm"),
            "stale-layout hint should mention mcp-llm: {:?}",
            chk.hint
        );
    }

    #[test]
    fn mcp_llm_stale_layout_single_registration_is_fail() {
        let dir = tmp("drift_stale_layout");
        write_config(&dir, "mcp-llm");
        // Pre-split single-server layout: grove carries `--explore`, no grove-explore.
        write_mcp_json(dir.as_path(), &["serve", "--explore"]);
        write_claude_md(&dir, "mcp__grove-explore__explore");
        write_agents_md(&dir);

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "harness_mcp_json")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Fail),
            "expected Fail for stale single-registration layout, got {:?}: {}",
            chk.status,
            chk.detail
        );
        assert!(
            chk.hint.as_deref() == Some("grove init --as mcp-llm"),
            "stale-layout hint should mention mcp-llm: {:?}",
            chk.hint
        );
    }

    #[test]
    fn mcp_mode_with_cursor_explore_registration_is_fail() {
        let dir = tmp("drift_cursor_explore");
        std::fs::create_dir_all(dir.join(".grove")).unwrap();
        std::fs::write(
            dir.join(".grove").join("config.json"),
            r#"{"version":1,"mode":"mcp","harnesses":["claude-code","cursor"]}"#,
        )
        .unwrap();
        write_mcp_json(dir.as_path(), &["serve"]);
        write_claude_md(&dir, "mcp__grove__outline");
        // Cursor registered for explore even though mode is mcp.
        write_cursor_mcp_json(dir.as_path(), &["serve"], Some(&[]));

        let report = diagnose(&dir, ModeChoice::None);
        let cursor = report.checks.iter().find(|c| c.name == "harness_mcp_cursor").unwrap();
        assert!(
            matches!(cursor.status, Status::Fail),
            "unexpected grove-explore in cursor registration for mcp mode → Fail: {}",
            cursor.detail
        );
    }

    #[test]
    fn mcp_llm_cursor_only_explore_registration_triggers_explore_group() {
        let dir = tmp("cursor_only_explore");
        std::fs::create_dir_all(dir.join(".grove")).unwrap();
        std::fs::write(
            dir.join(".grove").join("config.json"),
            r#"{"version":1,"mode":"mcp-llm","harnesses":["cursor"],"explore":{"provider":"ollama","base_url":"http://localhost:11434/v1","model":"x","steering":"standard","allowed_tools":[]}}"#,
        )
        .unwrap();
        // No Claude .mcp.json; only Cursor carries the explore registration.
        // Structural args are None: McpLlm registers grove-explore alone.
        write_cursor_mcp_json(dir.as_path(), &[], Some(&[]));
        write_agents_md(&dir);

        let report = diagnose(&dir, ModeChoice::None);
        let cursor = report.checks.iter().find(|c| c.name == "harness_mcp_cursor").unwrap();
        assert!(matches!(cursor.status, Status::Ok), "cursor OK: {}", cursor.detail);
        assert!(
            report.checks.iter().any(|c| c.group == "explore"),
            "explore group must run when grove-explore is registered in a selected harness"
        );
        let surface = report
            .checks
            .iter()
            .find(|c| c.name == "harness_serve_surface")
            .unwrap();
        assert!(
            surface.detail.starts_with("Explore"),
            "serve surface should report Explore: {}",
            surface.detail
        );
    }

    #[test]
    fn mcp_llm_without_explore_registration_skips_explore_group() {
        let dir = tmp("no_explore_reg");
        write_config(&dir, "mcp-llm");
        // Structural-only registration: no grove-explore anywhere.
        write_mcp_json(dir.as_path(), &["serve"]);
        write_claude_md(&dir, "mcp__grove-explore__explore");
        write_agents_md(&dir);

        let report = diagnose(&dir, ModeChoice::None);
        assert!(
            !report.checks.iter().any(|c| c.group == "explore"),
            "explore group must be skipped when no grove-explore registration is present"
        );
        let surface = report
            .checks
            .iter()
            .find(|c| c.name == "harness_serve_surface")
            .unwrap();
        assert!(
            surface.detail.starts_with("Standard"),
            "serve surface should report Standard without explore registration: {}",
            surface.detail
        );
    }

    #[test]
    fn mcp_llm_missing_explore_entry_is_fail() {
        let dir = tmp("missing_explore_entry");
        write_config(&dir, "mcp-llm");
        // Structural grove entry present but grove-explore missing.
        write_mcp_json(dir.as_path(), &["serve"]);
        write_claude_md(&dir, "mcp__grove-explore__explore");
        write_agents_md(&dir);

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "harness_mcp_json")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Fail),
            "expected Fail for mcp-llm without grove-explore entry, got {:?}: {}",
            chk.status,
            chk.detail
        );
        assert!(
            chk.hint.as_deref() == Some("grove init --as mcp-llm"),
            "hint should mention mcp-llm: {:?}",
            chk.hint
        );
    }

    #[test]
    fn mcp_mode_with_explore_marker_in_claude_md_is_fail() {
        let dir = tmp("drift_claude_md");
        write_config(&dir, "mcp");
        write_mcp_json(dir.as_path(), &["serve"]);
        write_claude_md(&dir, "mcp__grove__explore"); // wrong marker for mcp mode

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "harness_claude_md")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Fail),
            "expected Fail for mcp mode with explore marker, got {:?}: {}",
            chk.status,
            chk.detail
        );
    }

    #[test]
    fn mcp_llm_mode_without_agents_md_is_warn() {
        let dir = tmp("drift_agents_md");
        write_config(&dir, "mcp-llm");
        // Dual-server layout: grove with ["serve"], grove-explore with [].
        write_explore_mcp_json(dir.as_path());
        write_claude_md(&dir, "mcp__grove-explore__explore");
        // AGENTS.md intentionally absent

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "harness_agents_md")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Warn),
            "expected Warn for mcp-llm without AGENTS.md, got {:?}: {}",
            chk.status,
            chk.detail
        );
    }

    #[test]
    fn grammars_mode_with_grove_block_in_claude_md_is_warn() {
        let dir = tmp("drift_grammars");
        write_config(&dir, "grammars");
        // Write a CLAUDE.md with a grove block — should not be present for grammars mode
        write_claude_md(&dir, "mcp__grove__outline");

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "harness_claude_md")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Warn),
            "expected Warn for grammars with grove block in CLAUDE.md, got {:?}: {}",
            chk.status,
            chk.detail
        );
    }

    // ── lock integrity ────────────────────────────────────────────────────────

    #[test]
    fn lock_integrity_absent_lockfile_is_warn() {
        let dir = tmp("lock_absent");
        write_config(&dir, "mcp");
        seed_harness_for_mode(&dir, Mode::Mcp);
        // No grove.lock written

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "lock_integrity")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Warn),
            "expected Warn for absent grove.lock, got {:?}: {}",
            chk.status,
            chk.detail
        );
    }

    // ── explore-mode checks ────────────────────────────────────────────────────

    #[test]
    fn explore_config_absent_is_fail() {
        let dir = tmp("explore_absent");
        // mcp-llm mode but no explore section in config.
        // Use the dual-server layout that init now produces.
        write_config(&dir, "mcp-llm");
        write_explore_mcp_json(&dir);
        write_claude_md(&dir, "mcp__grove-explore__explore");
        write_agents_md(&dir);

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "explore_config_valid")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Fail),
            "expected Fail for absent explore config, got {:?}: {}",
            chk.status,
            chk.detail
        );
    }

    // provider_reachable / model_served checks were removed from grove doctor
    // (health_probe lives in grove-explore-core which grove-cst cannot depend on;
    // the probe still fires at `grove serve` startup). No test needed here.
}
