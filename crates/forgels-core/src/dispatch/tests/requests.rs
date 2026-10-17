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
