use super::super::handle_message;
use super::stub::{init, open_sol, session_with_cross, session_with_stub};
use crate::workspace::Workspace;
use forgels_syntax::Span;
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn hover_returns_contents_and_range() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &hover_body(5, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let result = value.get("result").expect("has result");
    assert_eq!(
        result.pointer("/contents/value").expect("has value"),
        &serde_json::json!("stub detail")
    );
    assert_eq!(
        result.pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 0, "character": 13})
    );
    assert_eq!(
        result.pointer("/range/end").expect("has end"),
        &serde_json::json!({"line": 0, "character": 13})
    );
}

#[test]
fn hover_without_declaration_returns_null() {
    let mut session = session_with_stub(false);
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &hover_body(5, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn hover_on_unknown_file_returns_null() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &hover_body(5, "file:///repo/ghost.sol", 0, 1),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn hover_without_id_stays_silent() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/hover","params":{"textDocument":{"uri":"file:///repo/A.sol"},"position":{"line":0,"character":10}}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

fn hover_body(id: u32, uri: &str, line: u32, character: u32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        }
    })
    .to_string()
}

fn completion_body(id: u32, uri: &str, line: u32, character: u32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        }
    })
    .to_string()
}

#[test]
fn completion_lists_matching_labels() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &completion_body(6, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let result = value.get("result").expect("has result");
    assert_eq!(
        result.pointer("/isIncomplete").expect("has flag"),
        &serde_json::json!(false)
    );
    let items = result
        .pointer("/items")
        .expect("has items")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].pointer("/label").expect("has label"),
        &serde_json::json!("stub_one")
    );
    assert_eq!(
        items[0].pointer("/kind").expect("has kind"),
        &serde_json::json!(3)
    );
    assert_eq!(
        items[1].pointer("/kind").expect("has kind"),
        &serde_json::json!(6)
    );
    assert!(items[0].get("textEdit").is_none());
    assert!(items[1].get("textEdit").is_none());
}

#[test]
fn completion_on_unknown_file_returns_empty_list() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &completion_body(6, "file:///repo/ghost.sol", 0, 1),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result/items")
        .expect("has items")
        .as_array()
        .expect("array");
    assert!(items.is_empty());
}

fn definition_body(id: u32, uri: &str, line: u32, character: u32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        }
    })
    .to_string()
}

#[test]
fn definition_returns_uri_and_range() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &definition_body(7, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let result = value.get("result").expect("has result");
    assert_eq!(
        result.pointer("/uri").expect("has uri"),
        &serde_json::json!("file:///repo/A.sol")
    );
    assert_eq!(
        result.pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 0, "character": 9})
    );
    assert_eq!(
        result.pointer("/range/end").expect("has end"),
        &serde_json::json!({"line": 0, "character": 10})
    );
}

#[test]
fn definition_on_unknown_file_returns_null() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &definition_body(7, "file:///repo/ghost.sol", 0, 1),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

fn open_pair(session: &mut super::super::Session, workspace: &mut Workspace, b_text: &str) {
    init(session, workspace);
    for (uri, text) in [
        ("file:///repo/A.sol", "contract A {}"),
        ("file:///repo/B.sol", b_text),
    ] {
        let open = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "solidity", "version": 1, "text": text,
            } },
        })
        .to_string();
        assert!(handle_message(session, workspace, &open).is_none());
    }
}

#[test]
fn definition_returns_cross_file_uri_and_range() {
    let mut session = session_with_cross(PathBuf::from("/repo/B.sol"), Span::new(16, 17));
    let mut workspace = Workspace::new();
    open_pair(&mut session, &mut workspace, "header\ncontract B {}");
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &definition_body(7, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let result = value.get("result").expect("has result");
    assert_eq!(
        result.pointer("/uri").expect("has uri"),
        &serde_json::json!("file:///repo/B.sol")
    );
    assert_eq!(
        result.pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 1, "character": 9})
    );
    assert_eq!(
        result.pointer("/range/end").expect("has end"),
        &serde_json::json!({"line": 1, "character": 10})
    );
}

