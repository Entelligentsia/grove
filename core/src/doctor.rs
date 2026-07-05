//! `core::doctor` — project health diagnostics.
//!
//! The public entry point is [`diagnose`], which returns a [`Report`]
//! describing the health of a grove project at `root`.  The function is
//! **pure and read-only**: it never writes to disk and performs at most one
//! outbound network request (the explore-mode health probe).

use std::path::Path;

use crate::{
    config::{active_mode, GroveConfig, Mode, ModeChoice},
    explore::{health_probe, ExploreConfig, HealthError},
    harness,
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
    // Determine whether the explore section is present for harness_serve_surface.
    let has_explore_cfg = cfg_result
        .as_ref()
        .ok()
        .and_then(|c| c.explore.as_ref())
        .is_some();

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

    // Resolve the effective mode for harness checks (respects --explore/--standard).
    let mode = active_mode(root, force);

    // ── harness sub-checks ──────────────────────────────────────────────────
    checks.push(check_harness_mcp_json(root, mode));
    checks.push(check_harness_claude_md(root, mode));
    checks.push(check_harness_agents_md(root, mode));
    checks.push(check_harness_serve_surface(mode, has_explore_cfg));

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

    // ── Explore-mode checks (McpLlm only) ───────────────────────────────────
    if mode == Mode::McpLlm {
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
    let expected_args = harness::expected_mcp_args(mode);

    let actual_args: Option<Vec<String>> = (|| -> Option<Vec<String>> {
        let text = std::fs::read_to_string(&path).ok()?;
        let doc: serde_json::Value = serde_json::from_str(&text).ok()?;
        let args = doc["mcpServers"][harness::MCP_SERVER_KEY]["args"].as_array()?;
        Some(
            args.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        )
    })();

    match (expected_args, actual_args) {
        // Mode expects no grove entry and none is present
        (None, None) => Check {
            group: "universal",
            name: "harness_mcp_json",
            status: Status::Ok,
            detail: format!(
                "mode={}: no grove entry expected and none present",
                mode_name(mode)
            ),
            hint: None,
        },
        // Mode expects no grove entry but one is present
        (None, Some(_)) => Check {
            group: "universal",
            name: "harness_mcp_json",
            status: Status::Fail,
            detail: format!(
                "mode={}: .mcp.json has a grove entry but none is expected",
                mode_name(mode)
            ),
            hint: Some(format!("grove init --as {}", mode_name(mode))),
        },
        // Mode expects a grove entry but .mcp.json is absent or has no grove entry
        (Some(expected), None) => Check {
            group: "universal",
            name: "harness_mcp_json",
            status: Status::Fail,
            detail: format!(
                "mode={}: .mcp.json absent or no grove entry; expected args {:?}",
                mode_name(mode),
                expected
            ),
            hint: Some(format!("grove init --as {}", mode_name(mode))),
        },
        // Mode expects a grove entry and one is present — compare args
        (Some(expected), Some(actual)) => {
            let expected_strs: Vec<&str> = expected.to_vec();
            let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
            if expected_strs == actual_refs {
                Check {
                    group: "universal",
                    name: "harness_mcp_json",
                    status: Status::Ok,
                    detail: format!("mode={}: args {:?}", mode_name(mode), actual),
                    hint: None,
                }
            } else {
                Check {
                    group: "universal",
                    name: "harness_mcp_json",
                    status: Status::Fail,
                    detail: format!(
                        "mode={}: expected args {:?}, found {:?}",
                        mode_name(mode),
                        expected_strs,
                        actual
                    ),
                    hint: Some(format!("grove init --as {}", mode_name(mode))),
                }
            }
        }
    }
}

fn check_harness_claude_md(root: &Path, mode: Mode) -> Check {
    let path = root.join("CLAUDE.md");
    let expected_marker = harness::expected_claude_marker(mode);
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

fn check_harness_serve_surface(mode: Mode, has_explore_cfg: bool) -> Check {
    let surface = if mode == Mode::McpLlm {
        if has_explore_cfg {
            "Explore (when provider is healthy)"
        } else {
            "Standard (explore config missing; would fall back)"
        }
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

fn explore_checks(cfg: Option<&ExploreConfig>) -> Vec<Check> {
    let mut checks = Vec::new();

    // ── explore_config_valid ─────────────────────────────────────────────────
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
        Some(c) => match c.validate() {
            Ok(()) => {
                checks.push(Check {
                    group: "explore",
                    name: "explore_config_valid",
                    status: Status::Ok,
                    detail: format!("base_url={} model={}", c.base_url, c.model),
                    hint: None,
                });
                c
            }
            Err(e) => {
                checks.push(Check {
                    group: "explore",
                    name: "explore_config_valid",
                    status: Status::Fail,
                    detail: format!("invalid explore config: {e}"),
                    hint: Some("grove config".to_string()),
                });
                return checks;
            }
        },
    };

    // ── provider_reachable / model_served ────────────────────────────────────
    match health_probe(cfg) {
        Ok(()) => {
            checks.push(Check {
                group: "explore",
                name: "provider_reachable",
                status: Status::Ok,
                detail: format!("reachable: {}", cfg.base_url),
                hint: None,
            });
            checks.push(Check {
                group: "explore",
                name: "model_served",
                status: Status::Ok,
                detail: format!("model {} is served", cfg.model),
                hint: None,
            });
        }
        Err(HealthError::Unreachable { url, detail }) => {
            checks.push(Check {
                group: "explore",
                name: "provider_reachable",
                status: Status::Fail,
                detail: format!("unreachable: {url} ({detail})"),
                hint: Some("start your local model provider and retry".to_string()),
            });
            checks.push(Check {
                group: "explore",
                name: "model_served",
                status: Status::Info,
                detail: "skipped — provider unreachable".to_string(),
                hint: None,
            });
        }
        Err(HealthError::ModelMissing {
            model,
            url,
            available,
        }) => {
            checks.push(Check {
                group: "explore",
                name: "provider_reachable",
                status: Status::Ok,
                detail: format!("reachable: {url}"),
                hint: None,
            });
            let avail_str = if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            };
            checks.push(Check {
                group: "explore",
                name: "model_served",
                status: Status::Fail,
                detail: format!("model {model} not found at {url}; available: {avail_str}"),
                hint: Some(format!(
                    "pull {model} or set a different model in `grove config`"
                )),
            });
        }
    }

    // ── allowed_tools_known (Ok / Warn) ──────────────────────────────────────
    {
        use crate::explore::toolset;
        let known = [toolset::READ, toolset::GLOB, toolset::GREP, toolset::GROVE];
        let unknown: Vec<_> = cfg
            .allowed_tools
            .iter()
            .filter(|t| !known.contains(&t.as_str()))
            .cloned()
            .collect();
        if unknown.is_empty() {
            checks.push(Check {
                group: "explore",
                name: "allowed_tools_known",
                status: Status::Ok,
                detail: format!("tools: {}", cfg.allowed_tools.join(", ")),
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
    checks.push(Check {
        group: "explore",
        name: "tap_config",
        status: Status::Info,
        detail: format!("tap={} trace_retain={}", cfg.tap, cfg.trace_retain),
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
            }
            Mode::McpLlm => {
                write_mcp_json(dir, &["serve", "--explore"]);
                write_claude_md(dir, "mcp__grove__explore");
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
    fn mcp_mode_with_explore_args_in_mcp_json_is_fail() {
        let dir = tmp("drift_mcp_json");
        write_config(&dir, "mcp");
        write_mcp_json(dir.as_path(), &["serve", "--explore"]); // wrong for mcp mode
        write_claude_md(&dir, "mcp__grove__outline");

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "harness_mcp_json")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Fail),
            "expected Fail for mcp mode with explore args, got {:?}: {}",
            chk.status,
            chk.detail
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
        write_mcp_json(dir.as_path(), &["serve", "--explore"]);
        write_claude_md(&dir, "mcp__grove__explore");
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
        // mcp-llm mode but no explore section in config
        write_config(&dir, "mcp-llm");
        write_mcp_json(&dir, &["serve", "--explore"]);
        write_claude_md(&dir, "mcp__grove__explore");
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

    #[test]
    fn provider_unreachable_is_fail() {
        let dir = tmp("explore_unreachable");
        // Write a full mcp-llm config with a dead base_url
        let grove = dir.join(".grove");
        fs::create_dir_all(&grove).unwrap();
        let cfg_json = r#"{
            "version": 1,
            "mode": "mcp-llm",
            "explore": {
                "provider": "ollama",
                "base_url": "http://127.0.0.1:19999/v1",
                "model": "nonexistent",
                "steering": "standard",
                "allowed_tools": ["grove"],
                "tap": false
            }
        }"#;
        fs::write(grove.join("config.json"), cfg_json).unwrap();
        write_mcp_json(&dir, &["serve", "--explore"]);
        write_claude_md(&dir, "mcp__grove__explore");
        write_agents_md(&dir);

        let report = diagnose(&dir, ModeChoice::None);
        let chk = report
            .checks
            .iter()
            .find(|c| c.name == "provider_reachable")
            .unwrap();
        assert!(
            matches!(chk.status, Status::Fail),
            "expected Fail for unreachable provider, got {:?}: {}",
            chk.status,
            chk.detail
        );
    }
}
