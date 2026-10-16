use std::path::PathBuf;

use serde_json::Value;

use forgels_core::diagnostics::{offset_to_position, position_to_offset};
use forgels_core::dispatch::{Session, flush_all, handle_message, immediate_notifications};
use forgels_core::routing::Language;
use forgels_core::workspace::Workspace;
use forgels_lang_solidity::SolidityAnalyzer;
use forgels_protocol::Position;

const A_URI: &str = "file:///repo/cross_a.sol";
const B_URI: &str = "file:///repo/cross_b.sol";

struct Harness {
    session: Session,
    workspace: Workspace,
    notes: Vec<Value>,
}

impl Harness {
    fn new() -> Self {
        let mut session = Session::new();
        session
            .analyzers
            .insert(Language::Solidity, Box::new(SolidityAnalyzer::new()));
        Self {
            session,
            workspace: Workspace::new(),
            notes: Vec::new(),
        }
    }

    fn send(&mut self, body: &str) -> Option<Value> {
        let reply = handle_message(&mut self.session, &mut self.workspace, body);
        for note in immediate_notifications(&self.session, &mut self.workspace, body) {
            let frame: Value = serde_json::from_str(&note).expect("note is json");
            self.notes.push(frame);
        }
        reply.map(|text| serde_json::from_str(&text).expect("reply is json"))
    }

    fn flush(&mut self) -> Vec<Value> {
        let mut out = Vec::new();
        for note in flush_all(&self.session, &mut self.workspace) {
            let frame: Value = serde_json::from_str(&note).expect("note is json");
            self.notes.push(frame.clone());
            out.push(frame);
        }
        out
    }

    fn publishes_for(&self, uri: &str) -> Vec<&Value> {
        self.notes
            .iter()
            .filter(|note| {
                note.pointer("/method").is_some_and(|method| {
                    method == &Value::String("textDocument/publishDiagnostics".to_string())
                }) && note
                    .pointer("/params/uri")
                    .is_some_and(|value| value == &Value::String(uri.to_string()))
            })
            .collect()
    }
}

fn load(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).expect("fixture exists")
}

fn initialize_body() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "capabilities": {} },
    })
    .to_string()
}

fn open_body(uri: &str, text: &str, version: i32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": uri,
                "languageId": "solidity",
                "version": version,
                "text": text,
            }
        },
    })
    .to_string()
}

fn change_body(uri: &str, text: &str, version: i32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": uri, "version": version },
            "contentChanges": [{ "text": text }],
        },
    })
    .to_string()
}

fn close_body(uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": { "textDocument": { "uri": uri } },
    })
    .to_string()
}

fn position_params(uri: &str, text: &str, offset: usize) -> Value {
    let pos = offset_to_position(text, offset as u32);
    serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": pos.line, "character": pos.character },
    })
}

fn request_body(id: u32, method: &str, params: Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
    .to_string()
}

fn open_pair() -> (Harness, String, String) {
    let mut harness = Harness::new();
    let a_text = load("cross_a.sol");
    let b_text = load("cross_b.sol");
    harness.send(&initialize_body()).expect("init replies");
    harness.send(&open_body(B_URI, &b_text, 1));
    harness.send(&open_body(A_URI, &a_text, 1));
    (harness, a_text, b_text)
}