#[test]
fn definition_with_untracked_target_returns_null() {
    let mut session = session_with_cross(PathBuf::from("/repo/Missing.sol"), Span::new(9, 10));
    let mut workspace = Workspace::new();
    open_pair(&mut session, &mut workspace, "contract B {}");
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &definition_body(7, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn definition_with_invalid_span_returns_null() {
    let mut session = session_with_cross(PathBuf::from("/repo/B.sol"), Span::new(500, 510));
    let mut workspace = Workspace::new();
    open_pair(&mut session, &mut workspace, "contract B {}");
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &definition_body(7, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn definition_reads_disk_only_target() {
    let root = std::env::temp_dir().join("forgels-goto-disk");
    let _ = std::fs::remove_dir_all(&root);
    let a = root.join("contracts/A.sol");
    let b = root.join("contracts/node_modules/pkg/B.sol");
    std::fs::create_dir_all(b.parent().expect("parent")).expect("dirs");
    std::fs::write(&b, "contract B {}").expect("write");
    let mut session = session_with_cross(b.clone(), Span::new(9, 10));
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let uri = super::super::sync::path_to_uri(&a);
    let open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": uri, "languageId": "solidity", "version": 1, "text": "contract A {}",
        } },
    })
    .to_string();
    assert!(handle_message(&mut session, &mut workspace, &open).is_none());
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &definition_body(7, &uri, 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let result = value.get("result").expect("has result");
    assert_eq!(
        result.pointer("/uri").expect("has uri"),
        &serde_json::json!(super::super::sync::path_to_uri(&b))
    );
    assert_eq!(
        result.pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 0, "character": 9})
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn hover_reads_current_file_through_context() {
    use crate::analyzer::{Analyzer, ProjectContext};

    #[derive(Debug)]
    struct EchoAnalyzer;

    impl Analyzer for EchoAnalyzer {
        fn hover(&self, ctx: &ProjectContext) -> Option<crate::analyzer::HoverData> {
            Some(crate::analyzer::HoverData {
                contents: ctx.text.to_string(),
                span: forgels_syntax::Span::new(0, 0),
            })
        }

        fn complete(&self, _ctx: &ProjectContext) -> Vec<crate::analyzer::CompletionData> {
            Vec::new()
        }

        fn goto(&self, _ctx: &ProjectContext) -> Option<forgels_syntax::Span> {
            None
        }

        fn diagnostics(&self, _text: &str) -> Vec<crate::analyzer::DiagnosticData> {
            Vec::new()
        }
    }

    let mut session = super::super::Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(EchoAnalyzer));
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    for (uri, text) in [
        ("file:///repo/A.sol", "contract A {}"),
        ("file:///repo/B.sol", "contract B {}"),
    ] {
        let open = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "solidity", "version": 1, "text": text,
            } },
        })
        .to_string();
        assert!(handle_message(&mut session, &mut workspace, &open).is_none());
    }
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &hover_body(8, "file:///repo/B.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert_eq!(
        value.pointer("/result/contents/value").expect("has value"),
        &serde_json::json!("contract B {}")
    );
}

#[test]
fn completion_lists_dep_symbol_with_origin() {
    use crate::analyzer::{Analyzer, ProjectContext, dep_texts};

    #[derive(Debug)]
    struct DepAnalyzer;

    fn prefix_before(text: &str, offset: u32) -> String {
        let end = (offset as usize).min(text.len());
        let bytes = text.as_bytes();
        let mut start = end;
        while start > 0 {
            let byte = bytes[start - 1];
            if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' {
                start -= 1;
            } else {
                break;
            }
        }
        text.get(start..end).unwrap_or("").to_string()
    }

    impl Analyzer for DepAnalyzer {
        fn hover(&self, _ctx: &ProjectContext) -> Option<crate::analyzer::HoverData> {
            None
        }

        fn complete(&self, ctx: &ProjectContext) -> Vec<crate::analyzer::CompletionData> {
            let prefix = prefix_before(ctx.text, ctx.offset);
            let mut out = Vec::new();
            for (dep_path, dep_text) in dep_texts(ctx, 16) {
                let Some(name) = dep_path.file_name().and_then(|item| item.to_str()) else {
                    continue;
                };
                if dep_text.contains("dep_fn") && "dep_fn".starts_with(prefix.as_str()) {
                    out.push(crate::analyzer::CompletionData {
                        label: "dep_fn".to_string(),
                        detail: format!("function dep_fn() - from {name}"),
                        kind: forgels_protocol::CompletionItemKind::FUNCTION,
                        text_edit: None,
                    });
                }
            }
            out
        }

        fn goto(&self, _ctx: &ProjectContext) -> Option<forgels_syntax::Span> {
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

    let mut session = super::super::Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(DepAnalyzer));
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    for (uri, text) in [
        (
            "file:///repo/B.sol",
            "contract B {\nfunction dep_fn() public {}\n}",
        ),
        (
            "file:///repo/A.sol",
            "import \"./B.sol\";\ncontract A {\nfunction f() public {\ndep\n}\n}",
        ),
    ] {
        let open = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "solidity", "version": 1, "text": text,
            } },
        })
        .to_string();
        assert!(handle_message(&mut session, &mut workspace, &open).is_none());
    }
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &completion_body(9, "file:///repo/A.sol", 3, 3),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result/items")
        .expect("has items")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/label").expect("has label"),
        &serde_json::json!("dep_fn")
    );
    assert_eq!(
        items[0].pointer("/detail").expect("has detail"),
        &serde_json::json!("function dep_fn() - from B.sol")
    );
    assert_eq!(
        items[0].pointer("/kind").expect("has kind"),
        &serde_json::json!(3)
    );
}

