use serde_json::Value;

use crate::analyzer::AnalyzerMap;
use crate::diagnostics::{position_to_offset, span_to_range};
use crate::routing::route_file;
use crate::workspace::Workspace;
use forgels_protocol::Position;

/// Capabilities the beta server advertises.
///
/// Keep this list exact. The initialize test asserts no more and no less.
pub fn server_capabilities() -> Value {
    serde_json::json!({
        "textDocumentSync": 1,
        "hoverProvider": true,
        "completionProvider": {},
        "definitionProvider": true
    })
}

/// Per-connection session state.
#[derive(Debug, Default)]
pub struct Session {
    /// Raw `params.capabilities` from the last initialize request.
    pub client_capabilities: Option<Value>,
    /// True once a `shutdown` request was answered.
    pub shutdown_received: bool,
    /// True once an `exit` notification arrived.
    pub exit_requested: bool,
    /// Registered language analyzers, wired by the binary.
    pub analyzers: AnalyzerMap,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process code once `exit` arrived. `0` after shutdown, `1` without.
    pub fn exit_code(&self) -> Option<i32> {
        if !self.exit_requested {
            return None;
        }
        Some(i32::from(!self.shutdown_received))
    }
}

/// Drive the session until `exit` or EOF. Returns the process code.
///
/// After every message, due debounced publishes flush. After shutdown,
/// all pending publishes flush so the client sees final diagnostics.
pub fn serve_session(
    reader: &mut impl std::io::BufRead,
    writer: &mut impl std::io::Write,
    session: &mut Session,
    workspace: &mut Workspace,
) -> std::io::Result<i32> {
    let mut shutdown_flushed = false;
    while session.exit_code().is_none() {
        let Some(body) = crate::transport::read_message(reader)? else {
            break;
        };
        if let Some(reply) = handle_message(session, workspace, &body) {
            crate::transport::write_message(writer, &reply)?;
        }
        for note in immediate_notifications(session, workspace, &body) {
            crate::transport::write_message(writer, &note)?;
        }
        let now = std::time::Instant::now();
        for note in drain_due(session, workspace, now) {
            crate::transport::write_message(writer, &note)?;
        }
        if session.shutdown_received && !shutdown_flushed {
            shutdown_flushed = true;
            for note in flush_all(session, workspace) {
                crate::transport::write_message(writer, &note)?;
            }
        }
    }
    Ok(session.exit_code().unwrap_or(0))
}

/// Immediate notifications for one message body: open publishes current
/// diagnostics at once, close pushes an empty array.
pub fn immediate_notifications(
    session: &Session,
    workspace: &mut Workspace,
    body: &str,
) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let Some(method) = value.get("method").and_then(Value::as_str) else {
        return Vec::new();
    };
    let params = value.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "textDocument/didOpen" => {
            let Some(uri) = params
                .get("textDocument")
                .and_then(|doc| doc.get("uri"))
                .and_then(Value::as_str)
            else {
                return Vec::new();
            };
            publish_for_uri(session, workspace, uri, None)
                .into_iter()
                .collect()
        }
        "textDocument/didClose" => {
            let Some(uri) = params
                .get("textDocument")
                .and_then(|doc| doc.get("uri"))
                .and_then(Value::as_str)
            else {
                return Vec::new();
            };
            vec![clear_frame(uri)]
        }
        _ => Vec::new(),
    }
}

/// Publish frames for debounced URIs due at `now`.
pub fn drain_due(
    session: &Session,
    workspace: &mut Workspace,
    now: std::time::Instant,
) -> Vec<String> {
    workspace
        .take_due_publish(now)
        .iter()
        .filter_map(|(uri, version)| publish_for_uri(session, workspace, uri, Some(*version)))
        .collect()
}

/// Publish frames for every pending URI, due or not.
pub fn flush_all(session: &Session, workspace: &mut Workspace) -> Vec<String> {
    workspace
        .take_all_pending()
        .iter()
        .filter_map(|(uri, version)| publish_for_uri(session, workspace, uri, Some(*version)))
        .collect()
}

