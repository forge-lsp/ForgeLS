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

const CLEAN_SRC: &str = "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n";
const DIRTY_SRC: &str = "pragma solidity ^0.8.0;   \ncontract A {}";

fn idempotency_fixtures() -> Vec<(&'static str, String)> {
    vec![
        ("spaces", "contract A {}   \n".to_string()),
        ("tabs", "contract A {}\t\t\n".to_string()),
        (
            "blanks",
            "contract A {}\n   \n\t\ncontract B {}\n".to_string(),
        ),
        ("missing-eof", "contract A {}".to_string()),
        ("crlf", "contract A {}\r\ncontract B {}  \r\n".to_string()),
        ("empty", String::new()),
    ]
}

#[test]
fn help_lists_format() {
    let dir = fresh_dir("fmt-help");
    let out = run_in(&dir, &["--help"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("format"), "help lists format: {stdout}");
    let out = run_in(&dir, &["format", "--help"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--write"), "format help: {stdout}");
    assert!(stdout.contains("--check"), "format help: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_noop_exits_zero_unchanged() {
    let dir = fresh_dir("fmt-noop");
    let file = dir.join("a.sol");
    write_file(&file, CLEAN_SRC);
    let out = run_in(&dir, &["format", "--write", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(std::fs::read_to_string(&file).expect("read"), CLEAN_SRC);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_change_trims_and_adds_eof() {
    let dir = fresh_dir("fmt-write");
    let file = dir.join("a.sol");
    write_file(&file, DIRTY_SRC);
    let out = run_in(&dir, &["format", "--write", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        std::fs::read_to_string(&file).expect("read"),
        "pragma solidity ^0.8.0;\ncontract A {}\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_clean_exits_zero() {
    let dir = fresh_dir("fmt-check-clean");
    let file = dir.join("a.sol");
    write_file(&file, CLEAN_SRC);
    let out = run_in(&dir, &["format", "--check", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_dirty_lists_files_without_writing() {
    let dir = fresh_dir("fmt-check-dirty");
    let file = dir.join("a.sol");
    write_file(&file, DIRTY_SRC);
    let out = run_in(&dir, &["format", "--check", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("a.sol"), "file list: {stdout}");
    assert_eq!(
        std::fs::read_to_string(&file).expect("read"),
        DIRTY_SRC,
        "check writes nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_and_check_together_exits_two() {
    let dir = fresh_dir("fmt-conflict");
    let file = dir.join("a.sol");
    write_file(&file, CLEAN_SRC);
    let out = run_in(
        &dir,
        &["format", "--write", "--check", file.to_str().expect("utf8")],
    );
    assert_eq!(out.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn default_prints_formatted_stdout_without_writing() {
    let dir = fresh_dir("fmt-preview");
    let file = dir.join("a.sol");
    write_file(&file, DIRTY_SRC);
    let out = run_in(&dir, &["format", file.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "pragma solidity ^0.8.0;\ncontract A {}\n"
    );
    assert_eq!(
        std::fs::read_to_string(&file).expect("read"),
        DIRTY_SRC,
        "preview writes nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn format_is_idempotent_on_fixtures() {
    let dir = fresh_dir("fmt-idem");
    for (name, src) in idempotency_fixtures() {
        let file = dir.join(format!("{name}.sol"));
        write_file(&file, &src);
        let first = run_in(&dir, &["format", "--write", file.to_str().expect("utf8")]);
        assert_eq!(first.status.code(), Some(0), "first pass {name}");
        let once = std::fs::read_to_string(&file).expect("read once");
        let second = run_in(&dir, &["format", "--write", file.to_str().expect("utf8")]);
        assert_eq!(second.status.code(), Some(0), "second pass {name}");
        let twice = std::fs::read_to_string(&file).expect("read twice");
        assert_eq!(once, twice, "idempotent for {name}");
        let check = run_in(&dir, &["format", "--check", file.to_str().expect("utf8")]);
        assert_eq!(check.status.code(), Some(0), "stable for {name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