#[test]
fn completion_emits_text_edit_for_path() {
    use crate::analyzer::{Analyzer, ProjectContext, TextEdit};

    #[derive(Debug)]
    struct EditStub;

    impl Analyzer for EditStub {
        fn hover(&self, _ctx: &ProjectContext) -> Option<crate::analyzer::HoverData> {
            None
        }

        fn complete(&self, _ctx: &ProjectContext) -> Vec<crate::analyzer::CompletionData> {
            vec![crate::analyzer::CompletionData {
                label: "./B.sol".to_string(),
                detail: "file".to_string(),
                kind: forgels_protocol::CompletionItemKind::FILE,
                text_edit: Some(TextEdit::new(Span::new(8, 11), "./B.sol".to_string())),
            }]
        }

        fn goto(&self, _ctx: &ProjectContext) -> Option<Span> {
            None
        }

        fn diagnostics(&self, _text: &str) -> Vec<crate::analyzer::DiagnosticData> {
            Vec::new()
        }
    }

    let mut session = super::super::Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(EditStub));
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": "file:///repo/A.sol",
            "languageId": "solidity",
            "version": 1,
            "text": "import \"./B\";",
        } },
    })
    .to_string();
    assert!(handle_message(&mut session, &mut workspace, &open).is_none());
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &completion_body(10, "file:///repo/A.sol", 0, 11),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result/items")
        .expect("has items")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/textEdit/newText").expect("has edit"),
        &serde_json::json!("./B.sol")
    );
    assert_eq!(
        items[0]
            .pointer("/textEdit/range/start")
            .expect("has start"),
        &serde_json::json!({"line": 0, "character": 8})
    );
    assert_eq!(
        items[0].pointer("/textEdit/range/end").expect("has end"),
        &serde_json::json!({"line": 0, "character": 11})
    );
}

#[test]
fn completion_skips_out_of_bounds_edit() {
    use crate::analyzer::{Analyzer, ProjectContext, TextEdit};

    #[derive(Debug)]
    struct BadStub;

    impl Analyzer for BadStub {
        fn hover(&self, _ctx: &ProjectContext) -> Option<crate::analyzer::HoverData> {
            None
        }

        fn complete(&self, _ctx: &ProjectContext) -> Vec<crate::analyzer::CompletionData> {
            vec![crate::analyzer::CompletionData {
                label: "./B.sol".to_string(),
                detail: "file".to_string(),
                kind: forgels_protocol::CompletionItemKind::FILE,
                text_edit: Some(TextEdit::new(Span::new(500, 510), "./B.sol".to_string())),
            }]
        }

        fn goto(&self, _ctx: &ProjectContext) -> Option<Span> {
            None
        }

        fn diagnostics(&self, _text: &str) -> Vec<crate::analyzer::DiagnosticData> {
            Vec::new()
        }
    }

    let mut session = super::super::Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(BadStub));
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": "file:///repo/A.sol",
            "languageId": "solidity",
            "version": 1,
            "text": "import \"./B\";",
        } },
    })
    .to_string();
    assert!(handle_message(&mut session, &mut workspace, &open).is_none());
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &completion_body(11, "file:///repo/A.sol", 0, 11),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result/items")
        .expect("has items")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert!(items[0].get("textEdit").is_none());
}

fn signature_body(id: u32, uri: &str, line: u32, character: u32) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/signatureHelp",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        }
    })
    .to_string()
}

#[derive(Debug)]
struct SigAnalyzer;