/// Build one `publishDiagnostics` frame for `uri`, storing the result.
/// Returns `None` for untracked files and languages without analyzers.
fn publish_for_uri(
    session: &Session,
    workspace: &mut Workspace,
    uri: &str,
    version: Option<u64>,
) -> Option<String> {
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let found = analyzer.diagnostics(&doc.text);
    let diagnostics: Vec<forgels_protocol::Diagnostic> = found
        .iter()
        .map(|item| {
            let range = crate::diagnostics::span_to_range(&doc.text, item.span);
            forgels_protocol::Diagnostic {
                range,
                severity: Some(forgels_protocol::DiagnosticSeverity::Error),
                message: item.message.clone(),
            }
        })
        .collect();
    workspace.set_diagnostics(path, diagnostics.clone());
    let items: Vec<Value> = diagnostics
        .iter()
        .map(|diagnostic| {
            serde_json::json!({
                "range": {
                    "start": { "line": diagnostic.range.start.line, "character": diagnostic.range.start.character },
                    "end": { "line": diagnostic.range.end.line, "character": diagnostic.range.end.character },
                },
                "severity": 1,
                "message": diagnostic.message,
            })
        })
        .collect();
    let mut params = serde_json::Map::new();
    params.insert("uri".to_string(), Value::String(uri.to_string()));
    if let Some(version) = version {
        params.insert("version".to_string(), Value::Number(version.into()));
    }
    params.insert("diagnostics".to_string(), Value::Array(items));
    let frame = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": Value::Object(params),
    });
    Some(frame.to_string())
}

/// Empty `publishDiagnostics` frame that clears one document.
fn clear_frame(uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": [] },
    })
    .to_string()
}

/// Turn a `file://` URI into a path. Plain paths pass through.
/// Percent escapes stay encoded; see the uri-decoding follow-up item.
fn uri_to_path(uri: &str) -> std::path::PathBuf {
    let stripped = uri.strip_prefix("file://").unwrap_or(uri);
    std::path::PathBuf::from(stripped)
}

/// Register roots from initialize params: folders, then rootUri, then rootPath.
fn apply_initialize_roots(workspace: &mut Workspace, params: &Value) {
    if let Some(folders) = params.get("workspaceFolders").and_then(Value::as_array) {
        let roots = folders
            .iter()
            .filter_map(|folder| folder.get("uri").and_then(Value::as_str))
            .map(uri_to_path)
            .collect();
        workspace.set_roots(roots);
    } else if let Some(uri) = params.get("rootUri").and_then(Value::as_str) {
        workspace.set_roots(vec![uri_to_path(uri)]);
    } else if let Some(path) = params.get("rootPath").and_then(Value::as_str) {
        workspace.set_roots(vec![std::path::PathBuf::from(path)]);
    }
}

/// Apply `didOpen`: route, then track. Unknown language or out-of-root
/// files stay untracked without noise.
fn apply_did_open(workspace: &mut Workspace, params: &Value) {
    let doc = params.get("textDocument");
    let (Some(uri), Some(language_id), Some(text)) = (
        doc.and_then(|doc| doc.get("uri")).and_then(Value::as_str),
        doc.and_then(|doc| doc.get("languageId"))
            .and_then(Value::as_str),
        doc.and_then(|doc| doc.get("text")).and_then(Value::as_str),
    ) else {
        return;
    };
    let path = uri_to_path(uri);
    if !workspace.allows(&path) {
        return;
    }
    if let Some(language) = route_file(&path, Some(language_id)) {
        let _ = workspace.open(path, language, text.to_string());
    }
}

