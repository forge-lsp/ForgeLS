/// Token kinds for Solidity, following the ANTLR grammar main mode.
///
/// Discriminants travel as `u16` inside [`Token`]. Keywords, literals,
/// symbols, and comments each own a variant so the parser can match on
/// kinds without re-reading text.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SolidityKind {
    Error = 0,
    Comment,
    Ident,
    Number,
    StringLit,
    HexString,
    UnicodeString,
    OctalNumber,
    // Directives.
    Pragma,
    Import,
    As,
    From,
    // Definitions.
    Contract,
    Function,
    Constructor,
    Modifier,
    Event,
    Emit,
    // Control flow.
    Returns,
    Return,
    If,
    Else,
    For,
    While,
    // Visibility and mutability.
    Public,
    Private,
    Internal,
    External,
    Pure,
    View,
    Payable,
    Constant,
    Virtual,
    Override,
    // Data location.
    Memory,
    Storage,
    Calldata,
    // Types.
    Bool,
    Address,
    String,
    Bytes,
    Mapping,
    Uint,
    Int,
    UintSized,
    IntSized,
    BytesSized,
    // Boolean literals.
    True,
    False,
    // Symbols.
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    Colon,
    Dot,
    Assign,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Plus,
    PlusPlus,
    PlusAssign,
    Minus,
    MinusMinus,
    MinusAssign,
    Star,
    StarStar,
    StarAssign,
    Slash,
    SlashAssign,
    Percent,
    PercentAssign,
    Amp,
    AmpAmp,
    AmpAssign,
    Pipe,
    PipePipe,
    PipeAssign,
    Caret,
    CaretAssign,
    Tilde,
    Shl,
    ShlAssign,
    Sar,
    AssignSar,
    Shr,
    AssignShr,
    Not,
    RightArrow,
    Question,
    FatArrow,
    // Contract heritage.
    Is,
    Abstract,
    // Full grammar words.
    Anonymous,
    Assembly,
    Break,
    Catch,
    Continue,
    Delete,
    Do,
    Enum,
    ErrorKw,
    Fallback,
    Fixed,
    Global,
    Hex,
    Immutable,
    Indexed,
    Interface,
    Library,
    New,
    Receive,
    Revert,
    Struct,
    Transient,
    Try,
    Type,
    Ufixed,
    Unchecked,
    Unicode,
    Using,
    // Grouped words: units and future-reserved words.
    SubDenomination,
    Reserved,
}

impl SolidityKind {
    /// Kind value stored in [`Token`].
    pub fn as_u16(self) -> u16 {
        self as u16
    }

    /// True for keywords, including sized types and boolean literals.
    pub fn is_keyword(self) -> bool {
        matches!(
            self,
            SolidityKind::Pragma
                | SolidityKind::Import
                | SolidityKind::As
                | SolidityKind::From
                | SolidityKind::Contract
                | SolidityKind::Function
                | SolidityKind::Constructor
                | SolidityKind::Modifier
                | SolidityKind::Event
                | SolidityKind::Emit
                | SolidityKind::Returns
                | SolidityKind::Return
                | SolidityKind::If
                | SolidityKind::Else
                | SolidityKind::For
                | SolidityKind::While
                | SolidityKind::Public
                | SolidityKind::Private
                | SolidityKind::Internal
                | SolidityKind::External
                | SolidityKind::Pure
                | SolidityKind::View
                | SolidityKind::Payable
                | SolidityKind::Constant
                | SolidityKind::Virtual
                | SolidityKind::Override
                | SolidityKind::Memory
                | SolidityKind::Storage
                | SolidityKind::Calldata
                | SolidityKind::Bool
                | SolidityKind::Address
                | SolidityKind::String
                | SolidityKind::Bytes
                | SolidityKind::Mapping
                | SolidityKind::Uint
                | SolidityKind::Int
                | SolidityKind::UintSized
                | SolidityKind::IntSized
                | SolidityKind::BytesSized
                | SolidityKind::True
                | SolidityKind::False
                | SolidityKind::Is
                | SolidityKind::Abstract
                | SolidityKind::Anonymous
                | SolidityKind::Assembly
                | SolidityKind::Break
                | SolidityKind::Catch
                | SolidityKind::Continue
                | SolidityKind::Delete
                | SolidityKind::Do
                | SolidityKind::Enum
                | SolidityKind::ErrorKw
                | SolidityKind::Fallback
                | SolidityKind::Fixed
                | SolidityKind::Global
                | SolidityKind::Hex
                | SolidityKind::Immutable
                | SolidityKind::Indexed
                | SolidityKind::Interface
                | SolidityKind::Library
                | SolidityKind::New
                | SolidityKind::Receive
                | SolidityKind::Revert
                | SolidityKind::Struct
                | SolidityKind::Transient
                | SolidityKind::Try
                | SolidityKind::Type
                | SolidityKind::Ufixed
                | SolidityKind::Unchecked
                | SolidityKind::Unicode
                | SolidityKind::Using
                | SolidityKind::SubDenomination
                | SolidityKind::Reserved
        )
    }
}

