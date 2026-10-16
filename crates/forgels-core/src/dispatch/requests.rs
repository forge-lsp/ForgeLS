use serde_json::Value;

use super::sync::{path_to_uri, uri_to_path};
use super::{Session, Workspace};
use crate::analyzer::ProjectContext;
use crate::diagnostics::{position_to_offset, span_to_range};
use forgels_protocol::Position;

/// Answer `textDocument/hover`. Notifications without id stay silent.
/// Unknown files and words without declarations get a null result.
pub(crate) fn hover_response(
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
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let offset = position_to_offset(&doc.text, Position { line, character });
    let ctx = ProjectContext::new(&path, &doc.text, offset, workspace);
    let found = analyzer.hover(&ctx)?;
    let range = span_to_range(&doc.text, found.span);
    Some(serde_json::json!({
        "contents": { "kind": "plaintext", "value": found.contents },
        "range": {
            "start": { "line": range.start.line, "character": range.start.character },
            "end": { "line": range.end.line, "character": range.end.character },
        }
    }))
}

/// Answer `textDocument/completion`. Notifications without id stay silent.
/// Unknown files get an empty list.
pub(crate) fn completion_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = completion_result(session, workspace, params)
        .unwrap_or_else(|| serde_json::json!({ "isIncomplete": false, "items": [] }));
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn completion_result(session: &Session, workspace: &Workspace, params: &Value) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let position = params.get("position")?;
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let offset = position_to_offset(&doc.text, Position { line, character });
    let ctx = ProjectContext::new(&path, &doc.text, offset, workspace);
    let items: Vec<Value> = analyzer
        .complete(&ctx)
        .iter()
        .map(|item| serde_json::json!({ "label": item.label, "detail": item.detail }))
        .collect();
    Some(serde_json::json!({ "isIncomplete": false, "items": items }))
}

/// Answer `textDocument/definition`. Notifications without id stay silent.
/// Words without declarations get a null result.
pub(crate) fn definition_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = definition_result(session, workspace, params).unwrap_or(Value::Null);
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn definition_result(session: &Session, workspace: &Workspace, params: &Value) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let position = params.get("position")?;
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let offset = position_to_offset(&doc.text, Position { line, character });
    let ctx = ProjectContext::new(&path, &doc.text, offset, workspace);
    let target = analyzer.goto_location(&ctx)?;
    let target_text = workspace.get_text(&target.path)?;
    target_text.get(target.span.start as usize..target.span.end as usize)?;
    let range = span_to_range(target_text, target.span);
    let target_uri = path_to_uri(&target.path);
    Some(serde_json::json!({
        "uri": target_uri,
        "range": {
            "start": { "line": range.start.line, "character": range.start.character },
            "end": { "line": range.end.line, "character": range.end.character },
        }
    }))
}