/// Apply `didChange` with full text. Unknown documents stay ignored.
/// Successful changes mark the URI dirty for debounced publish.
fn apply_did_change(workspace: &mut Workspace, params: &Value) {
    let doc = params.get("textDocument");
    let (Some(uri), Some(changes)) = (
        doc.and_then(|doc| doc.get("uri")).and_then(Value::as_str),
        params.get("contentChanges").and_then(Value::as_array),
    ) else {
        return;
    };
    if let Some(last) = changes
        .last()
        .and_then(|change| change.get("text"))
        .and_then(Value::as_str)
        && workspace
            .change(&uri_to_path(uri), last.to_string())
            .is_ok()
        && let Some(version) = doc
            .and_then(|doc| doc.get("version"))
            .and_then(Value::as_u64)
    {
        workspace.mark_dirty(uri, version, std::time::Instant::now());
    }
}

/// Apply `didClose`. Unknown documents stay ignored.
fn apply_did_close(workspace: &mut Workspace, params: &Value) {
    if let Some(uri) = params
        .get("textDocument")
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
    {
        let _ = workspace.close(&uri_to_path(uri));
    }
}

/// Apply `workspace/didChangeWorkspaceFolders`: add, remove, evict orphans.
fn apply_folder_change(workspace: &mut Workspace, params: &Value) {
    let event = params.get("event");
    for key in ["added", "removed"] {
        let folders = event
            .and_then(|event| event.get(key))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for folder in &folders {
            if let Some(uri) = folder.get("uri").and_then(Value::as_str) {
                if key == "added" {
                    workspace.add_root(uri_to_path(uri));
                } else {
                    workspace.remove_root(&uri_to_path(uri));
                }
            }
        }
    }
    workspace.evict_outside_roots();
}

/// Answer `textDocument/hover`. Notifications without id stay silent.
/// Unknown files and words without declarations get a null result.
fn hover_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = hover_result(session, workspace, params).unwrap_or(Value::Null);
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn hover_result(session: &Session, workspace: &Workspace, params: &Value) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let position = params.get("position")?;
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;
    let doc = workspace.get(&uri_to_path(uri))?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let offset = position_to_offset(&doc.text, Position { line, character });
    let found = analyzer.hover(&doc.text, offset)?;
    let range = span_to_range(&doc.text, found.span);
    Some(serde_json::json!({
        "contents": { "kind": "plaintext", "value": found.contents },
        "range": {
            "start": { "line": range.start.line, "character": range.start.character },
            "end": { "line": range.end.line, "character": range.end.character },
        }
    }))
}

