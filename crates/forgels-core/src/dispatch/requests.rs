use serde_json::Value;

use super::sync::{path_to_uri, uri_to_path};
use super::{Session, Workspace};
use crate::analyzer::ProjectContext;
use crate::diagnostics::{offset_to_position, position_to_offset, span_to_range};
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

/// Answer `textDocument/references` with a `Location[]`.
/// Unknown files and unsupported words get an empty list.
/// `context.includeDeclaration` defaults to true; false drops decls.
pub(crate) fn references_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = references_result(session, workspace, params).unwrap_or(Value::Array(Vec::new()));
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn references_result(session: &Session, workspace: &Workspace, params: &Value) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let position = params.get("position")?;
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;
    let include_decl = params
        .get("context")
        .and_then(|ctx| ctx.get("includeDeclaration"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let offset = position_to_offset(&doc.text, Position { line, character });
    let ctx = ProjectContext::new(&path, &doc.text, offset, workspace);
    let found = analyzer.references(&ctx)?;
    let mut items = Vec::with_capacity(found.len());
    for entry in found.iter().take(crate::analyzer::MAX_REFERENCES) {
        if entry.is_declaration && !include_decl {
            continue;
        }
        let text: String = match workspace.get_text(&entry.path) {
            Some(cached) => cached.to_string(),
            None => {
                if !forgels_common::fs::is_trackable_file(&entry.path) {
                    continue;
                }
                let Ok(disk) = std::fs::read_to_string(&entry.path) else {
                    continue;
                };
                disk
            }
        };
        if text
            .get(entry.span.start as usize..entry.span.end as usize)
            .is_none()
        {
            continue;
        }
        let range = span_to_range(&text, entry.span);
        items.push(serde_json::json!({
            "uri": path_to_uri(&entry.path),
            "range": {
                "start": { "line": range.start.line, "character": range.start.character },
                "end": { "line": range.end.line, "character": range.end.character },
            },
        }));
    }
    Some(Value::Array(items))
}

/// Answer `textDocument/rename` with a `WorkspaceEdit`.
/// Valid renames map every reference to a `newText` edit grouped by
/// URI. Invalid names, unknown symbols, and unknown files answer
/// `InvalidParams` (-32602). No `prepareRename` in V1.
pub(crate) fn rename_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let Some(result) = rename_result(session, workspace, params) else {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": "Invalid rename position or name" }
        });
        return Some(response.to_string());
    };
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn rename_result(session: &Session, workspace: &Workspace, params: &Value) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let position = params.get("position")?;
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;
    let new_name = params.get("newName")?.as_str()?;
    if new_name.is_empty() {
        return None;
    }
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let offset = position_to_offset(&doc.text, Position { line, character });
    let ctx = ProjectContext::new(&path, &doc.text, offset, workspace);
    let found = analyzer.rename(&ctx, new_name)?;
    if found.is_empty() {
        return None;
    }
    let mut changes = serde_json::Map::new();
    for entry in found.iter().take(crate::analyzer::MAX_REFERENCES) {
        let text: String = match workspace.get_text(&entry.path) {
            Some(cached) => cached.to_string(),
            None => {
                if !forgels_common::fs::is_trackable_file(&entry.path) {
                    continue;
                }
                let Ok(disk) = std::fs::read_to_string(&entry.path) else {
                    continue;
                };
                disk
            }
        };
        if text
            .get(entry.span.start as usize..entry.span.end as usize)
            .is_none()
        {
            continue;
        }
        let range = span_to_range(&text, entry.span);
        let edit = serde_json::json!({
            "range": {
                "start": { "line": range.start.line, "character": range.start.character },
                "end": { "line": range.end.line, "character": range.end.character },
            },
            "newText": new_name,
        });
        changes
            .entry(path_to_uri(&entry.path))
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()?
            .push(edit);
    }
    if changes.is_empty() {
        return None;
    }
    Some(serde_json::json!({ "changes": Value::Object(changes) }))
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

/// Answer `workspace/symbol` with classic `SymbolInformation`.
/// Empty query returns an empty list. Notifications without id stay silent.
pub(crate) fn workspace_symbol_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result =
        workspace_symbol_result(session, workspace, params).unwrap_or(Value::Array(Vec::new()));
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn workspace_symbol_result(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
) -> Option<Value> {
    let query = params.get("query").and_then(Value::as_str).unwrap_or("");
    if query.is_empty() {
        return Some(Value::Array(Vec::new()));
    }
    let found = crate::analyzer::workspace_symbols(workspace, &session.analyzers, query);
    let mut items = Vec::with_capacity(found.len());
    for entry in &found {
        let Some(text) = workspace.get_text(&entry.path) else {
            continue;
        };
        if text
            .get(entry.span.start as usize..entry.span.end as usize)
            .is_none()
        {
            continue;
        }
        let range = span_to_range(text, entry.span);
        let mut value = serde_json::json!({
            "name": entry.name,
            "kind": entry.kind.value(),
            "location": {
                "uri": path_to_uri(&entry.path),
                "range": {
                    "start": { "line": range.start.line, "character": range.start.character },
                    "end": { "line": range.end.line, "character": range.end.character },
                },
            },
        });
        if let Some(container) = entry.container.as_ref() {
            value["containerName"] = Value::String(container.clone());
        }
        items.push(value);
    }
    Some(Value::Array(items))
}

