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
        .map(|item| completion_item(&doc.text, item))
        .collect();
    Some(serde_json::json!({ "isIncomplete": false, "items": items }))
}

/// One completion item as JSON. Path items carry `textEdit` so the
/// client replaces the full inner string instead of inserting at
/// the cursor. Invalid spans stay plain to avoid broken edits.
fn completion_item(text: &str, item: &crate::analyzer::CompletionData) -> Value {
    let mut value = serde_json::json!({
        "label": item.label,
        "detail": item.detail,
        "kind": item.kind.value(),
    });
    let Some(edit) = item.text_edit.as_ref() else {
        return value;
    };
    if text
        .get(edit.span.start as usize..edit.span.end as usize)
        .is_none()
    {
        return value;
    }
    let range = span_to_range(text, edit.span);
    value["textEdit"] = serde_json::json!({
        "range": {
            "start": { "line": range.start.line, "character": range.start.character },
            "end": { "line": range.end.line, "character": range.end.character },
        },
        "newText": edit.new_text,
    });
    value
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
    let target_text = match workspace.get_text(&target.path) {
        Some(text) => text.to_string(),
        None => {
            if !forgels_common::fs::is_trackable_file(&target.path) {
                return None;
            }
            std::fs::read_to_string(&target.path).ok()?
        }
    };
    target_text.get(target.span.start as usize..target.span.end as usize)?;
    let range = span_to_range(&target_text, target.span);
    let target_uri = path_to_uri(&target.path);
    Some(serde_json::json!({
        "uri": target_uri,
        "range": {
            "start": { "line": range.start.line, "character": range.start.character },
            "end": { "line": range.end.line, "character": range.end.character },
        }
    }))
}

/// Answer `textDocument/signatureHelp`. Notifications without id stay
/// silent. Positions outside any call and unknown callees get null.
pub(crate) fn signature_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = signature_result(session, workspace, params).unwrap_or(Value::Null);
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn signature_result(session: &Session, workspace: &Workspace, params: &Value) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let position = params.get("position")?;
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let offset = position_to_offset(&doc.text, Position { line, character });
    let ctx = ProjectContext::new(&path, &doc.text, offset, workspace);
    let list = analyzer.signature_help(&ctx)?;
    let head = list.first()?;
    let signatures: Vec<Value> = list
        .iter()
        .map(|item| {
            let params: Vec<Value> = item
                .params
                .iter()
                .map(|param| serde_json::json!({ "label": param.label }))
                .collect();
            serde_json::json!({ "label": item.label, "parameters": params })
        })
        .collect();
    Some(serde_json::json!({
        "signatures": signatures,
        "activeSignature": 0,
        "activeParameter": head.active_param,
    }))
}

/// Answer `textDocument/documentSymbol`. Notifications without id stay
/// silent. Unknown files get a null result, open files get the array
/// (possibly empty).
pub(crate) fn document_symbol_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = document_symbol_result(session, workspace, params).unwrap_or(Value::Null);
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn document_symbol_result(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let items: Vec<Value> = analyzer
        .document_symbols(&doc.text)
        .iter()
        .map(|item| symbol_value(&doc.text, item))
        .collect();
    Some(Value::Array(items))
}

fn symbol_value(text: &str, item: &crate::analyzer::DocumentSymbolData) -> Value {
    let range = span_to_range(text, item.range);
    let selection = span_to_range(text, item.selection);
    let children: Vec<Value> = item
        .children
        .iter()
        .map(|child| symbol_value(text, child))
        .collect();
    serde_json::json!({
        "name": item.name,
        "detail": item.detail,
        "kind": item.kind.value(),
        "range": {
            "start": { "line": range.start.line, "character": range.start.character },
            "end": { "line": range.end.line, "character": range.end.character },
        },
        "selectionRange": {
            "start": { "line": selection.start.line, "character": selection.start.character },
            "end": { "line": selection.end.line, "character": selection.end.character },
        },
        "children": children,
    })
}
