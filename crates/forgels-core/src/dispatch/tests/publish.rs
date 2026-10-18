use super::super::{Session, drain_due, flush_all, handle_message, immediate_notifications};
use super::stub::session_with_stub;
use crate::workspace::Workspace;
use serde_json::Value;

fn open_broken(session: &mut Session, workspace: &mut Workspace) {
    super::stub::init(session, workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/B.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
    assert!(handle_message(session, workspace, open).is_none());
}

#[test]
fn open_publishes_current_diagnostics() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_broken(&mut session, &mut workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/B.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
    let notes = immediate_notifications(&session, &mut workspace, open);
    assert_eq!(notes.len(), 1);
    let value: Value = serde_json::from_str(&notes[0]).expect("frame is json");
    assert_eq!(
        value.pointer("/method").expect("has method"),
        &serde_json::json!("textDocument/publishDiagnostics")
    );
    let diagnostics = value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].pointer("/message").expect("has message"),
        &serde_json::json!("stub broken")
    );
    assert_eq!(
        diagnostics[0].pointer("/code").expect("has code"),
        &serde_json::json!("correctness/parse-error")
    );
    assert_eq!(
        diagnostics[0].pointer("/source").expect("has source"),
        &serde_json::json!("forgels")
    );
}

#[test]
fn close_publishes_empty_and_clears_store() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_broken(&mut session, &mut workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/B.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
    assert_eq!(
        immediate_notifications(&session, &mut workspace, open).len(),
        1
    );
    let close = r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///repo/B.sol"}}}"#;
    let mut session = session_with_stub(true);
    super::stub::init(&mut session, &mut workspace);
    assert!(handle_message(&mut session, &mut workspace, close).is_none());
    let notes = immediate_notifications(&session, &mut workspace, close);
    assert_eq!(notes.len(), 1);
    let value: Value = serde_json::from_str(&notes[0]).expect("frame is json");
    assert!(
        value
            .pointer("/params/diagnostics")
            .expect("has diagnostics")
            .as_array()
            .expect("array")
            .is_empty()
    );
}

#[test]
fn change_publishes_only_when_due() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_broken(&mut session, &mut workspace);
    let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/B.sol","version":2},"contentChanges":[{"text":"broken!"}]}}"#;
    assert!(handle_message(&mut session, &mut workspace, change).is_none());
    let now = std::time::Instant::now();
    assert!(drain_due(&session, &mut workspace, now).is_empty());
    let later = now + std::time::Duration::from_secs(60);
    let notes = drain_due(&session, &mut workspace, later);
    assert_eq!(notes.len(), 1);
}

#[test]
fn shutdown_flushes_pending() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_broken(&mut session, &mut workspace);
    let change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///repo/B.sol","version":2},"contentChanges":[{"text":"broken!"}]}}"#;
    assert!(handle_message(&mut session, &mut workspace, change).is_none());
    let notes = flush_all(&session, &mut workspace);
    assert_eq!(notes.len(), 1);
    assert!(flush_all(&session, &mut workspace).is_empty());
}

#[derive(Debug)]
struct NodeStub;

impl crate::analyzer::Analyzer for NodeStub {
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

fn node_session() -> Session {
    let mut session = Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(NodeStub));
    session
}

fn diagnostics_for(session: &Session, workspace: &mut Workspace, uri: &str) -> Vec<Value> {
    let open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": uri, "languageId": "solidity",
            "version": 1, "text": "import \"pkg/B.sol\"; contract A {}",
        } },
    })
    .to_string();
    let notes = immediate_notifications(session, workspace, &open);
    assert_eq!(notes.len(), 1);
    let value: Value = serde_json::from_str(&notes[0]).expect("frame is json");
    value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array")
        .clone()
}

#[test]
fn node_hit_resolves_no_unresolved() {
    let root = std::env::temp_dir().join("forgels-publish-node-hit");
    let _ = std::fs::remove_dir_all(&root);
    let a = root.join("contracts/A.sol");
    let b = root.join("contracts/node_modules/pkg/B.sol");
    std::fs::create_dir_all(b.parent().expect("parent")).expect("dirs");
    std::fs::write(&b, "contract B {}").expect("write");
    let mut session = node_session();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    let uri = super::super::sync::path_to_uri(&a);
    let open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": uri, "languageId": "solidity",
            "version": 1, "text": "import \"pkg/B.sol\"; contract A {}",
        } },
    })
    .to_string();
    assert!(handle_message(&mut session, &mut workspace, &open).is_none());
    let found = diagnostics_for(&session, &mut workspace, &uri);
    assert!(found.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn truly_missing_reports_unresolved() {
    let root = std::env::temp_dir().join("forgels-publish-node-miss");
    let _ = std::fs::remove_dir_all(&root);
    let a = root.join("contracts/A.sol");
    std::fs::create_dir_all(a.parent().expect("parent")).expect("dirs");
    let mut session = node_session();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    let uri = super::super::sync::path_to_uri(&a);
    let open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": uri, "languageId": "solidity",
            "version": 1, "text": "import \"pkg/B.sol\"; contract A {}",
        } },
    })
    .to_string();
    assert!(handle_message(&mut session, &mut workspace, &open).is_none());
    let found = diagnostics_for(&session, &mut workspace, &uri);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].pointer("/message").expect("has message"),
        &serde_json::json!("unresolved import \"pkg/B.sol\"")
    );
    assert_eq!(
        found[0].pointer("/code").expect("has code"),
        &serde_json::json!("correctness/unresolved-import")
    );
    assert_eq!(
        found[0].pointer("/source").expect("has source"),
        &serde_json::json!("forgels")
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[derive(Debug)]
struct MixedStub;

impl crate::analyzer::Analyzer for MixedStub {
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
        vec![
            crate::analyzer::DiagnosticData::error(
                forgels_syntax::Span::new(0, 1),
                "mixed error".to_string(),
                forgels_lint::RuleId::PARSE_ERROR,
            ),
            crate::analyzer::DiagnosticData::warning(
                forgels_syntax::Span::new(2, 3),
                "mixed warning".to_string(),
                forgels_lint::RuleId::UNUSED,
            ),
        ]
    }
}