fn diagnostics_for(note: &Value) -> Vec<Value> {
    note.pointer("/params/diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

#[test]
fn did_open_links_router_to_vault() {
    let (harness, _, _) = open_pair();
    let a_path = PathBuf::from("/repo/cross_a.sol");
    let b_path = PathBuf::from("/repo/cross_b.sol");
    assert!(harness.workspace.get(&a_path).is_some(), "A stays tracked");
    assert!(harness.workspace.get(&b_path).is_some(), "B stays tracked");
    assert!(
        harness.workspace.index().deps_of(&a_path).contains(&b_path),
        "A depends on B, got {:?}",
        harness.workspace.index().deps_of(&a_path)
    );
    let published = harness.publishes_for(A_URI);
    assert!(!published.is_empty(), "A publishes on open");
    let last = published.last().expect("has publish");
    assert!(
        diagnostics_for(last).is_empty(),
        "A resolves clean, got {last}"
    );
}

#[test]
fn completion_suggests_withdraw_from_vault() {
    let (mut harness, a_text, _) = open_pair();
    let at = a_text.rfind("with").expect("prefix") + "with".len();
    let body = request_body(
        10,
        "textDocument/completion",
        position_params(A_URI, &a_text, at),
    );
    let reply = harness.send(&body).expect("completion replies");
    let items = reply
        .pointer("/result/items")
        .and_then(Value::as_array)
        .expect("has items");
    let hit = items.iter().find(|item| {
        item.pointer("/label")
            .is_some_and(|label| label == &Value::String("withdraw".to_string()))
    });
    let hit = hit.expect("withdraw suggested");
    let detail = hit.pointer("/detail").and_then(Value::as_str).unwrap_or("");
    assert!(
        detail.contains("from cross_b.sol"),
        "origin tagged, got {detail}"
    );
}

#[test]
fn hover_shows_withdraw_with_origin() {
    let (mut harness, a_text, _) = open_pair();
    let at = a_text.rfind("withdraw").expect("usage");
    let body = request_body(
        11,
        "textDocument/hover",
        position_params(A_URI, &a_text, at),
    );
    let reply = harness.send(&body).expect("hover replies");
    let value = reply
        .pointer("/result/contents/value")
        .and_then(Value::as_str)
        .expect("has value");
    assert!(
        value.contains("function withdraw"),
        "decl shown, got {value}"
    );
    assert!(
        value.contains("from cross_b.sol"),
        "origin tagged, got {value}"
    );
}

#[test]
fn definition_jumps_to_vault_withdraw() {
    let (mut harness, a_text, b_text) = open_pair();
    let at = a_text.rfind("withdraw").expect("usage");
    let body = request_body(
        12,
        "textDocument/definition",
        position_params(A_URI, &a_text, at),
    );
    let reply = harness.send(&body).expect("definition replies");
    assert_eq!(
        reply.pointer("/result/uri").expect("has uri"),
        &Value::String(B_URI.to_string())
    );
    let start_line = reply
        .pointer("/result/range/start/line")
        .and_then(Value::as_u64)
        .expect("has start line") as u32;
    let start_char = reply
        .pointer("/result/range/start/character")
        .and_then(Value::as_u64)
        .expect("has start char") as u32;
    let end_line = reply
        .pointer("/result/range/end/line")
        .and_then(Value::as_u64)
        .expect("has end line") as u32;
    let end_char = reply
        .pointer("/result/range/end/character")
        .and_then(Value::as_u64)
        .expect("has end char") as u32;
    let start = position_to_offset(
        &b_text,
        Position {
            line: start_line,
            character: start_char,
        },
    );
    let end = position_to_offset(
        &b_text,
        Position {
            line: end_line,
            character: end_char,
        },
    );
    let slice = b_text
        .get(start as usize..end as usize)
        .expect("range in file");
    assert!(
        slice.contains("withdraw"),
        "target covers decl, got {slice:?}"
    );
}

#[test]
fn change_b_republishes_dirty_dependent() {
    let (mut harness, _, b_text) = open_pair();
    let edited = format!("{b_text}\n");
    harness.send(&change_body(B_URI, &edited, 2));
    let flushed = harness.flush();
    let uris: Vec<&str> = flushed
        .iter()
        .filter_map(|note| note.pointer("/params/uri").and_then(Value::as_str))
        .collect();
    assert!(uris.contains(&A_URI), "dependent republished, got {uris:?}");
    assert!(
        uris.contains(&B_URI),
        "changed file republished, got {uris:?}"
    );
    let for_a = flushed
        .iter()
        .find(|note| {
            note.pointer("/params/uri")
                .is_some_and(|uri| uri == &Value::String(A_URI.to_string()))
        })
        .expect("A frame");
    assert!(
        diagnostics_for(for_a).is_empty(),
        "A stays clean, got {for_a}"
    );
}

#[test]
fn close_b_marks_import_unresolved_in_a() {
    let (mut harness, _, _) = open_pair();
    harness.send(&close_body(B_URI));
    let flushed = harness.flush();
    let for_a = flushed
        .iter()
        .find(|note| {
            note.pointer("/params/uri")
                .is_some_and(|uri| uri == &Value::String(A_URI.to_string()))
        })
        .expect("A republished");
    let diagnostics = diagnostics_for(for_a);
    assert_eq!(diagnostics.len(), 1, "one unresolved, got {diagnostics:?}");
    let message = diagnostics[0]
        .pointer("/message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        message.contains("./cross_b.sol"),
        "names missing import, got {message}"
    );
}