/// Every fixed keyword spelling, for completion lists.
/// Sized `uint`/`int`/`bytes` types live in `builtin_types`, not here.
/// Fixed `MxN` forms stay out: 5184 combos would flood completion.
pub const KEYWORDS: &[&str] = &[
    "pragma",
    "import",
    "as",
    "from",
    "contract",
    "function",
    "constructor",
    "modifier",
    "event",
    "emit",
    "returns",
    "return",
    "if",
    "else",
    "for",
    "while",
    "public",
    "private",
    "internal",
    "external",
    "pure",
    "view",
    "payable",
    "constant",
    "virtual",
    "override",
    "memory",
    "storage",
    "calldata",
    "bool",
    "address",
    "string",
    "bytes",
    "mapping",
    "uint",
    "int",
    "true",
    "false",
    "is",
    "abstract",
    "anonymous",
    "assembly",
    "break",
    "catch",
    "continue",
    "delete",
    "do",
    "enum",
    "error",
    "fallback",
    "fixed",
    "global",
    "hex",
    "immutable",
    "indexed",
    "interface",
    "library",
    "new",
    "receive",
    "revert",
    "struct",
    "transient",
    "try",
    "type",
    "ufixed",
    "unchecked",
    "unicode",
    "using",
];

/// All builtin sized types in deterministic order: `uint8` to `uint256`,
/// `int8` to `int256`, then `bytes1` to `bytes32`. Total is 96 entries.
/// Fixed `MxN` forms stay out by design: 5184 combos would flood
/// completion. Bare `fixed` and `ufixed` already live in `KEYWORDS`.
pub fn builtin_types() -> Vec<String> {
    let mut out = Vec::with_capacity(96);
    for size in (8..=256).step_by(8) {
        out.push(format!("uint{size}"));
    }
    for size in (8..=256).step_by(8) {
        out.push(format!("int{size}"));
    }
    for size in 1..=32 {
        out.push(format!("bytes{size}"));
    }
    out
}