impl crate::analyzer::Analyzer for SigAnalyzer {
    fn hover(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<crate::analyzer::HoverData> {
        None
    }

    fn complete(
        &self,
        _ctx: &crate::analyzer::ProjectContext,
    ) -> Vec<crate::analyzer::CompletionData> {
        Vec::new()
    }

    fn goto(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<Span> {
        None
    }

    fn diagnostics(&self, _text: &str) -> Vec<crate::analyzer::DiagnosticData> {
        Vec::new()
    }

    fn signature_help(
        &self,
        _ctx: &crate::analyzer::ProjectContext,
    ) -> Option<Vec<crate::analyzer::SignatureData>> {
        Some(vec![
            crate::analyzer::SignatureData::new(
                "function greet(uint a)".to_string(),
                vec![crate::analyzer::SignatureParam::new("uint a".to_string())],
                1,
            ),
            crate::analyzer::SignatureData::new(
                "function greet(uint a, uint b)".to_string(),
                vec![
                    crate::analyzer::SignatureParam::new("uint a".to_string()),
                    crate::analyzer::SignatureParam::new("uint b".to_string()),
                ],
                1,
            ),
        ])
    }
}

fn session_with_sig() -> super::super::Session {
    let mut session = super::super::Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(SigAnalyzer));
    session
}

#[test]
fn signature_returns_labels_and_active() {
    let mut session = session_with_sig();
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &signature_body(12, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let result = value.get("result").expect("has result");
    assert_eq!(
        result.pointer("/activeSignature").expect("has signature"),
        &serde_json::json!(0)
    );
    assert_eq!(
        result.pointer("/activeParameter").expect("has parameter"),
        &serde_json::json!(1)
    );
    let signatures = result
        .pointer("/signatures")
        .expect("has signatures")
        .as_array()
        .expect("array");
    assert_eq!(signatures.len(), 2);
    assert_eq!(
        signatures[0].pointer("/label").expect("has label"),
        &serde_json::json!("function greet(uint a)")
    );
    assert_eq!(
        signatures[0]
            .pointer("/parameters/0/label")
            .expect("has param"),
        &serde_json::json!("uint a")
    );
    assert_eq!(
        signatures[1].pointer("/label").expect("has label"),
        &serde_json::json!("function greet(uint a, uint b)")
    );
}

#[test]
fn signature_without_help_returns_null() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &signature_body(12, "file:///repo/A.sol", 0, 10),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn signature_on_unknown_file_returns_null() {
    let mut session = session_with_sig();
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &signature_body(12, "file:///repo/ghost.sol", 0, 1),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn signature_without_id_stays_silent() {
    let mut session = session_with_sig();
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/signatureHelp","params":{"textDocument":{"uri":"file:///repo/A.sol"},"position":{"line":0,"character":10}}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

fn symbol_body(id: u32, uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentSymbol",
        "params": { "textDocument": { "uri": uri } },
    })
    .to_string()
}

#[derive(Debug)]
struct SymbolAnalyzer;

impl crate::analyzer::Analyzer for SymbolAnalyzer {
    fn hover(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<crate::analyzer::HoverData> {
        None
    }

    fn complete(
        &self,
        _ctx: &crate::analyzer::ProjectContext,
    ) -> Vec<crate::analyzer::CompletionData> {
        Vec::new()
    }

    fn goto(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<Span> {
        None
    }

    fn diagnostics(&self, _text: &str) -> Vec<crate::analyzer::DiagnosticData> {
        Vec::new()
    }

    fn document_symbols(&self, _text: &str) -> Vec<crate::analyzer::DocumentSymbolData> {
        vec![crate::analyzer::DocumentSymbolData::new(
            "C".to_string(),
            "contract C".to_string(),
            forgels_protocol::SymbolKind::CLASS,
            Span::new(0, 13),
            Span::new(9, 10),
            vec![crate::analyzer::DocumentSymbolData::new(
                "f".to_string(),
                "function f()".to_string(),
                forgels_protocol::SymbolKind::METHOD,
                Span::new(0, 13),
                Span::new(9, 10),
                Vec::new(),
            )],
        )]
    }
}

fn session_with_symbols() -> super::super::Session {
    let mut session = super::super::Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(SymbolAnalyzer));
    session
}

#[test]
fn document_symbol_returns_nesting_and_selection() {
    let mut session = session_with_symbols();
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &symbol_body(13, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/name").expect("has name"),
        &serde_json::json!("C")
    );
    assert_eq!(
        items[0].pointer("/kind").expect("has kind"),
        &serde_json::json!(5)
    );
    assert_eq!(
        items[0].pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 0, "character": 0})
    );
    assert_eq!(
        items[0]
            .pointer("/selectionRange/start")
            .expect("has selection"),
        &serde_json::json!({"line": 0, "character": 9})
    );
    assert_eq!(
        items[0]
            .pointer("/selectionRange/end")
            .expect("has selection end"),
        &serde_json::json!({"line": 0, "character": 10})
    );
    let children = items[0]
        .pointer("/children")
        .expect("has children")
        .as_array()
        .expect("array");
    assert_eq!(children.len(), 1);
    assert_eq!(
        children[0].pointer("/kind").expect("has kind"),
        &serde_json::json!(6)
    );
}

#[test]
fn document_symbol_on_unknown_file_returns_null() {
    let mut session = session_with_symbols();
    let mut workspace = Workspace::new();
    init(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &symbol_body(13, "file:///repo/ghost.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn document_symbol_without_id_stays_silent() {
    let mut session = session_with_symbols();
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///repo/A.sol"}}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

#[test]
fn document_symbol_default_is_empty_list() {
    let mut session = session_with_stub(true);
    let mut workspace = Workspace::new();
    open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &symbol_body(14, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert!(items.is_empty());
}

fn workspace_body(id: u32, query: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "workspace/symbol",
        "params": { "query": query },
    })
    .to_string()
}

fn open_text(
    session: &mut super::super::Session,
    workspace: &mut Workspace,
    uri: &str,
    text: &str,
) {
    let open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": uri, "languageId": "solidity", "version": 1, "text": text,
        } },
    })
    .to_string();
    assert!(handle_message(session, workspace, &open).is_none());
}

#[derive(Debug)]
struct WsAnalyzer;

fn ws_symbol(
    text: &str,
    name: &str,
    kind: forgels_protocol::SymbolKind,
    children: Vec<crate::analyzer::DocumentSymbolData>,
) -> Option<crate::analyzer::DocumentSymbolData> {
    let start = text.find(name)? as u32;
    let end = start + name.len() as u32;
    Some(crate::analyzer::DocumentSymbolData::new(
        name.to_string(),
        String::new(),
        kind,
        Span::new(0, text.len() as u32),
        Span::new(start, end),
        children,
    ))
}

