use super::super::{Session, handle_message, serve_session};
use crate::workspace::Workspace;
use serde_json::Value;

fn init_body(id: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"initialize","params":{{"capabilities":{{"textDocument":{{"hover":{{"dynamicRegistration":true}}}}}}}}}}"#
    )
}

#[test]
fn advertises_exact_beta_capabilities() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let reply = handle_message(&mut session, &mut workspace, &init_body("1")).expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let caps = value
        .pointer("/result/capabilities")
        .expect("has capabilities");
    let mut keys: Vec<&str> = caps
        .as_object()
        .expect("capabilities is object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "completionProvider",
            "definitionProvider",
            "hoverProvider",
            "textDocumentSync"
        ]
    );
}

#[test]
fn stores_client_capabilities() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    handle_message(&mut session, &mut workspace, &init_body("7")).expect("must reply");
    let caps = session.client_capabilities.expect("stored");
    assert!(caps.pointer("/textDocument/hover").is_some());
}

#[test]
fn replies_method_not_found_and_keeps_running() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let body = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/signatureHelp","params":{}}"#;
    let reply = handle_message(&mut session, &mut workspace, body).expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert_eq!(
        value.pointer("/error/code").expect("has code"),
        &serde_json::json!(-32601)
    );
    assert!(value.pointer("/result").is_none());
    // Server still serves the next request.
    assert!(handle_message(&mut session, &mut workspace, &init_body("3")).is_some());
}

#[test]
fn ignores_unknown_notification_without_id() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let body = r#"{"jsonrpc":"2.0","method":"something/custom","params":{}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

#[test]
fn ignores_notifications_without_id() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let body = r#"{"jsonrpc":"2.0","method":"initialize","params":{"capabilities":{}}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

#[test]
fn ignores_invalid_json() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    assert!(handle_message(&mut session, &mut workspace, "not json").is_none());
}

#[test]
fn shutdown_replies_null_and_arms_flag() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let body = r#"{"jsonrpc":"2.0","id":9,"method":"shutdown","params":{}}"#;
    let reply = handle_message(&mut session, &mut workspace, body).expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert_eq!(value.pointer("/id").expect("has id"), &serde_json::json!(9));
    assert!(value.pointer("/result").expect("has result").is_null());
    assert!(session.shutdown_received);
    assert!(session.exit_code().is_none());
}

fn framed(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

#[test]
fn shutdown_then_exit_returns_zero() {
    let shutdown = r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#;
    let exit = r#"{"jsonrpc":"2.0","method":"exit"}"#;
    let mut raw = framed(shutdown);
    raw.extend_from_slice(&framed(exit));
    let mut input = std::io::Cursor::new(raw);
    let mut output = Vec::new();
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let code =
        serve_session(&mut input, &mut output, &mut session, &mut workspace).expect("serve runs");
    assert_eq!(code, 0);
    let text = String::from_utf8(output).expect("utf8 frames");
    assert!(text.contains(r#""result":null"#));
}

#[test]
fn exit_without_shutdown_returns_nonzero() {
    let exit = r#"{"jsonrpc":"2.0","method":"exit"}"#;
    let mut input = std::io::Cursor::new(framed(exit));
    let mut output = Vec::new();
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let code =
        serve_session(&mut input, &mut output, &mut session, &mut workspace).expect("serve runs");
    assert_eq!(code, 1);
    assert!(output.is_empty());
}

#[test]
fn clean_eof_without_exit_returns_zero() {
    let mut input = std::io::Cursor::new(Vec::new());
    let mut output = Vec::new();
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let code =
        serve_session(&mut input, &mut output, &mut session, &mut workspace).expect("serve runs");
    assert_eq!(code, 0);
}
