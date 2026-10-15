use serde_json::Value;

use super::sync::uri_to_path;
use super::{Session, Workspace};
use crate::diagnostics::span_to_range;
use forgels_protocol::{Diagnostic, DiagnosticSeverity};

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
    let diagnostics: Vec<Diagnostic> = found
        .iter()
        .map(|item| {
            let range = span_to_range(&doc.text, item.span);
            Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::Error),
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