/// Answer `textDocument/inlayHint` with a hint array.
/// Unknown files get an empty list. Notifications without id stay silent.
pub(crate) fn inlay_hint_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = inlay_hint_result(session, workspace, params).unwrap_or(Value::Array(Vec::new()));
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn inlay_hint_result(session: &Session, workspace: &Workspace, params: &Value) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let range = params.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    let start_pos = Position {
        line: start.get("line")?.as_u64()? as u32,
        character: start.get("character")?.as_u64()? as u32,
    };
    let end_pos = Position {
        line: end.get("line")?.as_u64()? as u32,
        character: end.get("character")?.as_u64()? as u32,
    };
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let start_off = position_to_offset(&doc.text, start_pos);
    let end_off = position_to_offset(&doc.text, end_pos);
    let span = forgels_syntax::Span::new(start_off.min(end_off), start_off.max(end_off));
    let ctx = ProjectContext::new(&path, &doc.text, start_off, workspace);
    let mut items = Vec::new();
    for hint in analyzer
        .inlay_hints(&ctx, span)
        .iter()
        .take(crate::analyzer::MAX_INLAY_HINTS)
    {
        if doc.text.get(..hint.offset as usize).is_none() {
            continue;
        }
        let pos = offset_to_position(&doc.text, hint.offset);
        items.push(serde_json::json!({
            "position": { "line": pos.line, "character": pos.character },
            "label": hint.label,
            "kind": hint.kind.value(),
            "paddingLeft": false,
            "paddingRight": true,
        }));
    }
    Some(Value::Array(items))
}

/// Answer `textDocument/semanticTokens/full` with delta-encoded data.
/// Unknown files get empty data. Notifications without id stay silent.
pub(crate) fn semantic_tokens_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = semantic_tokens_result(session, workspace, params)
        .unwrap_or(serde_json::json!({"data": []}));
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn semantic_tokens_result(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let tokens = analyzer.semantic_tokens(&doc.text);
    let mut data: Vec<u32> = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;
    for token in tokens.iter().take(crate::analyzer::MAX_SEMANTIC_TOKENS) {
        let Some(covered) = doc
            .text
            .get(token.span.start as usize..token.span.end as usize)
        else {
            continue;
        };
        if covered.is_empty() {
            continue;
        }
        let pos = offset_to_position(&doc.text, token.span.start);
        let len = covered.encode_utf16().count() as u32;
        if len == 0 {
            continue;
        }
        let Some(delta_line) = pos.line.checked_sub(prev_line) else {
            continue;
        };
        let delta_start = if delta_line == 0 {
            let Some(delta) = pos.character.checked_sub(prev_char) else {
                continue;
            };
            delta
        } else {
            pos.character
        };
        data.extend([
            delta_line,
            delta_start,
            len,
            token.token_type,
            token.modifiers,
        ]);
        prev_line = pos.line;
        prev_char = pos.character;
    }
    Some(serde_json::json!({ "data": data }))
}

fn call_item_value(text: &str, item: &crate::analyzer::CallHierarchyItemData) -> Option<Value> {
    text.get(item.span.start as usize..item.span.end as usize)?;
    text.get(item.selection.start as usize..item.selection.end as usize)?;
    let range = span_to_range(text, item.span);
    let selection = span_to_range(text, item.selection);
    Some(serde_json::json!({
        "name": item.name,
        "kind": item.kind.value(),
        "detail": item.detail,
        "uri": path_to_uri(&item.path),
        "range": {
            "start": { "line": range.start.line, "character": range.start.character },
            "end": { "line": range.end.line, "character": range.end.character },
        },
        "selectionRange": {
            "start": { "line": selection.start.line, "character": selection.start.character },
            "end": { "line": selection.end.line, "character": selection.end.character },
        },
    }))
}

