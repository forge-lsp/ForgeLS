#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

impl Location {
    pub fn new(uri: String, range: Range) -> Self {
        Self { uri, range }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Option<DiagnosticSeverity>,
    pub message: String,
}

impl Diagnostic {
    pub fn new(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: None,
            message: message.into(),
        }
    }

    pub fn error(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: Some(DiagnosticSeverity::Error),
            message: message.into(),
        }
    }
}

/// Severity levels from the LSP specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}
