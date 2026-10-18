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

const OVERRIDE_SRC: &str = "contract B {\nfunction greet(uint a) public virtual { a; }\n}\ncontract D is B {\nfunction greet(uint a) public { a; }\n}\n";
const CLEAN_SRC: &str = "contract A {}\n";

fn run_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run fls")
}

#[test]
fn human_shape_reports_one_based_line_col() {
    let dir = fresh_dir("human");
    let file = dir.join("a.sol");
    write_file(&file, OVERRIDE_SRC);
    let out = run_in(&dir, &["check", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("error[correctness/override]"),
        "human line: {stdout}"
    );
    assert!(stdout.contains(":5:10:"), "one based position: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn json_shape_has_zero_based_ranges_fixed_and_summary() {
    let dir = fresh_dir("json");
    let file = dir.join("a.sol");
    write_file(&file, OVERRIDE_SRC);
    let out = run_in(&dir, &["check", "--json", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        value.pointer("/summary").expect("summary"),
        &serde_json::json!({"files": 1, "errors": 1, "warnings": 0, "fixed": 0})
    );
    let diag = value.pointer("/files/0/diagnostics/0").expect("diagnostic");
    assert_eq!(
        diag.pointer("/rule").expect("rule"),
        &serde_json::json!("correctness/override")
    );
    assert_eq!(
        diag.pointer("/range/start").expect("start"),
        &serde_json::json!({"line": 4, "character": 9})
    );
    assert_eq!(
        diag.pointer("/fixed").expect("fixed"),
        &serde_json::json!(false)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_applies_safe_fix_and_marks_flags() {
    let dir = fresh_dir("write");
    let file = dir.join("a.sol");
    write_file(&file, OVERRIDE_SRC);
    let out = run_in(
        &dir,
        &["check", "--write", "--json", file.to_str().expect("utf8")],
    );
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        value.pointer("/summary/fixed").expect("fixed count"),
        &serde_json::json!(1)
    );
    assert_eq!(
        value
            .pointer("/files/0/diagnostics/0/fixed")
            .expect("diag fixed"),
        &serde_json::json!(true)
    );
    assert_eq!(
        value
            .pointer("/files/0/diagnostics/0/fix/applicability")
            .expect("applicability"),
        &serde_json::json!("safe")
    );
    let after = std::fs::read_to_string(&file).expect("read back");
    assert!(after.contains("override"), "file fixed: {after}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exit_codes_cover_clean_warn_err() {
    let dir = fresh_dir("codes");
    let clean = dir.join("clean.sol");
    let err = dir.join("err.sol");
    write_file(&clean, CLEAN_SRC);
    write_file(&err, OVERRIDE_SRC);
    let ok = run_in(&dir, &["check", clean.to_str().expect("utf8")]);
    assert_eq!(ok.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&ok.stdout).is_empty());
    let bad = run_in(&dir, &["check", err.to_str().expect("utf8")]);
    assert_eq!(bad.status.code(), Some(1));
    write_file(
        &dir.join("forgels.toml"),
        "[lint.rules]\n\"correctness/override\" = \"warn\"\n",
    );
    let warn = run_in(&dir, &["check", err.to_str().expect("utf8")]);
    assert_eq!(warn.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&warn.stdout).contains("warning[correctness/override]"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_exits_two() {
    let dir = fresh_dir("missing");
    let out = run_in(
        &dir,
        &["check", dir.join("nope.sol").to_str().expect("utf8")],
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("cannot read"),
        "stderr warns"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn broken_config_warns_but_runs() {
    let dir = fresh_dir("broken");
    let file = dir.join("a.sol");
    write_file(&file, CLEAN_SRC);
    write_file(&dir.join("forgels.toml"), "publish_delay_ms = [\n");
    let out = run_in(&dir, &["check", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&out.stderr).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_rule_exits_two() {
    let dir = fresh_dir("unknown");
    let file = dir.join("a.sol");
    write_file(&file, CLEAN_SRC);
    write_file(
        &dir.join("forgels.toml"),
        "[lint.rules]\n\"correctness/nope\" = \"warn\"\n",
    );
    let out = run_in(&dir, &["check", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown lint rule"));
    let _ = std::fs::remove_dir_all(&dir);
}
