//! Local inference-engine discovery for the `grove config` picker.
//!
//! Two sources are merged: a table of well-known default ports
//! ([`ENGINE_CANDIDATES`]) and — on Linux — **running-process detection**, which
//! scans `/proc` for a `llama-server` / `llama serve` / `ollama` / `vllm` / LM
//! Studio process and probes the port it is *actually* bound to. Both are probed
//! via the shared `{base_url}/models` GET, so an engine on a non-default port is
//! found where a fixed-port probe would miss it.

use std::time::Duration;

#[cfg(target_os = "linux")]
use std::path::Path;

use super::health::fetch_models_at;

/// Connect deadline for a discovery probe — short, so the common case (nothing
/// listening on that port → connection refused) returns effectively instantly
/// and a filtered/hung port is capped rather than stalling the config TUI.
const DISCOVER_CONNECT_TIMEOUT: Duration = Duration::from_millis(600);
/// Overall discovery-probe deadline. Long enough for a live local server to
/// answer `/models` (instant on loopback), short enough to stay snappy.
const DISCOVER_PROBE_TIMEOUT: Duration = Duration::from_millis(1200);

/// A well-known local inference server the config TUI probes for auto-detection.
/// Both faces speak the same OpenAI-compatible wire protocol, so the `label` is
/// only a human hint; `base_url` is the server's conventional local default.
#[derive(Debug, Clone)]
pub struct EngineCandidate {
    /// Human label (e.g. `"ollama"`, `"llama.cpp"`).
    pub label: &'static str,
    /// Conventional local OpenAI-compatible base URL for this server.
    pub base_url: &'static str,
}

/// The built-in probe table: the local inference servers grove auto-detects, in
/// display order. Every entry exposes the standard `{base_url}/models` listing.
pub const ENGINE_CANDIDATES: &[EngineCandidate] = &[
    EngineCandidate { label: "ollama", base_url: "http://localhost:11434/v1" },
    EngineCandidate { label: "llama.cpp", base_url: "http://localhost:8080/v1" },
    EngineCandidate { label: "lm-studio", base_url: "http://localhost:1234/v1" },
    EngineCandidate { label: "vllm", base_url: "http://localhost:8000/v1" },
];

/// A probed local inference endpoint: whether it answered `/models`, and the
/// models it serves (empty when it answered with none, or wasn't reachable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredEngine {
    /// Human label carried from the [`EngineCandidate`].
    pub label: String,
    /// The OpenAI-compatible base URL probed.
    pub base_url: String,
    /// `true` when `{base_url}/models` answered.
    pub alive: bool,
    /// The served model ids (empty unless `alive` and the server reported some).
    pub models: Vec<String>,
}