impl crate::analyzer::Analyzer for WsAnalyzer {
    fn hover(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<crate::analyzer::HoverData> {
        None
    }

    fn complete(
        &self,
        _ctx: &crate::analyzer::ProjectContext,
    ) -> Vec<crate::analyzer::CompletionData> {
        Vec::new()
    }

    fn goto(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<Span> {
        None
    }

    fn diagnostics(&self, _text: &str) -> Vec<crate::analyzer::DiagnosticData> {
        Vec::new()
    }

    fn document_symbols(&self, text: &str) -> Vec<crate::analyzer::DocumentSymbolData> {
        if text.starts_with("many") {
            let mut out = Vec::new();
            for index in 0..60 {
                let name = format!("Sym{index:02}");
                out.push(crate::analyzer::DocumentSymbolData::new(
                    name,
                    String::new(),
                    forgels_protocol::SymbolKind::CLASS,
                    Span::new(0, 4),
                    Span::new(0, 4),
                    Vec::new(),
                ));
            }
            return out;
        }
        if text.contains("Bad") {
            let mut out = Vec::new();
            if let Some(item) = ws_symbol(
                text,
                "GoodOne",
                forgels_protocol::SymbolKind::CLASS,
                Vec::new(),
            ) {
                out.push(item);
            }
            out.push(crate::analyzer::DocumentSymbolData::new(
                "GoodBad".to_string(),
                String::new(),
                forgels_protocol::SymbolKind::CLASS,
                Span::new(500, 510),
                Span::new(500, 510),
                Vec::new(),
            ));
            return out;
        }
        let mut out = Vec::new();
        if text.contains("Token")
            && let Some(child) = ws_symbol(
                text,
                "mint",
                forgels_protocol::SymbolKind::METHOD,
                Vec::new(),
            )
            && let Some(top) = ws_symbol(
                text,
                "Token",
                forgels_protocol::SymbolKind::CLASS,
                vec![child],
            )
        {
            out.push(top);
        }
        if text.contains("Vault")
            && let Some(child) = ws_symbol(
                text,
                "deposit",
                forgels_protocol::SymbolKind::METHOD,
                vec![crate::analyzer::DocumentSymbolData::new(
                    "inner".to_string(),
                    String::new(),
                    forgels_protocol::SymbolKind::FUNCTION,
                    Span::new(0, 4),
                    Span::new(0, 4),
                    Vec::new(),
                )],
            )
            && let Some(top) = ws_symbol(
                text,
                "Vault",
                forgels_protocol::SymbolKind::CLASS,
                vec![child],
            )
        {
            out.push(top);
        }
        if text.contains("Alpha")
            && let Some(item) = ws_symbol(
                text,
                "Alpha",
                forgels_protocol::SymbolKind::CLASS,
                Vec::new(),
            )
        {
            out.push(item);
        }
        out
    }
}

fn session_with_ws() -> super::super::Session {
    let mut session = super::super::Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(WsAnalyzer));
    session
}

fn workspace_items(reply: &str) -> Vec<Value> {
    let value: Value = serde_json::from_str(reply).expect("reply is json");
    value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array")
        .clone()
}

#[test]
fn workspace_symbol_empty_query_returns_empty() {
    let mut session = session_with_ws();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "contract Token { function mint() {} }",
    );
    let reply =
        handle_message(&mut session, &mut workspace, &workspace_body(20, "")).expect("must reply");
    assert!(workspace_items(&reply).is_empty());
}

#[test]
fn workspace_symbol_matches_case_insensitive() {
    let mut session = session_with_ws();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "contract Token { function mint() {} }",
    );
    let reply = handle_message(&mut session, &mut workspace, &workspace_body(21, "token"))
        .expect("must reply");
    let items = workspace_items(&reply);
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/name").expect("has name"),
        &serde_json::json!("Token")
    );
    assert_eq!(
        items[0].pointer("/kind").expect("has kind"),
        &serde_json::json!(5)
    );
    assert_eq!(
        items[0].pointer("/location/uri").expect("has uri"),
        &serde_json::json!("file:///repo/A.sol")
    );
}

#[test]
fn workspace_symbol_flattens_child_with_container() {
    let mut session = session_with_ws();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "contract Vault { function deposit() {} }",
    );
    let reply = handle_message(&mut session, &mut workspace, &workspace_body(22, "deposit"))
        .expect("must reply");
    let items = workspace_items(&reply);
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/name").expect("has name"),
        &serde_json::json!("deposit")
    );
    assert_eq!(
        items[0].pointer("/containerName").expect("has container"),
        &serde_json::json!("Vault")
    );
    let top_reply = handle_message(&mut session, &mut workspace, &workspace_body(23, "vault"))
        .expect("must reply");
    let top_items = workspace_items(&top_reply);
    assert_eq!(top_items.len(), 1);
    assert!(top_items[0].get("containerName").is_none());
}

#[test]
fn workspace_symbol_skips_grandchild_beyond_depth_one() {
    let mut session = session_with_ws();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "contract Vault { function deposit() {} }",
    );
    let reply = handle_message(&mut session, &mut workspace, &workspace_body(24, "inner"))
        .expect("must reply");
    assert!(workspace_items(&reply).is_empty());
}

