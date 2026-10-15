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
pub struct Diagnostic {
    pub range: Range,
    pub message: String,
}

impl Diagnostic {
    pub fn new(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            message: message.into(),
        }
    }
}
