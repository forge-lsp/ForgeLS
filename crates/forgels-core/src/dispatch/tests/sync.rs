use super::super::sync::{path_to_uri, uri_to_path};
use super::super::{Session, handle_message};
use super::stub::init;
use crate::workspace::Workspace;

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
    let due =
        workspace.take_due_publish(std::time::Instant::now() + std::time::Duration::from_secs(60));
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
    let due =
        workspace.take_due_publish(std::time::Instant::now() + std::time::Duration::from_secs(60));
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
    init(&mut session, &mut workspace);
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

#[test]
fn percent_escapes_decode_to_path_chars() {
    assert_eq!(
        uri_to_path("file:///repo/my%20proj/A.sol"),
        std::path::PathBuf::from("/repo/my proj/A.sol")
    );
    let check = char::from_u32(0x2713).expect("check mark");
    assert_eq!(
        uri_to_path("file:///repo/%E2%9C%93.sol"),
        std::path::PathBuf::from(format!("/repo/{check}.sol"))
    );
    assert_eq!(
        uri_to_path("file:///repo/plain.sol"),
        std::path::PathBuf::from("/repo/plain.sol")
    );
    assert_eq!(
        uri_to_path("file:///repo/bad%2G.sol"),
        std::path::PathBuf::from("/repo/bad%2G.sol")
    );
    assert_eq!(
        uri_to_path("file:///repo/tail%.sol"),
        std::path::PathBuf::from("/repo/tail%.sol")
    );
}

#[test]
fn encoded_uri_tracks_decoded_path() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/my%20proj/A.sol","languageId":"solidity","version":1,"text":"contract A {}"}}}"#;
    assert!(handle_message(&mut session, &mut workspace, open).is_none());
    let tracked = workspace
        .get(&std::path::PathBuf::from("/repo/my proj/A.sol"))
        .expect("tracked");
    assert_eq!(tracked.text, "contract A {}");
}

#[test]
fn path_to_uri_roundtrips_through_uri_to_path() {
    for path in ["/repo/A.sol", "/repo/my proj/A.sol", "/repo/plain.sol"] {
        let buf = std::path::PathBuf::from(path);
        let uri = path_to_uri(&buf);
        assert_eq!(uri_to_path(&uri), buf);
    }
    assert_eq!(
        path_to_uri(&std::path::PathBuf::from("/repo/my proj/A.sol")),
        "file:///repo/my%20proj/A.sol"
    );
    let check = char::from_u32(0x2713).expect("check mark");
    let uri = path_to_uri(&std::path::PathBuf::from(format!("/repo/{check}.sol")));
    assert_eq!(
        uri_to_path(&uri),
        std::path::PathBuf::from(format!("/repo/{check}.sol"))
    );
}

#[derive(Debug)]
struct BareStub;

impl crate::analyzer::Analyzer for BareStub {
    fn hover(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<crate::analyzer::HoverData> {
        None
    }

    fn complete(
        &self,
        _ctx: &crate::analyzer::ProjectContext,
    ) -> Vec<crate::analyzer::CompletionData> {
        Vec::new()
    }

    fn goto(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<forgels_syntax::Span> {
        None
    }

    fn diagnostics(&self, _text: &str) -> Vec<crate::analyzer::DiagnosticData> {
        Vec::new()
    }

    fn imports(&self, text: &str) -> Vec<crate::analyzer::ImportRef> {
        if text.contains("pkg/B.sol") {
            vec![crate::analyzer::ImportRef::new("pkg/B.sol".to_string())]
        } else {
            Vec::new()
        }
    }
}

#[test]
fn bare_import_indexes_node_modules_hit() {
    let root = std::env::temp_dir().join("forgels-sync-node");
    let _ = std::fs::remove_dir_all(&root);
    let a = root.join("contracts/A.sol");
    let b = root.join("contracts/node_modules/pkg/B.sol");
    std::fs::create_dir_all(b.parent().expect("parent")).expect("dirs");
    std::fs::write(&b, "contract B {}").expect("write");
    let mut session = Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(BareStub));
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let uri = path_to_uri(&a);
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": uri, "languageId": "solidity",
            "version": 1, "text": "import \"pkg/B.sol\"; contract A {}",
        } },
    })
    .to_string();
    assert!(handle_message(&mut session, &mut workspace, &body).is_none());
    assert_eq!(workspace.index().deps_of(&a), std::slice::from_ref(&b));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_import_missing_stays_unresolved() {
    let root = std::env::temp_dir().join("forgels-sync-node-missing");
    let _ = std::fs::remove_dir_all(&root);
    let a = root.join("contracts/A.sol");
    let mut session = Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(BareStub));
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let uri = path_to_uri(&a);
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": uri, "languageId": "solidity",
            "version": 1, "text": "import \"pkg/B.sol\"; contract A {}",
        } },
    })
    .to_string();
    assert!(handle_message(&mut session, &mut workspace, &body).is_none());
    assert!(workspace.index().deps_of(&a).is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[derive(Debug)]
struct ImportStub;

impl crate::analyzer::Analyzer for ImportStub {
    fn hover(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<crate::analyzer::HoverData> {
        None
    }

    fn complete(
        &self,
        _ctx: &crate::analyzer::ProjectContext,
    ) -> Vec<crate::analyzer::CompletionData> {
        Vec::new()
    }

    fn goto(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<forgels_syntax::Span> {
        None
    }

    fn diagnostics(&self, _text: &str) -> Vec<crate::analyzer::DiagnosticData> {
        Vec::new()
    }

    fn imports(&self, text: &str) -> Vec<crate::analyzer::ImportRef> {
        if text.contains("./B.sol") {
            vec![crate::analyzer::ImportRef::new("./B.sol".to_string())]
        } else {
            Vec::new()
        }
    }
}

fn session_with_imports() -> Session {
    let mut session = Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(ImportStub));
    session
}

fn open_text(
    session: &mut Session,
    workspace: &mut Workspace,
    uri: &str,
    text: &str,
    version: i32,
) {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": uri, "languageId": "solidity",
            "version": version, "text": text,
        } },
    })
    .to_string();
    assert!(handle_message(session, workspace, &body).is_none());
}

