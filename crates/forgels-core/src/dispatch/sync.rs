use serde_json::Value;

use super::Workspace;
use crate::routing::route_file;

/// Turn a `file://` URI into a path. Plain paths pass through.
/// Percent escapes stay encoded; see the uri-decoding follow-up item.
pub(crate) fn uri_to_path(uri: &str) -> std::path::PathBuf {
    let stripped = uri.strip_prefix("file://").unwrap_or(uri);
    std::path::PathBuf::from(stripped)
}

/// Register roots from initialize params: folders, then rootUri, then rootPath.
pub(crate) fn apply_initialize_roots(workspace: &mut Workspace, params: &Value) {
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
pub(crate) fn apply_did_open(workspace: &mut Workspace, params: &Value) {
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
pub(crate) fn apply_did_change(workspace: &mut Workspace, params: &Value) {
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
pub(crate) fn apply_did_close(workspace: &mut Workspace, params: &Value) {
    if let Some(uri) = params
        .get("textDocument")
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
    {
        let _ = workspace.close(&uri_to_path(uri));
    }
}

/// Apply `workspace/didChangeWorkspaceFolders`: add, remove, evict orphans.
pub(crate) fn apply_folder_change(workspace: &mut Workspace, params: &Value) {
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
