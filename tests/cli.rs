//! End-to-end tests for the `grove` CLI (the `main.rs` dispatch + output paths).
//!
//! Each test runs the built binary against the 3-language dev stub in `registry/`
//! (pinned via `GROVE_REGISTRY` so results are deterministic regardless of any OS
//! cache) and a throwaway fixture project.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The dev-stub registry shipped in the source tree (rust, python, javascript).
const DEV_REGISTRY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/registry");

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
