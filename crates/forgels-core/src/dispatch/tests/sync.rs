use super::super::{Session, handle_message};
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
