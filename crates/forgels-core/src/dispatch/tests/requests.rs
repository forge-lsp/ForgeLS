use super::super::handle_message;
use super::stub::{open_sol, session_with_stub};
use crate::workspace::Workspace;
use serde_json::Value;

#[test]
fn hover_returns_contents_and_range() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &hover_body(5, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let result = value.get("result").expect("has result");
    assert_eq!(
        result.pointer("/contents/value").expect("has value"),
        &serde_json::json!("stub detail")
    );
    assert_eq!(
        result.pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 0, "character": 13})
    );
    assert_eq!(
        result.pointer("/range/end").expect("has end"),
        &serde_json::json!({"line": 0, "character": 13})
    );
}

#[test]
fn hover_without_declaration_returns_null() {
    let mut session = session_with_stub(false);
    let mut workspace = Workspace::new();
    open_sol(&mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &hover_body(5, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn hover_on_unknown_file_returns_null() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &hover_body(5, "file:///repo/ghost.sol", 0, 1),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn hover_without_id_stays_silent() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut workspace);
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/hover","params":{"textDocument":{"uri":"file:///repo/A.sol"},"position":{"line":0,"character":10}}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

fn hover_body(id: u32, uri: &str, line: u32, character: u32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        }
    })
    .to_string()
}

fn completion_body(id: u32, uri: &str, line: u32, character: u32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        }
    })
    .to_string()
}

#[test]
fn completion_lists_matching_labels() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &completion_body(6, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let result = value.get("result").expect("has result");
    assert_eq!(
        result.pointer("/isIncomplete").expect("has flag"),
        &serde_json::json!(false)
    );
    let items = result
        .pointer("/items")
        .expect("has items")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].pointer("/label").expect("has label"),
        &serde_json::json!("stub_one")
    );
}

#[test]
fn completion_on_unknown_file_returns_empty_list() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &completion_body(6, "file:///repo/ghost.sol", 0, 1),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result/items")
        .expect("has items")
        .as_array()
        .expect("array");
    assert!(items.is_empty());
}

fn definition_body(id: u32, uri: &str, line: u32, character: u32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        }
    })
    .to_string()
}

#[test]
fn definition_returns_uri_and_range() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &definition_body(7, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let result = value.get("result").expect("has result");
    assert_eq!(
        result.pointer("/uri").expect("has uri"),
        &serde_json::json!("file:///repo/A.sol")
    );
    assert_eq!(
        result.pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 0, "character": 13})
    );
}

#[test]
fn definition_on_unknown_file_returns_null() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &definition_body(7, "file:///repo/ghost.sol", 0, 1),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}