#[test]
fn workspace_symbol_sorts_by_path() {
    let mut session = session_with_ws();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/B.sol",
        "contract Alpha {}",
    );
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "contract Alpha {}",
    );
    let reply = handle_message(&mut session, &mut workspace, &workspace_body(25, "alpha"))
        .expect("must reply");
    let items = workspace_items(&reply);
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].pointer("/location/uri").expect("has uri"),
        &serde_json::json!("file:///repo/A.sol")
    );
    assert_eq!(
        items[1].pointer("/location/uri").expect("has uri"),
        &serde_json::json!("file:///repo/B.sol")
    );
}

#[test]
fn workspace_symbol_caps_at_fifty() {
    let mut session = session_with_ws();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(&mut session, &mut workspace, "file:///repo/A.sol", "many");
    let reply = handle_message(&mut session, &mut workspace, &workspace_body(26, "sym"))
        .expect("must reply");
    assert_eq!(workspace_items(&reply).len(), 50);
}

#[test]
fn workspace_symbol_skips_bad_span() {
    let mut session = session_with_ws();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "contract Bad { contract GoodOne {} }",
    );
    let reply = handle_message(&mut session, &mut workspace, &workspace_body(27, "good"))
        .expect("must reply");
    let items = workspace_items(&reply);
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/name").expect("has name"),
        &serde_json::json!("GoodOne")
    );
}

#[test]
fn workspace_symbol_without_id_stays_silent() {
    let mut session = session_with_ws();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "contract Token { function mint() {} }",
    );
    let body = r#"{"jsonrpc":"2.0","method":"workspace/symbol","params":{"query":"token"}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

#[test]
fn workspace_symbol_missing_query_returns_empty() {
    let mut session = session_with_ws();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "contract Token { function mint() {} }",
    );
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 28,
        "method": "workspace/symbol",
        "params": {},
    })
    .to_string();
    let reply = handle_message(&mut session, &mut workspace, &body).expect("must reply");
    assert!(workspace_items(&reply).is_empty());
}

fn references_body(id: u32, uri: &str, line: u32, character: u32, include_decl: bool) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/references",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
            "context": { "includeDeclaration": include_decl },
        }
    })
    .to_string()
}

#[derive(Debug)]
struct RefAnalyzer;

impl crate::analyzer::Analyzer for RefAnalyzer {
    fn hover(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<crate::analyzer::HoverData> {
        None
    }

    fn complete(
        &self,
        _ctx: &crate::analyzer::ProjectContext,
    ) -> Vec<crate::analyzer::CompletionData> {
        Vec::new()
    }

    fn goto(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<Span> {
        None
    }

    fn diagnostics(&self, _text: &str) -> Vec<crate::analyzer::DiagnosticData> {
        Vec::new()
    }

    fn references(
        &self,
        ctx: &crate::analyzer::ProjectContext,
    ) -> Option<Vec<crate::analyzer::ReferenceData>> {
        Some(vec![
            crate::analyzer::ReferenceData::new(ctx.path.to_path_buf(), Span::new(9, 10), true),
            crate::analyzer::ReferenceData::new(ctx.path.to_path_buf(), Span::new(0, 1), false),
            crate::analyzer::ReferenceData::new(
                std::path::PathBuf::from("/repo/B.sol"),
                Span::new(500, 510),
                false,
            ),
        ])
    }
}

fn session_with_refs() -> super::super::Session {
    let mut session = super::super::Session::new();
    session
        .analyzers
        .insert(crate::routing::Language::Solidity, Box::new(RefAnalyzer));
    session
}

fn reference_items(reply: &str) -> Vec<Value> {
    let value: Value = serde_json::from_str(reply).expect("reply is json");
    value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array")
        .clone()
}

#[test]
fn references_default_is_empty_list() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &references_body(30, "file:///repo/A.sol", 0, 10, true),
    )
    .expect("must reply");
    assert!(reference_items(&reply).is_empty());
}

#[test]
fn references_includes_decl_when_requested() {
    let mut session = session_with_refs();
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &references_body(31, "file:///repo/A.sol", 0, 10, true),
    )
    .expect("must reply");
    let items = reference_items(&reply);
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].pointer("/uri").expect("has uri"),
        &serde_json::json!("file:///repo/A.sol")
    );
    assert_eq!(
        items[0].pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 0, "character": 9})
    );
}

#[test]
fn references_filters_decl_when_excluded() {
    let mut session = session_with_refs();
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &references_body(32, "file:///repo/A.sol", 0, 10, false),
    )
    .expect("must reply");
    let items = reference_items(&reply);
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 0, "character": 0})
    );
}

#[test]
fn references_on_unknown_file_returns_empty() {
    let mut session = session_with_refs();
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &references_body(33, "file:///repo/ghost.sol", 0, 1, true),
    )
    .expect("must reply");
    assert!(reference_items(&reply).is_empty());
}

#[test]
fn references_without_id_stays_silent() {
    let mut session = session_with_refs();
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/references","params":{"textDocument":{"uri":"file:///repo/A.sol"},"position":{"line":0,"character":10},"context":{"includeDeclaration":true}}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

fn rename_body(id: u32, uri: &str, line: u32, character: u32, new_name: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/rename",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
            "newName": new_name,
        }
    })
    .to_string()
}

