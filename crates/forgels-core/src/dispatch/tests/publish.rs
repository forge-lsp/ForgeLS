use super::super::{Session, drain_due, flush_all, handle_message, immediate_notifications};
use super::stub::session_with_stub;
use crate::workspace::Workspace;
use serde_json::Value;

fn open_broken(session: &mut Session, workspace: &mut Workspace) {
    super::stub::init(session, workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/B.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
    assert!(handle_message(session, workspace, open).is_none());
}

#[test]
fn open_publishes_current_diagnostics() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_broken(&mut session, &mut workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/B.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
    let notes = immediate_notifications(&session, &mut workspace, open);
    assert_eq!(notes.len(), 1);
    let value: Value = serde_json::from_str(&notes[0]).expect("frame is json");
    assert_eq!(
        value.pointer("/method").expect("has method"),
        &serde_json::json!("textDocument/publishDiagnostics")
    );
    let diagnostics = value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].pointer("/message").expect("has message"),
        &serde_json::json!("stub broken")
    );
}

#[test]
fn close_publishes_empty_and_clears_store() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_broken(&mut session, &mut workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/B.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
    assert_eq!(
        immediate_notifications(&session, &mut workspace, open).len(),
        1
    );
    let close = r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///repo/B.sol"}}}"#;
    let mut session = session_with_stub(true);
    super::stub::init(&mut session, &mut workspace);
    assert!(handle_message(&mut session, &mut workspace, close).is_none());
    let notes = immediate_notifications(&session, &mut workspace, close);
    assert_eq!(notes.len(), 1);
    let value: Value = serde_json::from_str(&notes[0]).expect("frame is json");
    assert!(
        value
            .pointer("/params/diagnostics")
            .expect("has diagnostics")
            .as_array()
            .expect("array")
            .is_empty()
    );
}

#[test]
fn change_publishes_only_when_due() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_broken(&mut session, &mut workspace);
    let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/B.sol","version":2},"contentChanges":[{"text":"broken!"}]}}"#;
    assert!(handle_message(&mut session, &mut workspace, change).is_none());
    let now = std::time::Instant::now();
    assert!(drain_due(&session, &mut workspace, now).is_empty());
    let later = now + std::time::Duration::from_secs(60);
    let notes = drain_due(&session, &mut workspace, later);
    assert_eq!(notes.len(), 1);
}

#[test]
fn shutdown_flushes_pending() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_broken(&mut session, &mut workspace);
    let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/B.sol","version":2},"contentChanges":[{"text":"broken!"}]}}"#;
    assert!(handle_message(&mut session, &mut workspace, change).is_none());
    let notes = flush_all(&session, &mut workspace);
    assert_eq!(notes.len(), 1);
    assert!(flush_all(&session, &mut workspace).is_empty());
}
