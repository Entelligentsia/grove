//! End-to-end tests for the `grove` CLI (the `main.rs` dispatch + output paths).
//!
//! Each test runs the built binary against the 3-language dev stub in `registry/`
//! (pinned via `GROVE_REGISTRY` so results are deterministic regardless of any OS
//! cache) and a throwaway fixture project.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The dev-stub registry shipped in the source tree (rust, python, javascript).
const DEV_REGISTRY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../registry");

fn fixture(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grove_cli_test_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("lib.rs"),
        "fn helper() {}\nfn caller() {\n    helper();\n}\nstruct Thing;\nimpl Thing {\n    fn method(&self) {}\n}\n",
    )
    .unwrap();
    dir
}

fn grove(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_grove"))
        .args(args)
        .current_dir(cwd)
        .env("GROVE_REGISTRY", DEV_REGISTRY)
        .output()
        .expect("running grove")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn outline_lists_definitions_human_and_json() {
    let dir = fixture("outline");

    let out = grove(&dir, &["outline", "lib.rs"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("helper"), "human outline names defs: {text}");
    assert!(text.contains("method"));

    let j = grove(&dir, &["--json", "outline", "lib.rs"]);
    assert!(j.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&j)).expect("outline --json is JSON");
    let names: Vec<&str> = v.as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"helper") && names.contains(&"method"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn outline_kind_filter_narrows_results() {
    let dir = fixture("outline_kind");
    let j = grove(&dir, &["--json", "outline", "lib.rs", "--kind", "function"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&j)).unwrap();
    assert!(v.as_array().unwrap().iter().all(|s| s["kind"] == "function"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn symbols_finds_across_dir() {
    let dir = fixture("symbols");
    let j = grove(&dir, &["--json", "symbols", ".", "--name", "helper"]);
    assert!(j.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&j)).unwrap();
    assert!(v.as_array().unwrap().iter().any(|s| s["name"] == "helper"));

    // Human table path too.
    let human = grove(&dir, &["symbols", ".", "--name", "helper"]);
    assert!(human.status.success());
    assert!(stdout(&human).contains("helper"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn source_prints_a_symbol_body() {
    let dir = fixture("source");
    let out = grove(&dir, &["source", "lib.rs", "helper"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("fn helper"));

    // Duplicate-named defs surface the other candidates on stderr.
    std::fs::write(dir.join("dup.rs"), "fn dup() { let _a = 1; }\nfn dup() { let _b = 2; }\n").unwrap();
    let d = grove(&dir, &["source", "dup.rs", "dup"]);
    assert!(d.status.success());
    assert!(String::from_utf8_lossy(&d.stderr).contains("also matched"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn check_ok_succeeds_and_broken_exits_nonzero() {
    let dir = fixture("check");

    let ok = grove(&dir, &["check", "lib.rs"]);
    assert!(ok.status.success());
    assert!(stdout(&ok).contains("ok"), "clean file reports ok");

    std::fs::write(dir.join("broken.rs"), "fn main( {\n").unwrap();
    let bad = grove(&dir, &["check", "broken.rs"]);
    assert!(!bad.status.success(), "broken file exits non-zero");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn callers_finds_call_sites() {
    let dir = fixture("callers");
    let out = grove(&dir, &["callers", "helper", "-d", "."]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("caller"), "enclosing fn shown: {}", stdout(&out));
    // Each row leads with the file path so directory-wide queries are usable
    // without a grep fallback (#29). The call sits on line 3 (1-based) of the fixture.
    assert!(stdout(&out).contains("lib.rs:3:"), "file path shown: {}", stdout(&out));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn definition_by_name_and_by_position() {
    let dir = fixture("definition");

    let by_name = grove(&dir, &["--json", "definition", "helper", "-d", "."]);
    assert!(by_name.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&by_name)).unwrap();
    assert!(v.as_array().unwrap().iter().any(|s| s["name"] == "helper"));

    // Human table path.
    let human = grove(&dir, &["definition", "helper", "-d", "."]);
    assert!(human.status.success());
    assert!(stdout(&human).contains("helper"));

    // Resolve the `helper()` call on line 3, col 5 (1-based) back to its definition.
    let by_at = grove(&dir, &["--json", "definition", "-d", ".", "--at", "lib.rs:3:5"]);
    assert!(by_at.status.success(), "stderr: {}", String::from_utf8_lossy(&by_at.stderr));
    let v2: serde_json::Value = serde_json::from_str(&stdout(&by_at)).unwrap();
    assert!(v2.as_array().unwrap().iter().any(|s| s["name"] == "helper"));

    std::fs::remove_dir_all(&dir).ok();
}

/// ADR 0001 Step 1: `definition --at` is scope-aware — a use of a name that has a
/// local binding resolves to that binding, even when a same-named global exists
/// (shadowing), and a use of a free/global name still resolves directory-wide.
#[test]
fn definition_at_is_scope_aware() {
    let dir = std::env::temp_dir().join(format!("grove_cli_test_{}_scope", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A local `run` (line 3) shadows the module-level `fn run` (line 1).
    std::fs::write(
        dir.join("lib.rs"),
        "fn run() {}\nfn caller() {\n    let run = 1;\n    let _x = run;\n}\n",
    )
    .unwrap();

    // Cursor on the `run` use in `let _x = run;` (line 4, col 14, 1-based).
    let local = grove(&dir, &["--json", "definition", "-d", ".", "--at", "lib.rs:4:14"]);
    assert!(local.status.success(), "stderr: {}", String::from_utf8_lossy(&local.stderr));
    let v: serde_json::Value = serde_json::from_str(&stdout(&local)).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1, "scope-aware resolution returns the single binding: {v}");
    assert_eq!(arr[0]["name"], "run");
    assert_eq!(arr[0]["kind"], "local", "must be the local binding, not the global fn");
    assert_eq!(arr[0]["line"], 3, "resolves to `let run` on line 3, not `fn run` on line 1");

    // Cursor on the `run` *definition* in `fn run()` (line 1, col 4): no enclosing
    // local binding, so it falls back to the directory-wide name lookup and finds
    // the global function definition.
    let global = grove(&dir, &["--json", "definition", "-d", ".", "--at", "lib.rs:1:4"]);
    assert!(global.status.success(), "stderr: {}", String::from_utf8_lossy(&global.stderr));
    let vg: serde_json::Value = serde_json::from_str(&stdout(&global)).unwrap();
    assert!(
        vg.as_array().unwrap().iter().any(|s| s["name"] == "run" && s["kind"] == "function"),
        "free/global name falls back to the global definition: {vg}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// An optional query (`locals.scm`/`imports.scm`) that fails to compile — e.g. an
/// upstream file authored against a different grammar version — must never break
/// the core tools; the feature just stays off (ADR 0001, hosting robustness).
#[test]
fn invalid_locals_query_is_non_fatal() {
    let reg = std::env::temp_dir().join(format!("grove_cli_test_{}_badlocals_reg", std::process::id()));
    let rust = reg.join("rust");
    std::fs::create_dir_all(&rust).unwrap();
    let src_reg = Path::new(DEV_REGISTRY).join("rust");
    for f in ["grammar.wasm", "tags.scm", "manifest.json"] {
        std::fs::copy(src_reg.join(f), rust.join(f)).unwrap();
    }
    // References a node kind rust's grammar doesn't have → query compile error.
    std::fs::write(rust.join("locals.scm"), "(this_is_not_a_real_node) @local.scope\n").unwrap();

    let dir = std::env::temp_dir().join(format!("grove_cli_test_{}_badlocals", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lib.rs"), "fn helper() {}\n").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_grove"))
        .args(["outline", "lib.rs"])
        .current_dir(&dir)
        .env("GROVE_REGISTRY", &reg)
        .output()
        .expect("run grove");
    assert!(
        out.status.success(),
        "core tools must survive a bad locals query: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout(&out).contains("helper"), "outline still works with a broken locals.scm");

    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&reg).ok();
}

/// ADR 0001 Step 2: `definition --at` follows an import edge to the *target file*
/// — cross-file go-to-def — for Python's `from pkg.mod import name`.
#[test]
fn definition_at_resolves_across_files_python() {
    let dir = std::env::temp_dir().join(format!("grove_cli_test_{}_imp_py", std::process::id()));
    let pkg = dir.join("pkg");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(pkg.join("util.py"), "def helper():\n    return 1\n").unwrap();
    std::fs::write(
        dir.join("main.py"),
        "from pkg.util import helper\ndef run():\n    return helper()\n",
    )
    .unwrap();

    // The `helper()` call on line 3, col 12 (1-based).
    let out = grove(&dir, &["--json", "definition", "-d", ".", "--at", "main.py:3:12"]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1, "resolves to the single target-file def: {v}");
    assert_eq!(arr[0]["name"], "helper");
    assert!(
        arr[0]["file"].as_str().unwrap().ends_with("util.py"),
        "resolved into the imported file pkg/util.py, not a dir-wide list: {v}"
    );
    assert_eq!(arr[0]["line"], 1);

    std::fs::remove_dir_all(&dir).ok();
}

/// ADR 0001 Step 2: the JavaScript relative-path strategy resolves an aliased
/// named import (`import { compute as c } from "./calc"`) to the original symbol
/// in the target module.
#[test]
fn definition_at_resolves_across_files_javascript_aliased() {
    let dir = std::env::temp_dir().join(format!("grove_cli_test_{}_imp_js", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("calc.js"), "export function compute(a) {\n  return a * 2;\n}\n").unwrap();
    std::fs::write(
        dir.join("app.js"),
        "import { compute as c } from \"./calc\";\nfunction run() {\n  return c(3);\n}\n",
    )
    .unwrap();

    // The `c(3)` call on line 3, col 10 (1-based).
    let out = grove(&dir, &["--json", "definition", "-d", ".", "--at", "app.js:3:10"]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1, "resolves to the single target-file def: {v}");
    assert_eq!(arr[0]["name"], "compute", "alias resolves to the original symbol name");
    assert!(arr[0]["file"].as_str().unwrap().ends_with("calc.js"), "resolved into calc.js: {v}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn languages_lists_the_dev_stub() {
    let dir = fixture("languages");
    let out = grove(&dir, &["languages"]);
    assert!(out.status.success());
    let text = stdout(&out);
    for lang in ["rust", "python", "javascript"] {
        assert!(text.contains(lang), "languages should list {lang}: {text}");
    }

    let j = grove(&dir, &["--json", "languages"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&j)).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 3, "dev stub has 3 languages");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn registry_shows_resolved_root() {
    let dir = fixture("registry");
    let out = grove(&dir, &["registry"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("registry root:"));
    assert!(text.contains("search order"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn lock_writes_grove_lock() {
    let dir = fixture("lock");
    let out = grove(&dir, &["lock"]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let lock = std::fs::read_to_string(dir.join("grove.lock")).expect("grove.lock written");
    let doc: serde_json::Value = serde_json::from_str(&lock).unwrap();
    assert_eq!(doc["version"], 1);
    assert_eq!(doc["grammars"].as_array().unwrap().len(), 3, "all dev-stub grammars pinned");
    std::fs::remove_dir_all(&dir).ok();
}

/// GROVE-S01-T03 (AC4): end-to-end `grove init` — the orchestration split across
/// `grove_core::init::provision_project` (grammars + grove.lock) and the CLI
/// harness (.mcp.json + CLAUDE.md). Regression guard that the core/CLI split
/// leaves `grove init` behaving identically: the files written + stdout shape
/// per target and for --dry-run.
#[test]
fn init_provisions_and_wires_harness_per_target() {
    // A fake OS cache seeded with the rust grammar so init's filesystem
    // `is_cached` check passes deterministically (resolve still reads the full
    // grammar from the dev-stub GROVE_REGISTRY). GROVE_REGISTRY_URL points at a
    // dead host so the hosted catalog is skipped — detection falls back to the
    // dev-stub manifests, with no network access.
    let base = std::env::temp_dir().join(format!("grove_cli_test_{}_init", std::process::id()));
    let cache = base.join("cache");
    let rust_cache = cache.join("grove").join("grammars").join("rust");
    std::fs::create_dir_all(&rust_cache).unwrap();
    std::fs::write(rust_cache.join("grammar.wasm"), b"").unwrap();

    let run = |proj: &Path, args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_grove"))
            .args(args)
            .current_dir(proj)
            .env("GROVE_REGISTRY", DEV_REGISTRY)
            .env("GROVE_REGISTRY_URL", "http://127.0.0.1:1")
            .env("XDG_CACHE_HOME", &cache)
            .output()
            .expect("running grove init")
    };
    let seed_proj = |name: &str| {
        let p = base.join(name);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("lib.rs"), "fn helper() {}\n").unwrap();
        p
    };

    // --- default target (mcp): writes .mcp.json + CLAUDE.md + grove.lock ---
    let mcp = seed_proj("mcp");
    let out = run(&mcp, &["init"]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("detected"), "narrates detection: {text}");
    assert!(text.contains("rust"), "names the detected language: {text}");
    assert!(text.contains("wrote"), "narrates the wrote block: {text}");
    assert!(mcp.join(".mcp.json").exists(), "mcp target writes .mcp.json");
    assert!(mcp.join("CLAUDE.md").exists(), "mcp target writes steering");
    assert!(mcp.join("grove.lock").exists(), "provisioning writes the lock");
    // wrote order preserved across the split: .mcp.json, then CLAUDE.md, then grove.lock.
    let (i_mcp, i_claude, i_lock) = (
        text.find(".mcp.json").unwrap(),
        text.find("CLAUDE.md").unwrap(),
        text.find("grove.lock").unwrap(),
    );
    assert!(i_mcp < i_claude && i_claude < i_lock, "wrote order .mcp.json, CLAUDE.md, grove.lock: {text}");

    // --- grammars target: only grove.lock, no harness files ---
    let gram = seed_proj("grammars");
    let g = run(&gram, &["init", "--as", "grammars"]);
    assert!(g.status.success(), "stderr: {}", String::from_utf8_lossy(&g.stderr));
    assert!(gram.join("grove.lock").exists(), "grammars target pins the lock");
    assert!(!gram.join(".mcp.json").exists(), "grammars target writes no .mcp.json");
    assert!(!gram.join("CLAUDE.md").exists(), "grammars target writes no steering");

    // --- dry-run: detects but writes nothing ---
    let dry = seed_proj("dry");
    let d = run(&dry, &["init", "--dry-run"]);
    assert!(d.status.success(), "stderr: {}", String::from_utf8_lossy(&d.stderr));
    let dtext = String::from_utf8_lossy(&d.stdout);
    assert!(dtext.contains("detected"), "dry-run still narrates detection: {dtext}");
    assert!(dtext.contains("dry run"), "dry-run prints the dry-run note: {dtext}");
    assert!(!dry.join("grove.lock").exists(), "dry-run writes no lock");
    assert!(!dry.join(".mcp.json").exists(), "dry-run writes no .mcp.json");
    assert!(!dry.join("CLAUDE.md").exists(), "dry-run writes no steering");

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn index_writes_catalog() {
    let dir = fixture("index");
    let out = grove(&dir, &["index", DEV_REGISTRY, "-o", dir.join("index.json").to_str().unwrap()]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let cat: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).unwrap()).unwrap();
    assert_eq!(cat["grammars"].as_array().unwrap().len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}

/// GROVE-S02-T04 (AC-6): when `.grove/explore.json` names an unreachable provider,
/// `grove serve` falls back to the standard 7-tool structural surface and emits a
/// diagnostic note to stderr. No real provider is required — port 1 fails immediately.
#[test]
fn explore_mode_unhealthy_provider_falls_back_to_standard_surface() {
    use std::io::Write;
    use std::process::Stdio;

    let dir = std::env::temp_dir().join(format!(
        "grove_cli_test_{}_explore_fallback",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join(".grove")).unwrap();

    // Port 1 is IANA reserved — guaranteed connection-refused (fast fail).
    let config = serde_json::json!({
        "provider": "ollama",
        "base_url": "http://127.0.0.1:1/v1",
        "model": "nomodel",
        "mode": "standard",
        "allowed_tools": []
    });
    std::fs::write(
        dir.join(".grove").join("explore.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_grove"))
        .arg("serve")
        .arg(dir.to_str().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GROVE_REGISTRY", DEV_REGISTRY)
        .spawn()
        .expect("spawning grove serve");

    let mut stdin = child.stdin.take().unwrap();
    // initialize (id=1)
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": "2025-06-18" }
    });
    writeln!(stdin, "{init_req}").unwrap();
    // tools/list (id=2)
    let list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    writeln!(stdin, "{list_req}").unwrap();
    drop(stdin); // close stdin → server exits at EOF

    let output = child.wait_with_output().expect("grove serve to finish");

    // Parse stdout lines and find the tools/list response (id=2).
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let tools_response = stdout_str
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|v| v["id"] == serde_json::json!(2))
        .expect("tools/list response (id=2) must be present");

    let tools = tools_response["result"]["tools"]
        .as_array()
        .expect("result.tools is an array");
    assert_eq!(
        tools.len(),
        7,
        "unhealthy explore provider must fall back to the 7-tool standard surface, got: {tools:?}"
    );

    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr_str.contains("falling back"),
        "stderr must contain 'falling back' diagnostic; got: {stderr_str}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unknown_extension_errors() {
    let dir = fixture("badext");
    std::fs::write(dir.join("notes.unknownext"), "hello\n").unwrap();
    let out = grove(&dir, &["outline", "notes.unknownext"]);
    assert!(!out.status.success(), "no grammar for unknown extension");
    assert!(String::from_utf8_lossy(&out.stderr).contains("no registered grammar"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn map_returns_definitions_with_references() {
    let dir = fixture("map");

    // JSON output: structured file maps with references.
    let j = grove(&dir, &["--json", "map", "."]);
    assert!(j.status.success(), "stderr: {}", String::from_utf8_lossy(&j.stderr));
    let v: serde_json::Value = serde_json::from_str(&stdout(&j)).unwrap();
    let arr = v.as_array().expect("map returns an array");
    assert!(!arr.is_empty(), "map should return at least one file map");
    let fm = &arr[0];
    let entries = fm["entries"].as_array().expect("entries is an array");
    // The fixture has helper + caller + Thing + method.
    assert!(entries.len() >= 3, "should have multiple definitions, got {}", entries.len());

    // main/caller references helper.
    let caller = entries.iter().find(|e| e["name"] == "caller").expect("caller definition");
    let refs = caller["references"].as_array().expect("references is an array");
    assert!(refs.iter().any(|r| r.as_str() == Some("helper")),
        "caller should reference helper, got {:?}", refs);

    // Human-readable output.
    let human = grove(&dir, &["map", "."]);
    assert!(human.status.success());
    let text = stdout(&human);
    assert!(text.contains("helper"), "human map shows helper: {text}");
    assert!(text.contains("caller"), "human map shows caller: {text}");

    // Kind filter.
    let j2 = grove(&dir, &["--json", "map", ".", "--kind", "function"]);
    assert!(j2.status.success());
    let v2: serde_json::Value = serde_json::from_str(&stdout(&j2)).unwrap();
    let entries2 = v2.as_array().unwrap()[0]["entries"].as_array().unwrap();
    assert!(entries2.iter().all(|e| e["kind"] == "function"), "kind filter works");

    std::fs::remove_dir_all(&dir).ok();
}

/// GROVE-S02-T06 (AC1-AC7): `grove init --as mcp-llm` integration tests.
///
/// Shared setup: fake OS cache with the rust grammar (same pattern as
/// `init_provisions_and_wires_harness_per_target`).
fn mcp_llm_setup(tag: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let base = std::env::temp_dir().join(format!("grove_cli_test_{}_mcp_llm_{tag}", std::process::id()));
    let cache = base.join("cache");
    let rust_cache = cache.join("grove").join("grammars").join("rust");
    std::fs::create_dir_all(&rust_cache).unwrap();
    std::fs::write(rust_cache.join("grammar.wasm"), b"").unwrap();
    let proj = base.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("lib.rs"), "fn helper() {}\n").unwrap();
    (base, cache, proj)
}

fn grove_mcp_llm(proj: &Path, cache: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_grove"))
        .args(args)
        .current_dir(proj)
        .env("GROVE_REGISTRY", DEV_REGISTRY)
        .env("GROVE_REGISTRY_URL", "http://127.0.0.1:1")
        .env("XDG_CACHE_HOME", cache)
        .output()
        .expect("running grove init --as mcp-llm")
}

/// AC5: dry-run prints the planned harness files and exits 0 (non-TTY, no explore.json).
/// The non-TTY guard is bypassed because --dry-run is set.
#[test]
fn mcp_llm_dry_run_output_shape() {
    let (base, cache, proj) = mcp_llm_setup("dry_run");
    let out = grove_mcp_llm(&proj, &cache, &["init", "--as", "mcp-llm", "--dry-run"]);
    let text = String::from_utf8_lossy(&out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stderr: {err}\nstdout: {text}");
    assert!(text.contains("detected"), "dry-run narrates detection: {text}");
    assert!(text.contains("mcp.json"), "dry-run prints .mcp.json: {text}");
    assert!(text.contains("CLAUDE.md"), "dry-run prints CLAUDE.md: {text}");
    assert!(text.contains("AGENTS.md"), "dry-run prints AGENTS.md: {text}");
    // No files written.
    assert!(!proj.join(".mcp.json").exists(), "dry-run writes no .mcp.json");
    assert!(!proj.join("CLAUDE.md").exists(), "dry-run writes no CLAUDE.md");
    assert!(!proj.join("AGENTS.md").exists(), "dry-run writes no AGENTS.md");
    std::fs::remove_dir_all(&base).ok();
}

/// AC3 + AC6: two consecutive runs produce exactly one grove sentinel block in
/// each of CLAUDE.md and AGENTS.md. Pre-seed explore.json so the non-TTY
/// guard is bypassed (CI re-runs work without an interactive terminal).
#[test]
fn mcp_llm_steering_block_idempotency() {
    let (base, cache, proj) = mcp_llm_setup("idempotency");
    // Pre-seed .grove/explore.json so the TUI is not launched and the non-TTY
    // guard is bypassed.
    let grove_dir = proj.join(".grove");
    std::fs::create_dir_all(&grove_dir).unwrap();
    std::fs::write(
        grove_dir.join("explore.json"),
        serde_json::json!({
            "provider": "ollama",
            "base_url": "http://127.0.0.1:11434/v1",
            "model": "llama3",
            "mode": "standard",
            "allowed_tools": []
        })
        .to_string(),
    )
    .unwrap();

    // First run.
    let out1 = grove_mcp_llm(&proj, &cache, &["init", "--as", "mcp-llm"]);
    let err1 = String::from_utf8_lossy(&out1.stderr);
    assert!(out1.status.success(), "first run failed — stderr: {err1}");

    // Second run (idempotency).
    let out2 = grove_mcp_llm(&proj, &cache, &["init", "--as", "mcp-llm"]);
    let err2 = String::from_utf8_lossy(&out2.stderr);
    assert!(out2.status.success(), "second run failed — stderr: {err2}");

    let claude = std::fs::read_to_string(proj.join("CLAUDE.md")).unwrap();
    let agents = std::fs::read_to_string(proj.join("AGENTS.md")).unwrap();

    assert_eq!(claude.matches("<!-- grove:start -->").count(), 1, "CLAUDE.md: exactly 1 grove block");
    assert_eq!(claude.matches("<!-- grove:end -->").count(), 1);
    assert_eq!(agents.matches("<!-- grove:start -->").count(), 1, "AGENTS.md: exactly 1 grove block");
    assert_eq!(agents.matches("<!-- grove:end -->").count(), 1);

    std::fs::remove_dir_all(&base).ok();
}

/// AC3 + AC6: AGENTS.md is created when absent; when hand-written content
/// exists it is preserved and the grove block is appended exactly once.
#[test]
fn mcp_llm_agents_md_created_and_appended() {
    let (base, cache, proj) = mcp_llm_setup("agents_append");
    // Pre-seed explore.json so TUI / non-TTY guard is bypassed.
    let grove_dir = proj.join(".grove");
    std::fs::create_dir_all(&grove_dir).unwrap();
    std::fs::write(
        grove_dir.join("explore.json"),
        serde_json::json!({
            "provider": "ollama",
            "base_url": "http://127.0.0.1:11434/v1",
            "model": "llama3",
            "mode": "standard",
            "allowed_tools": []
        })
        .to_string(),
    )
    .unwrap();

    // (a) No AGENTS.md: first run creates it.
    assert!(!proj.join("AGENTS.md").exists(), "AGENTS.md must not exist before first run");
    let out1 = grove_mcp_llm(&proj, &cache, &["init", "--as", "mcp-llm"]);
    assert!(out1.status.success(), "stderr: {}", String::from_utf8_lossy(&out1.stderr));
    assert!(proj.join("AGENTS.md").exists(), "AGENTS.md created on first run");
    let agents1 = std::fs::read_to_string(proj.join("AGENTS.md")).unwrap();
    assert!(agents1.contains("<!-- grove:start -->"), "grove block present");

    // (b) Hand-written content present: grove block appended, existing content preserved.
    // Simulate: add hand-written content before the existing grove block by
    // prepending it to the current file.
    let hand = "# My Agent\n\nCustom notes here.\n\n";
    // Re-seed without a grove block to test the append path.
    std::fs::write(proj.join("AGENTS.md"), hand).unwrap();
    // Remove CLAUDE.md so the test doesn't conflict (it was also written).
    std::fs::remove_file(proj.join("CLAUDE.md")).ok();
    // Remove grove.lock so init re-provisions (needed to re-write harness).
    std::fs::remove_file(proj.join("grove.lock")).ok();

    let out2 = grove_mcp_llm(&proj, &cache, &["init", "--as", "mcp-llm"]);
    assert!(out2.status.success(), "stderr: {}", String::from_utf8_lossy(&out2.stderr));
    let agents2 = std::fs::read_to_string(proj.join("AGENTS.md")).unwrap();
    assert!(agents2.contains("Custom notes here."), "hand-written content preserved");
    assert!(agents2.contains("<!-- grove:start -->"), "grove block appended");
    assert_eq!(agents2.matches("<!-- grove:start -->").count(), 1, "exactly one grove block");
    assert!(
        agents2.find("Custom notes").unwrap() < agents2.find("<!-- grove:start -->").unwrap(),
        "hand-written content precedes grove block"
    );

    std::fs::remove_dir_all(&base).ok();
}