/// Probe local inference endpoints **concurrently** (short deadline) and report
/// which answered. Two sources are merged:
///
/// 1. The [`ENGINE_CANDIDATES`] default ports — always present, always first, so
///    the result begins with the stable `{ollama, llama.cpp, lm-studio, vllm}`
///    rows the config TUI renders.
/// 2. **Running-process detection** ([`detect_engine_ports`]) — a `llama-server`
///    / `ollama` / `vllm` / LM Studio process bound to a *non-default* port
///    contributes an extra endpoint, so an engine on e.g. `:8081` is found where
///    a fixed-port probe would miss it. (Linux only; a no-op elsewhere.)
///
/// Endpoints are deduped by URL (defaults win order). Never blocks beyond one
/// probe timeout: dead ports (connection refused) return instantly and the
/// slowest live probe bounds the wall time.
pub fn discover_engines() -> Vec<DiscoveredEngine> {
    let mut candidates: Vec<(String, String)> = ENGINE_CANDIDATES
        .iter()
        .map(|c| (c.label.to_string(), c.base_url.to_string()))
        .collect();
    for (label, port) in detect_engine_ports() {
        candidates.push((label.to_string(), format!("http://localhost:{port}/v1")));
    }
    // Dedup by base_url, preserving order (the default candidates come first, so
    // a detected endpoint on a default port collapses into its default row).
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|(_, url)| seen.insert(url.clone()));

    let handles: Vec<_> = candidates
        .into_iter()
        .map(|(label, base_url)| {
            std::thread::spawn(move || {
                // `None`: local engine auto-detection probes fixed `localhost`
                // ports only and must never be routed through a proxy, even if
                // one is configured in the environment.
                match fetch_models_at(&base_url, DISCOVER_CONNECT_TIMEOUT, DISCOVER_PROBE_TIMEOUT, None)
                {
                    Ok(models) => DiscoveredEngine { label, base_url, alive: true, models },
                    Err(_) => DiscoveredEngine { label, base_url, alive: false, models: Vec::new() },
                }
            })
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// The default port an engine binds when its command line advertises none.
#[cfg(target_os = "linux")]
fn default_port(label: &str) -> u16 {
    match label {
        "ollama" => 11434,
        "llama.cpp" => 8080,
        "lm-studio" => 1234,
        "vllm" => 8000,
        _ => 0,
    }
}

/// Identify a local inference server from its argv, returning the engine label.
///
/// Matching is **argv-aware**, not a flat substring scan, because `llama` and
/// `ollama` would otherwise cross-match: the router binary runs `llama serve`
/// and ollama runs `ollama serve`, and `"ollama serve"` contains `"llama
/// serve"`. So the executables are matched on their basename, and the newer
/// unified `llama serve` router is disambiguated by its `serve` subcommand.
#[cfg(target_os = "linux")]
fn match_engine(args: &[String]) -> Option<&'static str> {
    let exe = args
        .first()
        .map(|a| a.rsplit('/').next().unwrap_or(a).to_ascii_lowercase())
        .unwrap_or_default();
    if exe == "ollama" {
        // The `runner` subcommand is ollama's internal per-model worker; its
        // `--port` is an ephemeral internal API, not the OpenAI surface — a
        // false detection would add a dead row per loaded model.
        if args.get(1).is_some_and(|a| a == "runner") {
            return None;
        }
        return Some("ollama");
    }
    // `llama-server` (classic) or the unified `llama serve` router.
    if exe == "llama-server" {
        return Some("llama.cpp");
    }
    if exe == "llama" && args.iter().any(|a| a == "serve") {
        return Some("llama.cpp");
    }
    // A shell is never the inference server itself (servers exec their binary),
    // but a shell's `-c` string routinely QUOTES engine names — grep/log/build
    // commands about vllm would otherwise match the substring scan below.
    if matches!(exe.as_str(), "bash" | "sh" | "zsh" | "fish" | "dash" | "ksh") {
        return None;
    }
    // vLLM / LM Studio run under an interpreter or helper, so match anywhere in
    // the command line — the tokens are distinctive enough not to cross-match.
    let joined = args.join(" ").to_ascii_lowercase();
    if joined.contains("vllm") {
        return Some("vllm");
    }
    if joined.contains("lm-studio") || joined.contains("lmstudio") {
        return Some("lm-studio");
    }
    None
}

/// Extract an explicit `--port N` / `--port=N` from a process's argv (the flag
/// `llama-server`, `llama serve`, `vllm serve`, and friends use to bind a
/// non-default port). Port `0` (auto-assign, used by router-spawned workers) is
/// treated as "unspecified" so it falls back to the default rather than probing
/// `:0`.
#[cfg(target_os = "linux")]
fn port_from_argv(args: &[String]) -> Option<u16> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let parsed = if let Some(rest) = a.strip_prefix("--port=") {
            rest.parse().ok()
        } else if a == "--port" {
            it.next().and_then(|v| v.parse().ok())
        } else {
            None
        };
        if let Some(p) = parsed {
            return if p == 0 { None } else { Some(p) };
        }
    }
    None
}