#[test]
fn rename_returns_changes_map_with_new_text() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &rename_body(40, "file:///repo/A.sol", 0, 10, "total"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let changes = value.pointer("/result/changes").expect("has changes");
    let edits = changes
        .get("file:///repo/A.sol")
        .expect("has file edits")
        .as_array()
        .expect("array");
    assert_eq!(edits.len(), 2);
    for edit in edits {
        assert_eq!(
            edit.pointer("/newText").expect("has text"),
            &serde_json::json!("total")
        );
    }
    assert_eq!(
        edits[0].pointer("/range/start").expect("has start"),
        &serde_json::json!({"line": 0, "character": 9})
    );
}

#[test]
fn rename_without_symbol_returns_invalid_params() {
    let mut session = super::stub::session_with_stub(false);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &rename_body(41, "file:///repo/A.sol", 0, 10, "total"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert_eq!(
        value.pointer("/error/code").expect("has code"),
        &serde_json::json!(-32602)
    );
    assert!(value.get("result").is_none());
}

#[test]
fn rename_with_empty_name_returns_invalid_params() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &rename_body(42, "file:///repo/A.sol", 0, 10, ""),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert_eq!(
        value.pointer("/error/code").expect("has code"),
        &serde_json::json!(-32602)
    );
}

#[test]
fn rename_on_unknown_file_returns_invalid_params() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &rename_body(43, "file:///repo/ghost.sol", 0, 1, "total"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert_eq!(
        value.pointer("/error/code").expect("has code"),
        &serde_json::json!(-32602)
    );
}

#[test]
fn rename_default_returns_invalid_params() {
    let mut session = session_with_refs();
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &rename_body(44, "file:///repo/A.sol", 0, 10, "total"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert_eq!(
        value.pointer("/error/code").expect("has code"),
        &serde_json::json!(-32602)
    );
}

#[test]
fn rename_without_id_stays_silent() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/rename","params":{"textDocument":{"uri":"file:///repo/A.sol"},"position":{"line":0,"character":10},"newName":"total"}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

fn inlay_body(id: u32, uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/inlayHint",
        "params": {
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 13 },
            },
        }
    })
    .to_string()
}

#[test]
fn inlay_returns_label_and_kind() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &inlay_body(50, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/label").expect("has label"),
        &serde_json::json!("a:")
    );
    assert_eq!(
        items[0].pointer("/kind").expect("has kind"),
        &serde_json::json!(2)
    );
}

#[test]
fn inlay_on_unknown_file_returns_empty() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &inlay_body(51, "file:///repo/ghost.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(
        value
            .pointer("/result")
            .expect("has result")
            .as_array()
            .expect("array")
            .is_empty()
    );
}

#[test]
fn inlay_without_id_stays_silent() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/inlayHint","params":{"textDocument":{"uri":"file:///repo/A.sol"},"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":13}}}}"#;
    assert!(handle_message(&mut session, &mut workspace, body).is_none());
}

fn semantic_body(id: u32, uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/semanticTokens/full",
        "params": { "textDocument": { "uri": uri } },
    })
    .to_string()
}

#[test]
fn semantic_returns_delta_data() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &semantic_body(52, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let data = value
        .pointer("/result/data")
        .expect("has data")
        .as_array()
        .expect("array");
    assert_eq!(data.len(), 5);
    assert_eq!(data[2], serde_json::json!(8));
    assert_eq!(data[3], serde_json::json!(14));
}

#[test]
fn semantic_on_unknown_file_returns_empty_data() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &semantic_body(53, "file:///repo/ghost.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(
        value
            .pointer("/result/data")
            .expect("has data")
            .as_array()
            .expect("array")
            .is_empty()
    );
}

fn prepare_body(id: u32, uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/prepareCallHierarchy",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 10 },
        }
    })
    .to_string()
}

#[test]
fn prepare_returns_one_item() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &prepare_body(54, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/name").expect("has name"),
        &serde_json::json!("stub_fn")
    );
    assert_eq!(
        items[0].pointer("/uri").expect("has uri"),
        &serde_json::json!("file:///repo/A.sol")
    );
}

#[test]
fn prepare_without_symbol_returns_null() {
    let mut session = super::stub::session_with_stub(false);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &prepare_body(55, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

fn call_body(id: u32, method: &str, item: Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": { "item": item },
    })
    .to_string()
}

fn stub_item() -> Value {
    serde_json::json!({
        "name": "stub_fn",
        "kind": 6,
        "detail": "function stub_fn()",
        "uri": "file:///repo/A.sol",
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 12 },
        },
        "selectionRange": {
            "start": { "line": 0, "character": 9 },
            "end": { "line": 0, "character": 10 },
        },
    })
}

#[test]
fn incoming_returns_from_with_ranges() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &call_body(56, "callHierarchy/incomingCalls", stub_item()),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/from/name").expect("has name"),
        &serde_json::json!("caller")
    );
    assert_eq!(
        items[0]
            .pointer("/fromRanges")
            .expect("has ranges")
            .as_array()
            .expect("array")
            .len(),
        1
    );
}

#[test]
fn outgoing_returns_to_with_ranges() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &call_body(57, "callHierarchy/outgoingCalls", stub_item()),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/to/name").expect("has name"),
        &serde_json::json!("callee")
    );
}

