use serde_json::Value;

use crate::analyzer::AnalyzerMap;
use crate::workspace::Workspace;

mod publish;
mod requests;
mod sync;

#[cfg(test)]
mod tests;

pub use publish::{drain_due, flush_all, immediate_notifications};
pub(crate) use requests::{completion_response, definition_response, hover_response};
pub(crate) use sync::{
    apply_did_change, apply_did_close, apply_did_open, apply_folder_change, apply_initialize_roots,
};

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
    /// True once `initialize` succeeded. Requests before this get -32002.
    pub initialized: bool,
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
/// Handle one raw JSON-RPC body.
///
/// Lifecycle is strict: `exit` always works; requests before `initialize`
/// get -32002 and other early notifications are dropped; requests after
/// `shutdown` get -32600. Returns `Some(response_body)` for answered
/// requests, `None` for notifications and dropped messages.
pub fn handle_message(
    session: &mut Session,
    workspace: &mut Workspace,
    body: &str,
) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let method = value.get("method")?.as_str()?;
    let params = value.get("params").cloned().unwrap_or(Value::Null);
    // Exit always works, even before initialize, and never replies.
    if method == "exit" {
        session.exit_requested = true;
        return None;
    }
    // Before initialize: requests get ServerNotInitialized, other
    // notifications are dropped.
    if !session.initialized && method != "initialize" {
        let id = value.get("id")?.clone();
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32002, "message": "Server not initialized" }
        });
        return Some(response.to_string());
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
    // After shutdown: only exit may follow. Requests get InvalidRequest,
    // other notifications are dropped.
    if session.shutdown_received {
        let id = value.get("id")?.clone();
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32600, "message": "Invalid request after shutdown" }
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
        "textDocument/completion" => {
            return completion_response(session, workspace, &params, value.get("id"));
        }
        "textDocument/definition" => {
            return definition_response(session, workspace, &params, value.get("id"));
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
    session.initialized = true;
    apply_initialize_roots(workspace, &params);
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "capabilities": server_capabilities() }
    });
    Some(response.to_string())
}