/// Map a word to its keyword kind. Returns `None` for identifiers,
/// including sized types outside their valid ranges.
pub fn keyword_kind(word: &str) -> Option<SolidityKind> {
    let kind = match word {
        "pragma" => SolidityKind::Pragma,
        "import" => SolidityKind::Import,
        "as" => SolidityKind::As,
        "from" => SolidityKind::From,
        "contract" => SolidityKind::Contract,
        "function" => SolidityKind::Function,
        "constructor" => SolidityKind::Constructor,
        "modifier" => SolidityKind::Modifier,
        "event" => SolidityKind::Event,
        "emit" => SolidityKind::Emit,
        "returns" => SolidityKind::Returns,
        "return" => SolidityKind::Return,
        "if" => SolidityKind::If,
        "else" => SolidityKind::Else,
        "for" => SolidityKind::For,
        "while" => SolidityKind::While,
        "public" => SolidityKind::Public,
        "private" => SolidityKind::Private,
        "internal" => SolidityKind::Internal,
        "external" => SolidityKind::External,
        "pure" => SolidityKind::Pure,
        "view" => SolidityKind::View,
        "payable" => SolidityKind::Payable,
        "constant" => SolidityKind::Constant,
        "virtual" => SolidityKind::Virtual,
        "override" => SolidityKind::Override,
        "memory" => SolidityKind::Memory,
        "storage" => SolidityKind::Storage,
        "calldata" => SolidityKind::Calldata,
        "bool" => SolidityKind::Bool,
        "address" => SolidityKind::Address,
        "string" => SolidityKind::String,
        "bytes" => SolidityKind::Bytes,
        "mapping" => SolidityKind::Mapping,
        "uint" => SolidityKind::Uint,
        "int" => SolidityKind::Int,
        "true" => SolidityKind::True,
        "false" => SolidityKind::False,
        "is" => SolidityKind::Is,
        "abstract" => SolidityKind::Abstract,
        "anonymous" => SolidityKind::Anonymous,
        "assembly" => SolidityKind::Assembly,
        "break" => SolidityKind::Break,
        "catch" => SolidityKind::Catch,
        "continue" => SolidityKind::Continue,
        "delete" => SolidityKind::Delete,
        "do" => SolidityKind::Do,
        "enum" => SolidityKind::Enum,
        "error" => SolidityKind::ErrorKw,
        "fallback" => SolidityKind::Fallback,
        "fixed" => SolidityKind::Fixed,
        "global" => SolidityKind::Global,
        "hex" => SolidityKind::Hex,
        "immutable" => SolidityKind::Immutable,
        "indexed" => SolidityKind::Indexed,
        "interface" => SolidityKind::Interface,
        "library" => SolidityKind::Library,
        "new" => SolidityKind::New,
        "receive" => SolidityKind::Receive,
        "revert" => SolidityKind::Revert,
        "struct" => SolidityKind::Struct,
        "transient" => SolidityKind::Transient,
        "try" => SolidityKind::Try,
        "type" => SolidityKind::Type,
        "ufixed" => SolidityKind::Ufixed,
        "unchecked" => SolidityKind::Unchecked,
        "unicode" => SolidityKind::Unicode,
        "using" => SolidityKind::Using,
        _ => {
            return fixed_type_kind(word)
                .or_else(|| sized_type_kind(word))
                .or_else(|| grouped_kind(word));
        }
    };
    Some(kind)
}

/// Map sized types: `uint8` to `uint256` in steps of 8, `int8` to
/// `int256` in steps of 8, `bytes1` to `bytes32` in steps of 1.
fn sized_type_kind(word: &str) -> Option<SolidityKind> {
    if let Some(bits) = word.strip_prefix("uint")
        && let Ok(size) = bits.parse::<u32>()
        && (8..=256).contains(&size)
        && size % 8 == 0
    {
        return Some(SolidityKind::UintSized);
    }
    if let Some(bits) = word.strip_prefix("int")
        && let Ok(size) = bits.parse::<u32>()
        && (8..=256).contains(&size)
        && size % 8 == 0
    {
        return Some(SolidityKind::IntSized);
    }
    if let Some(size) = word.strip_prefix("bytes")
        && let Ok(size) = size.parse::<u32>()
        && (1..=32).contains(&size)
    {
        return Some(SolidityKind::BytesSized);
    }
    None
}

/// Map fixed-point types: bare `fixed` and `ufixed`, plus `MxN` forms
/// where both parts are non-zero leading digits without a leading zero.
fn fixed_type_kind(word: &str) -> Option<SolidityKind> {
    for (prefix, kind) in [
        ("fixed", SolidityKind::Fixed),
        ("ufixed", SolidityKind::Ufixed),
    ] {
        if let Some(rest) = word.strip_prefix(prefix) {
            if rest.is_empty() {
                return Some(kind);
            }
            let mut halves = rest.split('x');
            match (halves.next(), halves.next(), halves.next()) {
                (Some(mantissa), Some(exponent), None)
                    if is_nonzero_digits(mantissa) && is_nonzero_digits(exponent) =>
                {
                    return Some(kind);
                }
                _ => {}
            }
        }
    }
    None
}