#[test]
fn incoming_default_returns_empty() {
    let mut session = super::stub::session_with_stub(false);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &call_body(58, "callHierarchy/incomingCalls", stub_item()),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(
        value
            .pointer("/result")
            .expect("has result")
            .as_array()
            .expect("array")
            .is_empty()
    );
}

fn code_action_body(id: u32, uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/codeAction",
        "params": {
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 1 },
            },
            "context": { "diagnostics": [] },
        }
    })
    .to_string()
}

#[test]
fn code_action_returns_quickfix_with_edit() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &code_action_body(60, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/title").expect("has title"),
        &serde_json::json!("stub fix")
    );
    assert_eq!(
        items[0].pointer("/kind").expect("has kind"),
        &serde_json::json!("quickfix")
    );
    let changes = items[0].pointer("/edit/changes").expect("has changes");
    let edits = changes
        .get("file:///repo/A.sol")
        .expect("has file edits")
        .as_array()
        .expect("array");
    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].pointer("/newText").expect("has edit"),
        &serde_json::json!("fixed")
    );
    assert!(items[0].get("command").is_none());
}

#[test]
fn code_action_default_returns_empty() {
    let mut session = super::stub::session_with_stub(false);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &code_action_body(61, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(
        value
            .pointer("/result")
            .expect("has result")
            .as_array()
            .expect("array")
            .is_empty()
    );
}

fn folding_body(id: u32, uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/foldingRange",
        "params": { "textDocument": { "uri": uri } },
    })
    .to_string()
}

#[test]
fn folding_returns_start_and_end_line() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::init(&mut session, &mut workspace);
    open_text(
        &mut session,
        &mut workspace,
        "file:///repo/A.sol",
        "contract A {\nuint x;\n}",
    );
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &folding_body(62, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/startLine").expect("has start"),
        &serde_json::json!(0)
    );
    assert_eq!(
        items[0].pointer("/endLine").expect("has end"),
        &serde_json::json!(1)
    );
}

#[test]
fn folding_default_returns_empty() {
    let mut session = super::stub::session_with_stub(false);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &folding_body(63, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(
        value
            .pointer("/result")
            .expect("has result")
            .as_array()
            .expect("array")
            .is_empty()
    );
}

fn selection_body(id: u32, uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/selectionRange",
        "params": {
            "textDocument": { "uri": uri },
            "positions": [{ "line": 0, "character": 0 }],
        }
    })
    .to_string()
}

#[test]
fn selection_returns_nested_parent() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &selection_body(64, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert!(items[0].pointer("/range/start").is_some());
    assert!(items[0].pointer("/parent/range/start").is_some());
}

#[test]
fn selection_default_falls_back_to_cursor() {
    let mut session = super::stub::session_with_stub(false);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &selection_body(65, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert!(items[0].get("parent").is_none());
}

fn type_prepare_body(id: u32, uri: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/prepareTypeHierarchy",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 10 },
        }
    })
    .to_string()
}

fn type_item_param() -> Value {
    serde_json::json!({
        "name": "Stub",
        "kind": 5,
        "detail": "contract Stub",
        "uri": "file:///repo/A.sol",
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 12 },
        },
        "selectionRange": {
            "start": { "line": 0, "character": 9 },
            "end": { "line": 0, "character": 10 },
        },
    })
}

fn type_body(id: u32, method: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": { "item": type_item_param() },
    })
    .to_string()
}

#[test]
fn type_prepare_returns_one_item() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &type_prepare_body(66, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/name").expect("has name"),
        &serde_json::json!("Stub")
    );
    assert_eq!(
        items[0].pointer("/uri").expect("has uri"),
        &serde_json::json!("file:///repo/A.sol")
    );
}

#[test]
fn type_prepare_default_returns_null() {
    let mut session = super::stub::session_with_stub(false);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &type_prepare_body(67, "file:///repo/A.sol"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(value.get("result").expect("has result").is_null());
}

#[test]
fn type_supertypes_returns_bases() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &type_body(68, "typeHierarchy/supertypes"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/name").expect("has name"),
        &serde_json::json!("Base")
    );
}

#[test]
fn type_subtypes_returns_derived() {
    let mut session = super::stub::session_with_stub(true);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &type_body(69, "typeHierarchy/subtypes"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    let items = value
        .pointer("/result")
        .expect("has result")
        .as_array()
        .expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].pointer("/name").expect("has name"),
        &serde_json::json!("Derived")
    );
}

#[test]
fn type_hierarchy_default_returns_empty() {
    let mut session = super::stub::session_with_stub(false);
    let mut workspace = Workspace::new();
    super::stub::open_sol(&mut session, &mut workspace);
    let reply = handle_message(
        &mut session,
        &mut workspace,
        &type_body(70, "typeHierarchy/supertypes"),
    )
    .expect("must reply");
    let value: Value = serde_json::from_str(&reply).expect("reply is json");
    assert!(
        value
            .pointer("/result")
            .expect("has result")
            .as_array()
            .expect("array")
            .is_empty()
    );
}
