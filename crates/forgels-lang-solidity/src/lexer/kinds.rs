/// Token kinds for the Solidity beta subset.
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
    Shr,
    ShrAssign,
    Question,
    FatArrow,
    // Contract heritage.
    Is,
    Abstract,
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
        )
    }
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
        _ => return sized_type_kind(word),
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
        ">>" => SolidityKind::Shr,
        ">>=" => SolidityKind::ShrAssign,
        "?" => SolidityKind::Question,
        "=>" => SolidityKind::FatArrow,
        _ => return None,
    };
    Some(kind)
}
