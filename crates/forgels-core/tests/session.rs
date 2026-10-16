use forgels_core::analyzer::{Analyzer, DiagnosticData, HoverData};
use forgels_core::dispatch::{
    Session, drain_due, flush_all, handle_message, immediate_notifications,
};
use forgels_core::routing::Language;
use forgels_core::workspace::Workspace;
use forgels_syntax::Span;
use serde_json::Value;

#[derive(Debug)]
struct StubAnalyzer;

impl Analyzer for StubAnalyzer {
    fn hover(&self, _ctx: &forgels_core::analyzer::ProjectContext) -> Option<HoverData> {
        Some(HoverData {
            contents: "stub hover".to_string(),
            span: Span::new(0, 8),
        })
    }

    fn complete(
        &self,
        _ctx: &forgels_core::analyzer::ProjectContext,
    ) -> Vec<forgels_core::analyzer::CompletionData> {
        Vec::new()
    }

    fn goto(&self, _ctx: &forgels_core::analyzer::ProjectContext) -> Option<Span> {
        None
    }

    fn diagnostics(&self, text: &str) -> Vec<DiagnosticData> {
        if text.contains("broken") {
            vec![DiagnosticData {
                span: Span::new(0, 6),
                message: "stub broken".to_string(),
            }]
        } else {
            Vec::new()
        }
    }
}

/// Reusable session driver: open a server, send frames, read replies.
struct Harness {
    session: Session,
    workspace: Workspace,
    notes: Vec<Value>,
}

impl Harness {
    fn new() -> Self {
        let mut session = Session::new();
        session.analyzers.insert(
            Language::Solidity,
            Box::new(StubAnalyzer) as Box<dyn Analyzer>,
        );
        Self {
            session,
            workspace: Workspace::new(),
            notes: Vec::new(),
        }
    }

    /// Send one raw body. Returns the reply, if any, and stores any
    /// notification bodies for later inspection.
    fn send(&mut self, body: &str) -> Option<Value> {
        let reply = handle_message(&mut self.session, &mut self.workspace, body);
        for note in immediate_notifications(&self.session, &mut self.workspace, body) {
            let frame: Value = serde_json::from_str(&note).expect("note is json");
            self.notes.push(frame);
        }
        reply.map(|text| serde_json::from_str(&text).expect("reply is json"))
    }

    /// Drain debounced publishes due right now.
    fn drain_now(&mut self) {
        let now = std::time::Instant::now();
        for note in drain_due(&self.session, &mut self.workspace, now) {
            let frame: Value = serde_json::from_str(&note).expect("note is json");
            self.notes.push(frame);
        }
    }

    /// Flush every pending publish, due or not.
    fn flush(&mut self) {
        for note in flush_all(&self.session, &mut self.workspace) {
            let frame: Value = serde_json::from_str(&note).expect("note is json");
            self.notes.push(frame);
        }
    }

    fn notifications(&self) -> &[Value] {
        &self.notes
    }
}

fn initialize_body() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "capabilities": {} },
    })
    .to_string()
}

fn open_body(uri: &str, text: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": uri,
                "languageId": "solidity",
                "version": 1,
                "text": text,
            }
        },
    })
    .to_string()
}

#[test]
fn session_shows_diagnostics_then_clears_on_fix() {
    let mut harness = Harness::new();
    let init = harness.send(&initialize_body()).expect("init replies");
    assert!(init.get("result").expect("has result").is_object());

    harness.send(&open_body("file:///repo/Bad.sol", "broken"));
    let published: Vec<&Value> = harness
        .notifications()
        .iter()
        .filter(|note| {
            note.pointer("/method").expect("has method")
                == &serde_json::json!("textDocument/publishDiagnostics")
        })
        .collect();
    assert_eq!(published.len(), 1);
    assert_eq!(
        published[0]
            .pointer("/params/diagnostics/0/message")
            .expect("has message"),
        &serde_json::json!("stub broken")
    );

    let change = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": "file:///repo/Bad.sol", "version": 2 },
            "contentChanges": [{ "text": "fixed" }],
        },
    })
    .to_string();
    harness.send(&change);
    harness.drain_now();
    harness.flush();
    let empties: Vec<&Value> = harness
        .notifications()
        .iter()
        .filter(|note| {
            note.pointer("/params/diagnostics")
                .expect("has diagnostics")
                .as_array()
                .expect("array")
                .is_empty()
        })
        .collect();
    assert!(!empties.is_empty());
}

#[test]
fn session_answers_hover() {
    let mut harness = Harness::new();
    harness.send(&initialize_body());
    harness.send(&open_body("file:///repo/A.sol", "contract A {}"));
    let hover = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": "file:///repo/A.sol" },
            "position": { "line": 0, "character": 10 },
        },
    })
    .to_string();
    let reply = harness.send(&hover).expect("hover replies");
    assert_eq!(
        reply.pointer("/result/contents/value").expect("has value"),
        &serde_json::json!("stub hover")
    );
}