/// Detect running local inference servers and the ports they're bound to, by
/// scanning `/proc/<pid>/cmdline`. Port precedence: an explicit `--port`, then
/// (for ollama) `OLLAMA_HOST` from the process environment, then the engine's
/// default. Best-effort — unreadable entries (other users' processes) are
/// skipped.
///
/// **Worker suppression:** a matched process whose *parent* matches the same
/// engine is an internal worker (the `llama serve` router re-spawns itself on
/// an ephemeral port; vLLM forks workers). Only the tree root — the stable,
/// user-facing endpoint — is reported; saving a worker's ephemeral port would
/// break on the next model reload. Returns `(label, port)` pairs, deduped.
#[cfg(target_os = "linux")]
fn detect_engine_ports() -> Vec<(&'static str, u16)> {
    struct Hit {
        pid: u32,
        ppid: Option<u32>,
        label: &'static str,
        port: u16,
    }
    let mut hits: Vec<Hit> = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        // Only numeric /proc/<pid> directories.
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(raw) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        // cmdline is NUL-separated argv.
        let args: Vec<String> = raw
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();
        let Some(label) = match_engine(&args) else {
            continue;
        };
        let port = port_from_argv(&args)
            .or_else(|| (label == "ollama").then(|| ollama_host_port(&entry.path())).flatten())
            .unwrap_or_else(|| default_port(label));
        hits.push(Hit { pid, ppid: ppid_of(&entry.path()), label, port });
    }

    // Drop workers: any hit whose parent is itself a hit with the same label.
    let roots: std::collections::HashMap<u32, &'static str> =
        hits.iter().map(|h| (h.pid, h.label)).collect();
    let mut found: Vec<(&'static str, u16)> = hits
        .iter()
        .filter(|h| h.ppid.and_then(|p| roots.get(&p)) != Some(&h.label))
        .map(|h| (h.label, h.port))
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

/// The parent pid from `/proc/<pid>/stat` (field 4). The comm field (2) may
/// contain spaces or parens, so parsing starts after the *last* `)`.
#[cfg(target_os = "linux")]
fn ppid_of(proc_dir: &Path) -> Option<u32> {
    let stat = std::fs::read_to_string(proc_dir.join("stat")).ok()?;
    let rest = stat.rsplit_once(')')?.1;
    // After the comm: state, ppid, …
    rest.split_whitespace().nth(1)?.parse().ok()
}

/// Read `OLLAMA_HOST` from a process's environment and return its port. Handles
/// `host:port`, `:port`, and `http://host:port` spellings.
#[cfg(target_os = "linux")]
fn ollama_host_port(proc_dir: &Path) -> Option<u16> {
    let raw = std::fs::read(proc_dir.join("environ")).ok()?;
    raw.split(|&b| b == 0).find_map(|kv| {
        let kv = String::from_utf8_lossy(kv);
        let val = kv.strip_prefix("OLLAMA_HOST=")?;
        val.rsplit(':').next()?.parse().ok()
    })
}

/// Process detection is Linux-only (reads `/proc`); elsewhere discovery falls
/// back to the default-port candidates.
#[cfg(not(target_os = "linux"))]
fn detect_engine_ports() -> Vec<(&'static str, u16)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_engines_starts_with_every_default_candidate_in_order() {
        // The result always begins with the default-port candidates, in order —
        // that stable prefix is what the config TUI renders. Process detection
        // may append extra endpoints (an engine on a non-default port), so the
        // list can be longer, but never shorter or reordered.
        let found = discover_engines();
        assert!(found.len() >= ENGINE_CANDIDATES.len());
        for (got, want) in found.iter().zip(ENGINE_CANDIDATES) {
            assert_eq!(got.label, want.label);
            assert_eq!(got.base_url, want.base_url);
        }
        // A dead endpoint never reports models.
        for e in &found {
            if !e.alive {
                assert!(e.models.is_empty(), "dead engine must list no models");
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn match_engine_identifies_servers_without_cross_matching() {
        let argv = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        // Classic llama-server binary.
        assert_eq!(match_engine(&argv("/usr/bin/llama-server --port 8081 --model x.gguf")), Some("llama.cpp"));
        // The unified `llama serve` router — the real-world 8081 case. Its argv
        // starts with the `llama` binary and a `serve` subcommand.
        assert_eq!(match_engine(&argv("llama serve --alias grove --port 8081")), Some("llama.cpp"));
        // Critical: `ollama serve` must NOT read as llama.cpp even though
        // "ollama serve" contains "llama serve".
        assert_eq!(match_engine(&argv("/usr/local/bin/ollama serve")), Some("ollama"));
        // A bare `llama` with no `serve` subcommand is not the router.
        assert_eq!(match_engine(&argv("llama --help")), None);
        assert_eq!(match_engine(&argv("python -m vllm.entrypoints.openai.api_server")), Some("vllm"));
        // A plain editor session is not an engine.
        assert_eq!(match_engine(&argv("vim src/main.rs")), None);
        // ollama's internal per-model worker is NOT the OpenAI surface — its
        // --port is an ephemeral internal API; matching it would add a dead row.
        assert_eq!(
            match_engine(&argv("/usr/local/bin/ollama runner --model m.gguf --port 34371")),
            None
        );
        // A shell whose -c string merely QUOTES an engine name (grep/log/build
        // commands about vllm) must not match — observed live: the shell running
        // a discovery probe matched its own command text.
        assert_eq!(
            match_engine(&["/bin/bash".into(), "-c".into(), "grep vllm notes.md --port 9000".into()]),
            None
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn port_from_argv_reads_both_spellings_and_ignores_zero() {
        let split = ["llama-server".into(), "--port".into(), "8081".into()];
        assert_eq!(port_from_argv(&split), Some(8081));
        let eq = ["llama-server".into(), "--port=9000".into()];
        assert_eq!(port_from_argv(&eq), Some(9000));
        let none: [String; 1] = ["llama-server".into()];
        assert_eq!(port_from_argv(&none), None);
        // `--port 0` (router-spawned worker, auto-assign) → treated as unset.
        let zero = ["llama".into(), "serve".into(), "--port".into(), "0".into()];
        assert_eq!(port_from_argv(&zero), None);
    }
}
