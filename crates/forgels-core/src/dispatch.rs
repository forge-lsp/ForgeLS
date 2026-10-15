use serde_json::Value;

use crate::routing::route_file;
use crate::workspace::Workspace;

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
pub fn serve_session(
    reader: &mut impl std::io::BufRead,
    writer: &mut impl std::io::Write,
    session: &mut Session,
    workspace: &mut Workspace,
) -> std::io::Result<i32> {
    while session.exit_code().is_none() {
        let Some(body) = crate::transport::read_message(reader)? else {
            break;
        };
        if let Some(reply) = handle_message(session, workspace, &body) {
            crate::transport::write_message(writer, &reply)?;
        }
    }
    Ok(session.exit_code().unwrap_or(0))
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
        let body = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{}}"#;
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
}
