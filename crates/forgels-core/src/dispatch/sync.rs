use serde_json::Value;

use forgels_common::paths::resolve_with_remappings;

use super::Workspace;
use crate::analyzer::AnalyzerMap;
use crate::config::Config;
use crate::routing::route_file;

/// Turn a `file://` URI into a path. Plain paths pass through.
/// Percent escapes decode (`%20` becomes a space); malformed escapes
/// stay literal so a weird URI never becomes an empty path.
pub(crate) fn uri_to_path(uri: &str) -> std::path::PathBuf {
    let stripped = uri.strip_prefix("file://").unwrap_or(uri);
    std::path::PathBuf::from(decode_percent(stripped))
}

/// Turn a path into a `file://` URI. Inverse of [`uri_to_path`].
/// Encodes bytes outside `[A-Za-z0-9-_.~/]` as `%XX` so spaces and
/// non-ASCII roundtrip through [`uri_to_path`].
pub(crate) fn path_to_uri(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy();
    let mut out = String::with_capacity(raw.len() + 7);
    out.push_str("file://");
    for byte in raw.as_bytes() {
        if matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/'
        ) {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn decode_percent(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(high) = bytes.get(index + 1).and_then(|byte| hex_value(*byte))
            && let Some(low) = bytes.get(index + 2).and_then(|byte| hex_value(*byte))
        {
            out.push(high * 16 + low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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

/// Root for import resolution: owning workspace root, else config root,
/// else the importing file parent. Always returns a usable base.
pub(crate) fn resolve_root_for(
    workspace: &Workspace,
    config: &Config,
    path: &std::path::Path,
) -> std::path::PathBuf {
    if let Some(root) = workspace.root_for(path) {
        return root.clone();
    }
    if let Some(root) = &config.root {
        return root.clone();
    }
    path.parent()
        .map(|parent| parent.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// Rebuild the index entry for one tracked file from its imports.
/// Every resolved path becomes a dep, even when untracked or disk only.
fn refresh_index(
    workspace: &mut Workspace,
    analyzers: &AnalyzerMap,
    config: &Config,
    path: &std::path::Path,
) {
    let (language, text) = match workspace.get(path) {
        Some(doc) => (doc.language, doc.text.clone()),
        None => return,
    };
    let Some(analyzer) = analyzers.get(&language) else {
        workspace.set_file_deps(path.to_path_buf(), Vec::new());
        return;
    };
    let root = resolve_root_for(workspace, config, path);
    let deps: Vec<std::path::PathBuf> = analyzer
        .imports(&text)
        .iter()
        .filter_map(|item| resolve_with_remappings(path, &item.raw, &root, &config.remappings))
        .collect();
    workspace.set_file_deps(path.to_path_buf(), deps);
}

/// Apply `didOpen`: route, then track. Unknown language or out-of-root
/// files stay untracked without noise.
pub(crate) fn apply_did_open(
    workspace: &mut Workspace,
    analyzers: &AnalyzerMap,
    config: &Config,
    params: &Value,
) {
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
        let version = doc
            .and_then(|doc| doc.get("version"))
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32;
        if workspace
            .open(path.clone(), language, text.to_string(), version)
            .is_ok()
        {
            refresh_index(workspace, analyzers, config, &path);
        }
    }
}

/// Apply `didChange` with full text. Unknown documents stay ignored.
/// Successful changes mark the URI dirty for debounced publish.
pub(crate) fn apply_did_change(
    workspace: &mut Workspace,
    analyzers: &AnalyzerMap,
    config: &Config,
    params: &Value,
) {
    let doc = params.get("textDocument");
    let (Some(uri), Some(changes)) = (
        doc.and_then(|doc| doc.get("uri")).and_then(Value::as_str),
        params.get("contentChanges").and_then(Value::as_array),
    ) else {
        return;
    };
    let version = doc
        .and_then(|doc| doc.get("version"))
        .and_then(Value::as_i64)
        .map(|version| version as i32);
    let path = uri_to_path(uri);
    let current = workspace.get(&path).map(|doc| doc.version).unwrap_or(0);
    if let Some(last) = changes
        .last()
        .and_then(|change| change.get("text"))
        .and_then(Value::as_str)
        && workspace
            .change(&path, last.to_string(), version.unwrap_or(current))
            .is_ok()
    {
        refresh_index(workspace, analyzers, config, &path);
        if let Some(version) = version {
            workspace.mark_dirty(uri, version as u64, std::time::Instant::now());
        }
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
