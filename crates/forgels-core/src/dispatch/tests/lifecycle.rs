use super::super::{Session, handle_message, serve_session};
use super::stub::init;
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
            "signatureHelpProvider",
            "textDocumentSync"
        ]
    );
    let triggers = caps
        .pointer("/completionProvider/triggerCharacters")
        .expect("has triggers");
    assert_eq!(triggers, &serde_json::json!(["\"", "'", "/"]));
    let sig_triggers = caps
        .pointer("/signatureHelpProvider/triggerCharacters")
        .expect("has signature triggers");
    assert_eq!(sig_triggers, &serde_json::json!(["(", ","]));
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
    init(&mut session, &mut workspace);
    let body = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/inlineValue","params":{}}"#;
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
fn ignores_standard_noise_notifications() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    for method in [
        "initialized",
        "$/setTrace",
        "$/cancelRequest",
        "textDocument/didSave",
    ] {
        let body = format!(r#"{{"jsonrpc":"2.0","method":"{method}","params":{{}}}}"#);
        assert!(
            handle_message(&mut session, &mut workspace, &body).is_none(),
            "{method} stays silent"
        );
    }
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
    init(&mut session, &mut workspace);
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
    let init = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"capabilities":{}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#;
    let exit = r#"{"jsonrpc":"2.0","method":"exit"}"#;
    let mut raw = framed(init);
    raw.extend_from_slice(&framed(shutdown));
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

#[test]
fn request_before_initialize_gets_not_initialized() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"textDocument/hover","params":{}}"#;
    let reply = handle_message(&mut session, &mut workspace, body).expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert_eq!(
        value.pointer("/error/code").expect("has code"),
        &serde_json::json!(-32002)
    );
    // Server still accepts initialize afterwards.
    assert!(handle_message(&mut session, &mut workspace, &init_body("2")).is_some());
    assert!(session.initialized);
}

#[test]
fn notification_before_initialize_is_dropped() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/A.sol","languageId":"solidity","version":1,"text":"contract A {}"}}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
    assert!(
        workspace
            .get(&std::path::PathBuf::from("/repo/A.sol"))
            .is_none()
    );
}

#[test]
fn request_after_shutdown_gets_invalid_request() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let shutdown = r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#;
    assert!(handle_message(&mut session, &mut workspace, shutdown).is_some());
    let body = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{}}"#;
    let reply = handle_message(&mut session, &mut workspace, body).expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert_eq!(
        value.pointer("/error/code").expect("has code"),
        &serde_json::json!(-32600)
    );
}
