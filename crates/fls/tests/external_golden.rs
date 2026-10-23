use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fls"))
}

fn fresh_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fls-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("temp root");
    root
}

fn write_file(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent");
    }
    std::fs::write(path, text).expect("write fixture");
}

fn run_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run fls")
}

const LINT_JSON: &str = r#"{"diagnostics":[{"rule":"example/guest-rule","message":"guest says hi","start":0,"end":1,"severity":"warning"}]}"#;
const FMT_JSON: &str = r#"{"edits":[{"start":9,"end":10,"replacement":"B"}]}"#;
const RULES_JSON: &str = r#"["example/guest-rule"]"#;

fn wat_escape(raw: &str) -> String {
    raw.replace('"', "\\\"")
}

fn guest_wat() -> String {
    let lint_escaped = wat_escape(LINT_JSON);
    let fmt_escaped = wat_escape(FMT_JSON);
    let rules_escaped = wat_escape(RULES_JSON);
    format!(
        r#"(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 4096))
  (data (i32.const 16) "{lint_escaped}")
  (data (i32.const 512) "{fmt_escaped}")
  (data (i32.const 1024) "{rules_escaped}")
  (func (export "forgels_plugin_abi_version") (result i32) (i32.const 1))
  (func (export "alloc") (param $size i32) (result i32)
    (global.get $heap)
    (global.get $heap)
    (local.get $size)
    (i32.add)
    (global.set $heap))
  (func (export "free") (param i32 i32))
  (func (export "lint") (param i32 i32) (result i64)
    (i64.or
      (i64.shl (i64.extend_i32_u (i32.const 16)) (i64.const 32))
      (i64.extend_i32_u (i32.const {lint_len}))))
  (func (export "format") (param i32 i32) (result i64)
    (i64.or
      (i64.shl (i64.extend_i32_u (i32.const 512)) (i64.const 32))
      (i64.extend_i32_u (i32.const {fmt_len}))))
  (func (export "forgels_plugin_rules") (result i64)
    (i64.or
      (i64.shl (i64.extend_i32_u (i32.const 1024)) (i64.const 32))
      (i64.extend_i32_u (i32.const {rules_len})))))
"#,
        lint_len = LINT_JSON.len(),
        fmt_len = FMT_JSON.len(),
        rules_len = RULES_JSON.len(),
    )
}

fn write_guest(dir: &Path) {
    let wat_text = guest_wat();
    let bytes = wat::parse_str(&wat_text).expect("wat compiles");
    std::fs::write(dir.join("plugin.wasm"), &bytes).expect("write wasm");
}

fn write_config(dir: &Path, rule_level: &str) {
    let text = format!(
        r#"{{"lint":{{"includes":[],"excludes":[],"rules":{{"example/guest-rule":"{rule_level}"}},"external":[{{"path":"plugin.wasm"}}]}},"format":{{}}}}"#
    );
    write_file(&dir.join("fls.json"), &text);
}

const CLEAN_SRC: &str = "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n";

#[test]
fn guest_lint_appears_in_json() {
    let dir = fresh_dir("ext-lint");
    write_guest(&dir);
    write_config(&dir, "warn");
    let file = dir.join("a.sol");
    write_file(&file, CLEAN_SRC);
    let out = run_in(&dir, &["check", "--json", file.to_str().expect("utf8")]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let diags = value
        .pointer("/files/0/diagnostics")
        .and_then(|list| list.as_array())
        .expect("diagnostics array");
    let found = diags
        .iter()
        .find(|item| item.pointer("/rule") == Some(&serde_json::json!("example/guest-rule")));
    let found = found.expect("guest diagnostic present");
    assert_eq!(
        found.pointer("/message").expect("message"),
        &serde_json::json!("guest says hi")
    );
    assert_eq!(
        found.pointer("/severity").expect("severity"),
        &serde_json::json!("warning")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn guest_off_suppresses_diagnostic() {
    let dir = fresh_dir("ext-off");
    write_guest(&dir);
    write_config(&dir, "off");
    let file = dir.join("a.sol");
    write_file(&file, CLEAN_SRC);
    let out = run_in(&dir, &["check", "--json", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.trim().is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let diags = value
        .pointer("/files/0/diagnostics")
        .and_then(|list| list.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        diags
            .iter()
            .all(|item| item.pointer("/rule") != Some(&serde_json::json!("example/guest-rule"))),
        "off drops guest: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn guest_format_write_twice_identical_and_check_clean() {
    let dir = fresh_dir("ext-fmt");
    write_guest(&dir);
    write_config(&dir, "warn");
    let file = dir.join("a.sol");
    write_file(&file, "contract A {}\n");
    let first = run_in(&dir, &["format", "--write", file.to_str().expect("utf8")]);
    assert_eq!(first.status.code(), Some(0));
    let once = std::fs::read_to_string(&file).expect("read once");
    assert_eq!(once, "contract B {}\n", "guest edit applied: {once}");
    let second = run_in(&dir, &["format", "--write", file.to_str().expect("utf8")]);
    assert_eq!(second.status.code(), Some(0));
    let twice = std::fs::read_to_string(&file).expect("read twice");
    assert_eq!(once, twice, "second write byte-identical");
    let check = run_in(&dir, &["format", "--check", file.to_str().expect("utf8")]);
    assert_eq!(check.status.code(), Some(0));
    let lint = run_in(&dir, &["check", file.to_str().expect("utf8")]);
    assert_eq!(lint.status.code(), Some(0), "warn-only check exits 0");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn guest_runs_despite_style_only_filter() {
    let dir = fresh_dir("ext-filter");
    write_guest(&dir);
    let text = "{\"plugin\":[\"forge:style\"],\"lint\":{\"includes\":[],\"excludes\":[],\"rules\":{\"example/guest-rule\":\"warn\"},\"external\":[{\"path\":\"plugin.wasm\"}]},\"format\":{}}";
    write_file(&dir.join("fls.json"), text);
    let file = dir.join("a.sol");
    write_file(
        &file,
        "// SPDX-License-Identifier: MIT\ncontract B {\n/// @notice Greets.\n/// @param a Value.\nfunction greet(uint a) public virtual { a; }\n}\n\n\ncontract D is B {\n/// @notice Greets.\n/// @param a Value.\nfunction greet(uint a) public { a; }\n}\n",
    );
    let out = run_in(&dir, &["check", "--json", file.to_str().expect("utf8")]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let diags = value
        .pointer("/files/0/diagnostics")
        .and_then(|list| list.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        diags
            .iter()
            .any(|item| item.pointer("/rule") == Some(&serde_json::json!("example/guest-rule"))),
        "guest passes filter: {stdout}"
    );
    assert!(
        diags
            .iter()
            .any(|item| item.pointer("/rule") == Some(&serde_json::json!("style/filename-match"))),
        "style kept: {stdout}"
    );
    assert!(
        diags
            .iter()
            .all(|item| item.pointer("/rule") != Some(&serde_json::json!("correctness/override"))),
        "correctness dropped: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
