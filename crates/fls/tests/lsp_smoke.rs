use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fls"))
}

fn framed(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

#[test]
fn lsp_help_exits_zero() {
    let out = Command::new(binary())
        .args(["lsp", "--help"])
        .output()
        .expect("run fls lsp --help");
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--log-level"), "help lists flag: {text}");
}

#[test]
fn version_exits_zero_and_names_fls() {
    let out = Command::new(binary())
        .arg("--version")
        .output()
        .expect("run fls --version");
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("fls"), "version names fls: {text}");
}

#[test]
fn lsp_handshake_answers_initialize_and_exits_zero() {
    let mut child = Command::new(binary())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn fls lsp");
    let init = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"capabilities":{}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":1,"method":"shutdown","params":{}}"#;
    let exit = r#"{"jsonrpc":"2.0","method":"exit"}"#;
    let mut input = Vec::new();
    input.extend_from_slice(&framed(init));
    input.extend_from_slice(&framed(shutdown));
    input.extend_from_slice(&framed(exit));
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(&input)
        .expect("write frames");
    let out = child.wait_with_output().expect("wait lsp");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"id\":0"),
        "initialize response present: {stdout}"
    );
    assert!(
        stdout.contains("capabilities"),
        "capabilities present: {stdout}"
    );
    assert!(
        stdout.contains("\"id\":1"),
        "shutdown reply present: {stdout}"
    );
}