fn load_item_text(workspace: &Workspace, path: &std::path::Path) -> Option<String> {
    if let Some(cached) = workspace.get_text(path) {
        return Some(cached.to_string());
    }
    if !forgels_common::fs::is_trackable_file(path) {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Answer `textDocument/prepareCallHierarchy` with a one item array.
/// Unknown files and non-callables get null. No id stays silent.
pub(crate) fn call_hierarchy_prepare_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = call_hierarchy_prepare_result(session, workspace, params).unwrap_or(Value::Null);
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn call_hierarchy_prepare_result(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
) -> Option<Value> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let position = params.get("position")?;
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;
    let path = uri_to_path(uri);
    let doc = workspace.get(&path)?;
    let analyzer = session.analyzers.get(&doc.language)?;
    let offset = position_to_offset(&doc.text, Position { line, character });
    let ctx = ProjectContext::new(&path, &doc.text, offset, workspace);
    let item = analyzer.prepare_call_hierarchy(&ctx)?;
    let text = load_item_text(workspace, &item.path)?;
    let value = call_item_value(&text, &item)?;
    Some(Value::Array(vec![value]))
}

fn call_hierarchy_item_ctx<'a>(
    session: &Session,
    workspace: &'a Workspace,
    item: &Value,
) -> Option<ProjectContext<'a>> {
    let uri = item.get("uri")?.as_str()?;
    let path = uri_to_path(uri);
    let (stored_path, doc) = workspace.iter_files().find(|(known, _)| **known == path)?;
    let selection = item.get("selectionRange")?.get("start")?;
    let range = item.get("range")?.get("start")?;
    let start = selection.get("line").or_else(|| range.get("line"))?;
    let character = selection
        .get("character")
        .or_else(|| range.get("character"))?;
    let pos = Position {
        line: start.as_u64()? as u32,
        character: character.as_u64()? as u32,
    };
    let offset = position_to_offset(&doc.text, pos);
    let language = doc.language;
    if !session.analyzers.contains_key(&language) {
        return None;
    }
    Some(ProjectContext::new(
        stored_path,
        &doc.text,
        offset,
        workspace,
    ))
}

/// Answer `callHierarchy/incomingCalls` with from items and ranges.
/// Unknown items get an empty list. No id stays silent.
pub(crate) fn call_hierarchy_incoming_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = call_hierarchy_incoming_result(session, workspace, params)
        .unwrap_or(Value::Array(Vec::new()));
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn call_hierarchy_incoming_result(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
) -> Option<Value> {
    let item = params.get("item")?;
    let ctx = call_hierarchy_item_ctx(session, workspace, item)?;
    let language = workspace.get(ctx.path)?.language;
    let analyzer = session.analyzers.get(&language)?;
    let found = analyzer.incoming_calls(&ctx)?;
    let mut out = Vec::with_capacity(found.len());
    for edge in found.iter().take(crate::analyzer::MAX_CALL_HIERARCHY) {
        let Some(edge_text) = load_item_text(workspace, &edge.item.path) else {
            continue;
        };
        let Some(from) = call_item_value(&edge_text, &edge.item) else {
            continue;
        };
        let mut ranges = Vec::with_capacity(edge.ranges.len());
        for span in &edge.ranges {
            if edge_text
                .get(span.start as usize..span.end as usize)
                .is_none()
            {
                continue;
            }
            let range = span_to_range(&edge_text, *span);
            ranges.push(serde_json::json!({
                "start": { "line": range.start.line, "character": range.start.character },
                "end": { "line": range.end.line, "character": range.end.character },
            }));
        }
        out.push(serde_json::json!({ "from": from, "fromRanges": ranges }));
    }
    Some(Value::Array(out))
}

/// Answer `callHierarchy/outgoingCalls` with to items and ranges.
/// Unknown items get an empty list. No id stays silent.
pub(crate) fn call_hierarchy_outgoing_response(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
    id: Option<&Value>,
) -> Option<String> {
    let id = id?.clone();
    let result = call_hierarchy_outgoing_result(session, workspace, params)
        .unwrap_or(Value::Array(Vec::new()));
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Some(response.to_string())
}

fn call_hierarchy_outgoing_result(
    session: &Session,
    workspace: &Workspace,
    params: &Value,
) -> Option<Value> {
    let item = params.get("item")?;
    let ctx = call_hierarchy_item_ctx(session, workspace, item)?;
    let language = workspace.get(ctx.path)?.language;
    let analyzer = session.analyzers.get(&language)?;
    let found = analyzer.outgoing_calls(&ctx)?;
    let Some(source_doc) = workspace.get(ctx.path) else {
        return Some(Value::Array(Vec::new()));
    };
    let source_display = source_doc.text.clone();
    let mut out = Vec::with_capacity(found.len());
    for edge in found.iter().take(crate::analyzer::MAX_CALL_HIERARCHY) {
        let Some(to_text) = load_item_text(workspace, &edge.item.path) else {
            continue;
        };
        let Some(to) = call_item_value(&to_text, &edge.item) else {
            continue;
        };
        let mut ranges = Vec::with_capacity(edge.ranges.len());
        for span in &edge.ranges {
            if source_display
                .get(span.start as usize..span.end as usize)
                .is_none()
            {
                continue;
            }
            let range = span_to_range(&source_display, *span);
            ranges.push(serde_json::json!({
                "start": { "line": range.start.line, "character": range.start.character },
                "end": { "line": range.end.line, "character": range.end.character },
            }));
        }
        out.push(serde_json::json!({ "to": to, "fromRanges": ranges }));
    }
    Some(Value::Array(out))
}