#[test]
fn publish_maps_error_and_warning_numbers() {
    let mut session = Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(MixedStub));
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/M.sol","languageId":"solidity","version":1,"text":"ab"}}}"#;
    assert!(handle_message(&mut session, &mut workspace, open).is_none());
    let notes = immediate_notifications(&session, &mut workspace, open);
    assert_eq!(notes.len(), 1);
    let value: Value = serde_json::from_str(&notes[0]).expect("frame is json");
    let diagnostics = value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array");
    assert_eq!(diagnostics.len(), 2);
    let severities: Vec<u64> = diagnostics
        .iter()
        .map(|item| {
            item.pointer("/severity")
                .expect("has severity")
                .as_u64()
                .expect("number")
        })
        .collect();
    assert_eq!(severities, vec![1, 2]);
    assert_eq!(
        diagnostics[0].pointer("/code").expect("has code"),
        &serde_json::json!("correctness/parse-error")
    );
    assert_eq!(
        diagnostics[1].pointer("/code").expect("has code"),
        &serde_json::json!("correctness/unused")
    );
    for item in diagnostics {
        assert_eq!(
            item.pointer("/source").expect("has source"),
            &serde_json::json!("forgels")
        );
    }
}

#[test]
fn off_level_drops_matching_diagnostic() {
    let mut session = session_with_stub(true);
    session.config.lint.rules.insert(
        "correctness/parse-error".to_string(),
        forgels_lint::RuleLevel::Off,
    );
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/O.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
    assert!(handle_message(&mut session, &mut workspace, open).is_none());
    let notes = immediate_notifications(&session, &mut workspace, open);
    assert_eq!(notes.len(), 1);
    let value: Value = serde_json::from_str(&notes[0]).expect("frame is json");
    let diagnostics = value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array");
    assert!(diagnostics.is_empty(), "off drops the diagnostic");
}

#[test]
fn warn_and_err_override_applied() {
    let mut session = Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(MixedStub));
    session.config.lint.rules.insert(
        "correctness/parse-error".to_string(),
        forgels_lint::RuleLevel::Warn,
    );
    session.config.lint.rules.insert(
        "correctness/unused".to_string(),
        forgels_lint::RuleLevel::Err,
    );
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/W.sol","languageId":"solidity","version":1,"text":"ab"}}}"#;
    assert!(handle_message(&mut session, &mut workspace, open).is_none());
    let notes = immediate_notifications(&session, &mut workspace, open);
    assert_eq!(notes.len(), 1);
    let value: Value = serde_json::from_str(&notes[0]).expect("frame is json");
    let diagnostics = value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array");
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].pointer("/severity").expect("severity"),
        &serde_json::json!(2),
        "parse-error downgraded to warn"
    );
    assert_eq!(
        diagnostics[1].pointer("/severity").expect("severity"),
        &serde_json::json!(1),
        "unused upgraded to err"
    );
}

#[test]
fn excluded_file_publishes_empty_and_clears_store() {
    let mut session = session_with_stub(true);
    session.config.lint.excludes = vec!["lib/**".to_string()];
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    workspace.set_roots(vec![std::path::PathBuf::from("/repo")]);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/lib/E.sol","languageId":"solidity","version":1,"text":"broken"}}}"#;
    assert!(handle_message(&mut session, &mut workspace, open).is_none());
    let notes = immediate_notifications(&session, &mut workspace, open);
    assert_eq!(notes.len(), 1);
    let value: Value = serde_json::from_str(&notes[0]).expect("frame is json");
    let diagnostics = value
        .pointer("/params/diagnostics")
        .expect("has diagnostics")
        .as_array()
        .expect("array");
    assert!(diagnostics.is_empty(), "excluded file publishes empty");
    let path = std::path::PathBuf::from("/repo/lib/E.sol");
    assert!(
        workspace.diagnostics_for(&path).is_empty(),
        "no stale diagnostics stored"
    );
}