fn change_text(
    session: &mut Session,
    workspace: &mut Workspace,
    uri: &str,
    text: &str,
    version: i32,
) {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": uri, "version": version },
            "contentChanges": [{ "text": text }],
        },
    })
    .to_string();
    assert!(handle_message(session, workspace, &body).is_none());
}

fn close_uri(session: &mut Session, workspace: &mut Workspace, uri: &str) {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": { "textDocument": { "uri": uri } },
    })
    .to_string();
    assert!(handle_message(session, workspace, &body).is_none());
}

fn due_uris(workspace: &mut Workspace) -> Vec<(String, u64)> {
    workspace.take_due_publish(std::time::Instant::now() + std::time::Duration::from_secs(60))
}

#[test]
fn change_republishes_direct_dependent() {
    let mut session = session_with_imports();
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "import \"./B.sol\"; contract A {}",
        1,
    );
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/B.sol",
        "contract B {}",
        1,
    );
    change_text(
        &mut session,
        &mut workspace,
        "file:///repo/B.sol",
        "contract B { uint x; }",
        2,
    );
    let mut due = due_uris(&mut workspace);
    due.sort();
    assert_eq!(
        due,
        vec![
            ("file:///repo/A.sol".to_string(), 1),
            ("file:///repo/B.sol".to_string(), 2),
        ]
    );
}

#[test]
fn close_marks_dependent_and_unresolved_returns() {
    let mut session = session_with_imports();
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "import \"./B.sol\"; contract A {}",
        1,
    );
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/B.sol",
        "contract B {}",
        1,
    );
    close_uri(&mut session, &mut workspace, "file:///repo/B.sol");
    let later = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let notes = super::super::drain_due(&session, &mut workspace, later);
    assert_eq!(notes.len(), 1);
    let value: serde_json::Value = serde_json::from_str(&notes[0]).expect("frame is json");
    assert_eq!(
        value.pointer("/params/uri").expect("has uri"),
        &serde_json::json!("file:///repo/A.sol")
    );
    let diagnostics = value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].pointer("/message").expect("has message"),
        &serde_json::json!("unresolved import \"./B.sol\"")
    );
}

#[test]
fn open_resolves_missing_dependent() {
    let mut session = session_with_imports();
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "import \"./B.sol\"; contract A {}",
        1,
    );
    assert_eq!(
        workspace.dependents_to_refresh(&std::path::PathBuf::from("/repo/B.sol")),
        vec![std::path::PathBuf::from("/repo/A.sol")]
    );
    let missing_open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": "file:///repo/A.sol", "languageId": "solidity",
            "version": 1, "text": "import \"./B.sol\"; contract A {}",
        } },
    })
    .to_string();
    let notes = super::super::immediate_notifications(&session, &mut workspace, &missing_open);
    assert_eq!(notes.len(), 1);
    let value: serde_json::Value = serde_json::from_str(&notes[0]).expect("frame is json");
    let before = value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array");
    assert_eq!(before.len(), 1);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/B.sol",
        "contract B {}",
        1,
    );
    let due = due_uris(&mut workspace);
    assert_eq!(due, vec![("file:///repo/A.sol".to_string(), 1)]);
    workspace.mark_dependents_dirty(
        &std::path::PathBuf::from("/repo/B.sol"),
        std::time::Instant::now(),
    );
    let later = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let notes = super::super::drain_due(&session, &mut workspace, later);
    assert_eq!(notes.len(), 1);
    let value: serde_json::Value = serde_json::from_str(&notes[0]).expect("frame is json");
    let after = value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array");
    assert!(after.is_empty());
}

#[test]
fn cycle_and_cap_stay_bounded() {
    let mut session = Session::new();
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    for name in ["/repo/A.sol", "/repo/B.sol"] {
        let uri = format!("file://{name}");
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "solidity",
                "version": 1, "text": "contract C {}",
            } },
        })
        .to_string();
        assert!(handle_message(&mut session, &mut workspace, &body).is_none());
    }
    workspace.set_file_deps(
        std::path::PathBuf::from("/repo/A.sol"),
        vec![std::path::PathBuf::from("/repo/B.sol")],
    );
    workspace.set_file_deps(
        std::path::PathBuf::from("/repo/B.sol"),
        vec![std::path::PathBuf::from("/repo/A.sol")],
    );
    let cycled = workspace.dependents_to_refresh(&std::path::PathBuf::from("/repo/A.sol"));
    assert_eq!(cycled, vec![std::path::PathBuf::from("/repo/B.sol")]);
    let mut big = Workspace::new();
    big.open(
        std::path::PathBuf::from("/repo/base.sol"),
        crate::routing::Language::Solidity,
        "contract Base {}".to_string(),
        1,
    )
    .expect("open works");
    for index in 0..40 {
        let name = format!("/repo/F{index}.sol");
        big.open(
            std::path::PathBuf::from(&name),
            crate::routing::Language::Solidity,
            "contract F {}".to_string(),
            1,
        )
        .expect("open works");
        big.set_file_deps(
            std::path::PathBuf::from(&name),
            vec![std::path::PathBuf::from("/repo/base.sol")],
        );
    }
    let capped = big.dependents_to_refresh(&std::path::PathBuf::from("/repo/base.sol"));
    assert_eq!(capped.len(), crate::workspace::MAX_DEPENDENT_REPUBLISH);
}
