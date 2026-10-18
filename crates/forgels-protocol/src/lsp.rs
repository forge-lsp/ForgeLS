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
    pub code: Option<String>,
    pub source: Option<String>,
}

impl Diagnostic {
    pub fn new(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: None,
            message: message.into(),
            code: None,
            source: None,
        }
    }

    pub fn error(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: Some(DiagnosticSeverity::Error),
            message: message.into(),
            code: None,
            source: None,
        }
    }

    pub fn warning(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: Some(DiagnosticSeverity::Warning),
            message: message.into(),
            code: None,
            source: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
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

/// LSP symbol kind, passed through as a plain number.
///
/// Core and language crates forward the value only. No mapping here.
/// Numbers follow LSP 3.17 in declaration order: File is 1 through
/// TypeParameter is 26.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolKind(pub u8);

impl SymbolKind {
    pub const FILE: Self = Self(1);
    pub const MODULE: Self = Self(2);
    pub const NAMESPACE: Self = Self(3);
    pub const PACKAGE: Self = Self(4);
    pub const CLASS: Self = Self(5);
    pub const METHOD: Self = Self(6);
    pub const PROPERTY: Self = Self(7);
    pub const FIELD: Self = Self(8);
    pub const CONSTRUCTOR: Self = Self(9);
    pub const ENUM: Self = Self(10);
    pub const INTERFACE: Self = Self(11);
    pub const FUNCTION: Self = Self(12);
    pub const VARIABLE: Self = Self(13);
    pub const CONSTANT: Self = Self(14);
    pub const STRING: Self = Self(15);
    pub const NUMBER: Self = Self(16);
    pub const BOOLEAN: Self = Self(17);
    pub const ARRAY: Self = Self(18);
    pub const OBJECT: Self = Self(19);
    pub const KEY: Self = Self(20);
    pub const NULL: Self = Self(21);
    pub const ENUM_MEMBER: Self = Self(22);
    pub const STRUCT: Self = Self(23);
    pub const EVENT: Self = Self(24);
    pub const OPERATOR: Self = Self(25);
    pub const TYPE_PARAMETER: Self = Self(26);

    pub fn value(self) -> u8 {
        self.0
    }
}

/// LSP completion kind, passed through as a plain number.
///
/// Core and language crates forward the value only. No mapping here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionItemKind(pub u8);
impl CompletionItemKind {
    pub const TEXT: Self = Self(1);
    pub const METHOD: Self = Self(2);
    pub const FUNCTION: Self = Self(3);
    pub const CONSTRUCTOR: Self = Self(4);
    pub const FIELD: Self = Self(5);
    pub const VARIABLE: Self = Self(6);
    pub const CLASS: Self = Self(7);
    pub const INTERFACE: Self = Self(8);
    pub const MODULE: Self = Self(9);
    pub const PROPERTY: Self = Self(10);
    pub const UNIT: Self = Self(11);
    pub const VALUE: Self = Self(12);
    pub const ENUM: Self = Self(13);
    pub const KEYWORD: Self = Self(14);
    pub const SNIPPET: Self = Self(15);
    pub const COLOR: Self = Self(16);
    pub const FILE: Self = Self(17);
    pub const REFERENCE: Self = Self(18);
    pub const FOLDER: Self = Self(19);
    pub const ENUM_MEMBER: Self = Self(20);
    pub const CONSTANT: Self = Self(21);
    pub const STRUCT: Self = Self(22);
    pub const EVENT: Self = Self(23);
    pub const OPERATOR: Self = Self(24);
    pub const TYPE_PARAMETER: Self = Self(25);

    pub fn value(self) -> u8 {
        self.0
    }
}

/// LSP inlay hint kind, passed through as a plain number.
///
/// Core and language crates forward the value only. Numbers follow
/// LSP 3.17: Type is 1, Parameter is 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlayHintKind(pub u8);

impl InlayHintKind {
    pub const TYPE: Self = Self(1);
    pub const PARAMETER: Self = Self(2);

    pub fn value(self) -> u8 {
        self.0
    }
}

/// Frozen semantic token legend for `semanticTokens/full`.
///
/// FREEZE: do not reorder, only append at the end. Clients cache
/// indices, so a reorder breaks highlighting. Languages map their
/// declarations to these indices.
pub const SEMANTIC_TOKEN_TYPES: &[&str] = &[
    "namespace",
    "type",
    "class",
    "enum",
    "interface",
    "struct",
    "typeParameter",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "event",
    "function",
    "method",
    "keyword",
    "modifier",
];

/// Frozen modifier legend, same freeze rule as the types above.
pub const SEMANTIC_TOKEN_MODIFIERS: &[&str] = &["declaration", "definition"];