/// Handle one raw JSON-RPC body.
///
/// Returns `Some(response_body)` for `initialize` and `shutdown` requests.
/// `exit` and other methods return `None`. `exit` only flips session flags;
/// read the process code via [`Session::exit_code`].
/// Sync notifications (`didOpen`, `didChange`, `didClose`,
/// `didChangeWorkspaceFolders`) update the workspace and never reply.
pub fn handle_message(
    session: &mut Session,
    workspace: &mut Workspace,
    body: &str,
) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let method = value.get("method")?.as_str()?;
    let params = value.get("params").cloned().unwrap_or(Value::Null);
    if method == "exit" {
        session.exit_requested = true;
        return None;
    }
    if method == "shutdown" {
        session.shutdown_received = true;
        let id = value.get("id")?.clone();
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": null
        });
        return Some(response.to_string());
    }
    match method {
        "textDocument/didOpen" => {
            apply_did_open(workspace, &params);
            return None;
        }
        "textDocument/didChange" => {
            apply_did_change(workspace, &params);
            return None;
        }
        "textDocument/didClose" => {
            apply_did_close(workspace, &params);
            return None;
        }
        "textDocument/hover" => {
            return hover_response(session, workspace, &params, value.get("id"));
        }
        "workspace/didChangeWorkspaceFolders" => {
            apply_folder_change(workspace, &params);
            return None;
        }
        _ => {}
    }
    if method != "initialize" {
        // Unknown request gets a polite error. Unknown notification
        // without id stays silent. The server keeps running either way.
        let id = value.get("id")?.clone();
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {method}") }
        });
        return Some(response.to_string());
    }
    let id = value.get("id")?.clone();
    if let Some(caps) = value.pointer("/params/capabilities") {
        session.client_capabilities = Some(caps.clone());
    }
    apply_initialize_roots(workspace, &params);
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "capabilities": server_capabilities() }
    });
    Some(response.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_body(id: &str) -> String {
        format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"initialize","params":{{"capabilities":{{"textDocument":{{"hover":{{"dynamicRegistration":true}}}}}}}}}}"#
        )
    }

    #[test]
    fn advertises_exact_beta_capabilities() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        let reply =
            handle_message(&mut session, &mut workspace, &init_body("1")).expect("must reply");
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
        let code = serve_session(&mut input, &mut output, &mut session, &mut workspace)
            .expect("serve runs");
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
        let code = serve_session(&mut input, &mut output, &mut session, &mut workspace)
            .expect("serve runs");
        assert_eq!(code, 1);
        assert!(output.is_empty());
    }

    #[test]
    fn clean_eof_without_exit_returns_zero() {
        let mut input = std::io::Cursor::new(Vec::new());
        let mut output = Vec::new();
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        let code = serve_session(&mut input, &mut output, &mut session, &mut workspace)
            .expect("serve runs");
        assert_eq!(code, 0);
    }

    fn init_with_folders(id: &str) -> String {
        format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"initialize","params":{{"capabilities":{{}},"workspaceFolders":[{{"uri":"file:///repo/backend","name":"backend"}},{{"uri":"file:///repo/frontend","name":"frontend"}}]}}}}"#
        )
    }

    fn did_open(uri: &str, language: &str, text: &str) -> String {
        format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","languageId":"{language}","text":"{text}"}}}}}}"#
        )
    }

    #[test]
    fn initialize_registers_all_roots() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        assert!(handle_message(&mut session, &mut workspace, &init_with_folders("1")).is_some());
        assert!(workspace.allows(&std::path::PathBuf::from("/repo/backend/Token.sol")));
        assert!(workspace.allows(&std::path::PathBuf::from("/repo/frontend/App.nr")));
    }

    #[test]
    fn files_from_each_root_track_with_right_language() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        assert!(handle_message(&mut session, &mut workspace, &init_with_folders("1")).is_some());
        assert!(
            handle_message(
                &mut session,
                &mut workspace,
                &did_open(
                    "file:///repo/backend/Token.sol",
                    "solidity",
                    "contract A {}"
                )
            )
            .is_none()
        );
        assert!(
            handle_message(
                &mut session,
                &mut workspace,
                &did_open("file:///repo/frontend/App.nr", "noir", "fn main() {}")
            )
            .is_none()
        );
        let backend = workspace
            .get(&std::path::PathBuf::from("/repo/backend/Token.sol"))
            .expect("backend tracked");
        assert_eq!(backend.language, crate::routing::Language::Solidity);
        assert_eq!(backend.text, "contract A {}");
        let frontend = workspace
            .get(&std::path::PathBuf::from("/repo/frontend/App.nr"))
            .expect("frontend tracked");
        assert_eq!(frontend.language, crate::routing::Language::Noir);
    }

    #[test]
    fn change_and_close_flow_through_notifications() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        assert!(handle_message(&mut session, &mut workspace, &init_with_folders("1")).is_some());
        assert!(
            handle_message(
                &mut session,
                &mut workspace,
                &did_open("file:///repo/backend/Token.sol", "solidity", "v1")
            )
            .is_none()
        );
        let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/backend/Token.sol"},"contentChanges":[{"text":"v2"}]}}"#;
        assert!(handle_message(&mut session, &mut workspace, change).is_none());
        assert_eq!(
            workspace
                .get(&std::path::PathBuf::from("/repo/backend/Token.sol"))
                .expect("tracked")
                .text,
            "v2"
        );
        let close = r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///repo/backend/Token.sol"}}}"#;
        assert!(handle_message(&mut session, &mut workspace, close).is_none());
        assert!(
            workspace
                .get(&std::path::PathBuf::from("/repo/backend/Token.sol"))
                .is_none()
        );
    }

    #[test]
    fn change_marks_uri_dirty_with_version() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        assert!(handle_message(&mut session, &mut workspace, &init_with_folders("1")).is_some());
        assert!(
            handle_message(
                &mut session,
                &mut workspace,
                &did_open("file:///repo/backend/Token.sol", "solidity", "v1")
            )
            .is_none()
        );
        let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/backend/Token.sol","version":9},"contentChanges":[{"text":"v9"}]}}"#;
        assert!(handle_message(&mut session, &mut workspace, change).is_none());
        let due = workspace
            .take_due_publish(std::time::Instant::now() + std::time::Duration::from_secs(60));
        assert_eq!(due, vec![("file:///repo/backend/Token.sol".to_string(), 9)]);
    }

    #[test]
    fn change_without_version_skips_dirty_mark() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        assert!(handle_message(&mut session, &mut workspace, &init_with_folders("1")).is_some());
        assert!(
            handle_message(
                &mut session,
                &mut workspace,
                &did_open("file:///repo/backend/Token.sol", "solidity", "v1")
            )
            .is_none()
        );
        let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/backend/Token.sol"},"contentChanges":[{"text":"v2"}]}}"#;
        assert!(handle_message(&mut session, &mut workspace, change).is_none());
        assert_eq!(
            workspace
                .get(&std::path::PathBuf::from("/repo/backend/Token.sol"))
                .expect("tracked")
                .text,
            "v2"
        );
        let due = workspace
            .take_due_publish(std::time::Instant::now() + std::time::Duration::from_secs(60));
        assert!(due.is_empty());
    }

    #[test]
    fn folder_change_adds_removes_and_evicts() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        assert!(handle_message(&mut session, &mut workspace, &init_with_folders("1")).is_some());
        assert!(
            handle_message(
                &mut session,
                &mut workspace,
                &did_open("file:///repo/frontend/App.nr", "noir", "fn main() {}")
            )
            .is_none()
        );
        let change = r#"{"jsonrpc":"2.0","method":"workspace/didChangeWorkspaceFolders","params":{"event":{"added":[{"uri":"file:///repo/shared","name":"shared"}],"removed":[{"uri":"file:///repo/frontend","name":"frontend"}]}}}"#;
        assert!(handle_message(&mut session, &mut workspace, change).is_none());
        assert!(workspace.allows(&std::path::PathBuf::from("/repo/shared/X.sol")));
        assert!(!workspace.allows(&std::path::PathBuf::from("/repo/frontend/App.nr")));
        assert!(
            workspace
                .get(&std::path::PathBuf::from("/repo/frontend/App.nr"))
                .is_none()
        );
    }

    #[test]
    fn open_outside_roots_stays_untracked() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        assert!(handle_message(&mut session, &mut workspace, &init_with_folders("1")).is_some());
        assert!(
            handle_message(
                &mut session,
                &mut workspace,
                &did_open("file:///other/Stray.sol", "solidity", "contract S {}")
            )
            .is_none()
        );
        assert!(
            workspace
                .get(&std::path::PathBuf::from("/other/Stray.sol"))
                .is_none()
        );
    }

    #[test]
    fn unknown_language_and_extension_stay_silent() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        // No reply, no tracked file, no crash. Diagnostics do not exist
        // yet, and the path takes no log branch by construction.
        assert!(
            handle_message(
                &mut session,
                &mut workspace,
                &did_open("file:///repo/notes.md", "markdown", "# notes")
            )
            .is_none()
        );
        assert!(
            workspace
                .get(&std::path::PathBuf::from("/repo/notes.md"))
                .is_none()
        );
    }

    #[test]
    fn sync_on_untracked_file_stays_silent() {
        let mut session = Session::new();
        let mut workspace = Workspace::new();
        let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/ghost.sol"},"contentChanges":[{"text":"v2"}]}}"#;
        let close = r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///repo/ghost.sol"}}}"#;
        assert!(handle_message(&mut session, &mut workspace, change).is_none());
        assert!(handle_message(&mut session, &mut workspace, close).is_none());
        assert!(
            workspace
                .get(&std::path::PathBuf::from("/repo/ghost.sol"))
                .is_none()
        );
    }

    #[derive(Debug)]
    struct StubAnalyzer {
        found: bool,
    }

    impl crate::analyzer::Analyzer for StubAnalyzer {
        fn hover(&self, _text: &str, _offset: u32) -> Option<crate::analyzer::HoverData> {
            if self.found {
                Some(crate::analyzer::HoverData {
                    contents: "stub detail".to_string(),
                    span: forgels_syntax::Span::new(13, 18),
                })
            } else {
                None
            }
        }

        fn complete(&self, _text: &str, _offset: u32) -> Vec<crate::analyzer::CompletionData> {
            Vec::new()
        }

        fn goto(&self, _text: &str, _offset: u32) -> Option<forgels_syntax::Span> {
            None
        }

        fn diagnostics(&self, text: &str) -> Vec<crate::analyzer::DiagnosticData> {
            if text.contains("broken") {
                vec![crate::analyzer::DiagnosticData {
                    span: forgels_syntax::Span::new(0, 6),
                    message: "stub broken".to_string(),
                }]
            } else {
                Vec::new()
            }
        }
    }

    fn session_with_stub(found: bool) -> Session {
        let mut session = Session::new();
        session.analyzers.insert(
            crate::routing::Language::Solidity,
            Box::new(StubAnalyzer { found }),
        );
        session
    }

    fn open_sol(workspace: &mut Workspace) {
        let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/A.sol","languageId":"solidity","version":1,"text":"contract A {}"}}}"#;
        let mut session = Session::new();
        assert!(handle_message(&mut session, workspace, open).is_none());
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

    fn open_broken(workspace: &mut Workspace) {
        let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/B.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
        let mut session = Session::new();
        assert!(handle_message(&mut session, workspace, open).is_none());
    }

    #[test]
    fn open_publishes_current_diagnostics() {
        let session = session_with_stub(true);
        let mut workspace = Workspace::new();
        open_broken(&mut workspace);
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
        let session = session_with_stub(true);
        let mut workspace = Workspace::new();
        open_broken(&mut workspace);
        let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/B.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
        assert_eq!(
            immediate_notifications(&session, &mut workspace, open).len(),
            1
        );
        let close = r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///repo/B.sol"}}}"#;
        let mut session = Session::new();
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
        let session = session_with_stub(true);
        let mut workspace = Workspace::new();
        open_broken(&mut workspace);
        let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/B.sol","version":2},"contentChanges":[{"text":"broken!"}]}}"#;
        let mut plain = Session::new();
        assert!(handle_message(&mut plain, &mut workspace, change).is_none());
        let now = std::time::Instant::now();
        assert!(drain_due(&session, &mut workspace, now).is_empty());
        let later = now + std::time::Duration::from_secs(60);
        let notes = drain_due(&session, &mut workspace, later);
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn shutdown_flushes_pending() {
        let session = session_with_stub(true);
        let mut workspace = Workspace::new();
        open_broken(&mut workspace);
        let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/B.sol","version":2},"contentChanges":[{"text":"broken!"}]}}"#;
        let mut plain = Session::new();
        assert!(handle_message(&mut plain, &mut workspace, change).is_none());
        let notes = flush_all(&session, &mut workspace);
        assert_eq!(notes.len(), 1);
        assert!(flush_all(&session, &mut workspace).is_empty());
    }
}
