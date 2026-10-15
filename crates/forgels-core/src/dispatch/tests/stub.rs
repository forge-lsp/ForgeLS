use super::super::{Session, handle_message};
use crate::workspace::Workspace;

#[derive(Debug)]
struct StubAnalyzer {
    found: bool,
}

impl crate::analyzer::Analyzer for StubAnalyzer {
    fn hover(&self, _text: &str, _offset: u32) -> Option<crate::analyzer::HoverData> {
        if self.found {
            Some(crate::analyzer::HoverData {
                contents: "stub detail".to_string(),
                span: forgels_syntax::Span::new(13, 18),
            })
        } else {
            None
        }
    }

    fn complete(&self, _text: &str, _offset: u32) -> Vec<crate::analyzer::CompletionData> {
        vec![
            crate::analyzer::CompletionData {
                label: "stub_one".to_string(),
                detail: "first".to_string(),
            },
            crate::analyzer::CompletionData {
                label: "stub_two".to_string(),
                detail: "second".to_string(),
            },
        ]
    }

    fn goto(&self, _text: &str, _offset: u32) -> Option<forgels_syntax::Span> {
        Some(forgels_syntax::Span::new(13, 18))
    }

    fn diagnostics(&self, text: &str) -> Vec<crate::analyzer::DiagnosticData> {
        if text.contains("broken") {
            vec![crate::analyzer::DiagnosticData {
                span: forgels_syntax::Span::new(0, 6),
                message: "stub broken".to_string(),
            }]
        } else {
            Vec::new()
        }
    }
}

pub(crate) fn session_with_stub(found: bool) -> Session {
    let mut session = Session::new();
    session.analyzers.insert(
        crate::routing::Language::Solidity,
        Box::new(StubAnalyzer { found }),
    );
    session
}

pub(crate) fn open_sol(workspace: &mut Workspace) {
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/A.sol","languageId":"solidity","version":1,"text":"contract A {}"}}}"#;
    let mut session = Session::new();
    assert!(handle_message(&mut session, workspace, open).is_none());
}