fn is_nonzero_digits(text: &str) -> bool {
    let mut chars = text.bytes();
    if !matches!(chars.next(), Some(b'1'..=b'9')) {
        return false;
    }
    chars.all(|byte| byte.is_ascii_digit())
}

/// Map unit denominations and future-reserved words to their group kinds.
fn grouped_kind(word: &str) -> Option<SolidityKind> {
    const DENOMINATIONS: &[&str] = &[
        "wei", "gwei", "ether", "seconds", "minutes", "hours", "days", "weeks", "years",
    ];
    const RESERVED: &[&str] = &[
        "after",
        "alias",
        "apply",
        "auto",
        "byte",
        "case",
        "copyof",
        "default",
        "define",
        "final",
        "implements",
        "in",
        "inline",
        "let",
        "macro",
        "match",
        "mutable",
        "null",
        "of",
        "partial",
        "promise",
        "reference",
        "relocatable",
        "sealed",
        "sizeof",
        "static",
        "supports",
        "switch",
        "typedef",
        "typeof",
        "var",
    ];
    if DENOMINATIONS.contains(&word) {
        Some(SolidityKind::SubDenomination)
    } else if RESERVED.contains(&word) {
        Some(SolidityKind::Reserved)
    } else {
        None
    }
}

/// Map an operator to its symbol kind. The lexer tries the longest match
/// first; this table only answers exact hits.
pub fn symbol_kind(op: &str) -> Option<SolidityKind> {
    let kind = match op {
        "{" => SolidityKind::LBrace,
        "}" => SolidityKind::RBrace,
        "(" => SolidityKind::LParen,
        ")" => SolidityKind::RParen,
        "[" => SolidityKind::LBracket,
        "]" => SolidityKind::RBracket,
        ";" => SolidityKind::Semicolon,
        "," => SolidityKind::Comma,
        ":" => SolidityKind::Colon,
        "." => SolidityKind::Dot,
        "=" => SolidityKind::Assign,
        "==" => SolidityKind::Eq,
        "!=" => SolidityKind::NotEq,
        "<" => SolidityKind::Lt,
        "<=" => SolidityKind::LtEq,
        ">" => SolidityKind::Gt,
        ">=" => SolidityKind::GtEq,
        "+" => SolidityKind::Plus,
        "++" => SolidityKind::PlusPlus,
        "+=" => SolidityKind::PlusAssign,
        "-" => SolidityKind::Minus,
        "--" => SolidityKind::MinusMinus,
        "-=" => SolidityKind::MinusAssign,
        "*" => SolidityKind::Star,
        "**" => SolidityKind::StarStar,
        "*=" => SolidityKind::StarAssign,
        "/" => SolidityKind::Slash,
        "/=" => SolidityKind::SlashAssign,
        "%" => SolidityKind::Percent,
        "%=" => SolidityKind::PercentAssign,
        "&" => SolidityKind::Amp,
        "&&" => SolidityKind::AmpAmp,
        "&=" => SolidityKind::AmpAssign,
        "|" => SolidityKind::Pipe,
        "||" => SolidityKind::PipePipe,
        "|=" => SolidityKind::PipeAssign,
        "^" => SolidityKind::Caret,
        "^=" => SolidityKind::CaretAssign,
        "~" => SolidityKind::Tilde,
        "<<" => SolidityKind::Shl,
        "<<=" => SolidityKind::ShlAssign,
        ">>" => SolidityKind::Sar,
        ">>=" => SolidityKind::AssignSar,
        ">>>" => SolidityKind::Shr,
        ">>>=" => SolidityKind::AssignShr,
        "!" => SolidityKind::Not,
        "->" => SolidityKind::RightArrow,
        "?" => SolidityKind::Question,
        "=>" => SolidityKind::FatArrow,
        _ => return None,
    };
    Some(kind)
}
