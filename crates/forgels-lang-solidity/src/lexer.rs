use forgels_syntax::{Lexer, Span, Token};

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

#[derive(Debug, Default)]
pub struct SolidityLexer;

impl Lexer for SolidityLexer {
    fn tokenize(&self, input: &str) -> Vec<Token> {
        Scanner::new(input).run()
    }
}

/// Byte scanner with char-boundary safe steps.
///
/// Whitespace is skipped. Everything else becomes one token with exact
/// byte spans. Unknown bytes surface as single-char [`SolidityKind::Error`]
/// tokens so the parser can report them instead of crashing.
struct Scanner<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
    tokens: Vec<Token>,
}

impl<'a> Scanner<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            pos: 0,
            tokens: Vec::new(),
        }
    }

    fn run(mut self) -> Vec<Token> {
        while self.pos < self.bytes.len() {
            let byte = self.bytes[self.pos];
            if byte.is_ascii_whitespace() {
                self.pos += 1;
            } else if byte == b'/' && self.at(1) == Some(b'/') {
                self.line_comment();
            } else if byte == b'/' && self.at(1) == Some(b'*') {
                self.block_comment();
            } else if is_ident_start(byte) {
                self.word();
            } else if byte.is_ascii_digit() {
                self.number();
            } else if byte == b'\'' || byte == b'"' {
                self.string(byte);
            } else {
                self.symbol_or_error();
            }
        }
        self.tokens
    }

    fn at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    fn push(&mut self, kind: SolidityKind, start: usize, end: usize) {
        let text = self.input.get(start..end).unwrap_or("").to_string();
        self.tokens.push(Token::new(
            kind.as_u16(),
            text,
            Span::new(start as u32, end as u32),
        ));
    }

    fn line_comment(&mut self) {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
            self.pos += 1;
        }
        self.push(SolidityKind::Comment, start, self.pos);
    }

    fn block_comment(&mut self) {
        let start = self.pos;
        self.pos += 2;
        while self.pos < self.bytes.len() {
            if self.bytes[self.pos] == b'*' && self.at(1) == Some(b'/') {
                self.pos += 2;
                break;
            }
            self.pos += 1;
        }
        // Unterminated block comments run to EOF on purpose: the parser
        // reports the missing close instead of the lexer dropping text.
        self.push(SolidityKind::Comment, start, self.pos);
    }

    fn word(&mut self) {
        let start = self.pos;
        while let Some(byte) = self.at(0)
            && is_ident_continue(byte)
        {
            self.pos += 1;
        }
        let text = self.input.get(start..self.pos).unwrap_or("");
        let kind = keyword_kind(text).unwrap_or(SolidityKind::Ident);
        self.push(kind, start, self.pos);
    }

    fn number(&mut self) {
        let start = self.pos;
        if self.at(0) == Some(b'0') && matches!(self.at(1), Some(b'x') | Some(b'X')) {
            self.pos += 2;
            while let Some(byte) = self.at(0)
                && (byte.is_ascii_hexdigit() || byte == b'_')
            {
                self.pos += 1;
            }
        } else {
            while let Some(byte) = self.at(0)
                && (byte.is_ascii_digit() || byte == b'_')
            {
                self.pos += 1;
            }
            if self.at(0) == Some(b'.')
                && let Some(next) = self.at(1)
                && next.is_ascii_digit()
            {
                self.pos += 1;
                while let Some(byte) = self.at(0)
                    && (byte.is_ascii_digit() || byte == b'_')
                {
                    self.pos += 1;
                }
            }
            if matches!(self.at(0), Some(b'e') | Some(b'E')) {
                let mut end = self.pos + 1;
                if matches!(self.bytes.get(end), Some(b'+') | Some(b'-')) {
                    end += 1;
                }
                if let Some(byte) = self.bytes.get(end)
                    && byte.is_ascii_digit()
                {
                    self.pos = end;
                    while let Some(byte) = self.at(0)
                        && (byte.is_ascii_digit() || byte == b'_')
                    {
                        self.pos += 1;
                    }
                }
            }
        }
        self.push(SolidityKind::Number, start, self.pos);
    }

    fn string(&mut self, quote: u8) {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.bytes.len() {
            let byte = self.bytes[self.pos];
            if byte == b'\\' {
                self.pos = (self.pos + 2).min(self.bytes.len());
            } else if byte == quote {
                self.pos += 1;
                break;
            } else {
                self.pos += 1;
            }
        }
        // Unterminated strings run to EOF on purpose, same as block comments.
        self.push(SolidityKind::StringLit, start, self.pos);
    }

    fn symbol_or_error(&mut self) {
        let start = self.pos;
        for len in [3, 2, 1] {
            if let Some(text) = self.input.get(start..start + len)
                && let Some(kind) = symbol_kind(text)
            {
                self.pos += len;
                self.push(kind, start, self.pos);
                return;
            }
        }
        // Unknown character: one Error token over its full width.
        let width = self.input[start..]
            .chars()
            .next()
            .map(|first| first.len_utf8())
            .unwrap_or(1);
        self.pos += width;
        self.push(SolidityKind::Error, start, self.pos);
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$'
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_subset_keyword_maps_to_its_kind() {
        let cases = [
            ("pragma", SolidityKind::Pragma),
            ("import", SolidityKind::Import),
            ("as", SolidityKind::As),
            ("from", SolidityKind::From),
            ("contract", SolidityKind::Contract),
            ("function", SolidityKind::Function),
            ("constructor", SolidityKind::Constructor),
            ("modifier", SolidityKind::Modifier),
            ("event", SolidityKind::Event),
            ("emit", SolidityKind::Emit),
            ("returns", SolidityKind::Returns),
            ("return", SolidityKind::Return),
            ("if", SolidityKind::If),
            ("else", SolidityKind::Else),
            ("for", SolidityKind::For),
            ("while", SolidityKind::While),
            ("public", SolidityKind::Public),
            ("private", SolidityKind::Private),
            ("internal", SolidityKind::Internal),
            ("external", SolidityKind::External),
            ("pure", SolidityKind::Pure),
            ("view", SolidityKind::View),
            ("payable", SolidityKind::Payable),
            ("constant", SolidityKind::Constant),
            ("virtual", SolidityKind::Virtual),
            ("override", SolidityKind::Override),
            ("memory", SolidityKind::Memory),
            ("storage", SolidityKind::Storage),
            ("calldata", SolidityKind::Calldata),
            ("bool", SolidityKind::Bool),
            ("address", SolidityKind::Address),
            ("string", SolidityKind::String),
            ("bytes", SolidityKind::Bytes),
            ("mapping", SolidityKind::Mapping),
            ("uint", SolidityKind::Uint),
            ("int", SolidityKind::Int),
            ("true", SolidityKind::True),
            ("false", SolidityKind::False),
        ];
        for (word, kind) in cases {
            assert_eq!(keyword_kind(word), Some(kind), "word {word}");
            assert!(kind.is_keyword(), "flag {word}");
        }
    }

    #[test]
    fn sized_types_cover_valid_ranges_only() {
        for size in [8, 128, 256] {
            assert_eq!(
                keyword_kind(&format!("uint{size}")),
                Some(SolidityKind::UintSized)
            );
            assert_eq!(
                keyword_kind(&format!("int{size}")),
                Some(SolidityKind::IntSized)
            );
        }
        for size in [1, 20, 32] {
            assert_eq!(
                keyword_kind(&format!("bytes{size}")),
                Some(SolidityKind::BytesSized)
            );
        }
        for word in [
            "uint7", "uint257", "uint", "int0", "bytes0", "bytes33", "uintx",
        ] {
            if word == "uint" {
                assert_eq!(keyword_kind(word), Some(SolidityKind::Uint));
            } else {
                assert_eq!(keyword_kind(word), None, "word {word}");
            }
        }
    }

    #[test]
    fn plain_identifiers_stay_identifiers() {
        for word in ["Token", "totalSupply", "struct", "myVar", "contract2"] {
            assert_eq!(keyword_kind(word), None, "word {word}");
        }
    }

    #[test]
    fn every_subset_symbol_maps() {
        let cases = [
            ("{", SolidityKind::LBrace),
            ("}", SolidityKind::RBrace),
            ("(", SolidityKind::LParen),
            (")", SolidityKind::RParen),
            ("[", SolidityKind::LBracket),
            ("]", SolidityKind::RBracket),
            (";", SolidityKind::Semicolon),
            (",", SolidityKind::Comma),
            (":", SolidityKind::Colon),
            (".", SolidityKind::Dot),
            ("=", SolidityKind::Assign),
            ("==", SolidityKind::Eq),
            ("!=", SolidityKind::NotEq),
            ("<", SolidityKind::Lt),
            ("<=", SolidityKind::LtEq),
            (">", SolidityKind::Gt),
            (">=", SolidityKind::GtEq),
            ("+", SolidityKind::Plus),
            ("++", SolidityKind::PlusPlus),
            ("+=", SolidityKind::PlusAssign),
            ("-", SolidityKind::Minus),
            ("--", SolidityKind::MinusMinus),
            ("-=", SolidityKind::MinusAssign),
            ("*", SolidityKind::Star),
            ("**", SolidityKind::StarStar),
            ("*=", SolidityKind::StarAssign),
            ("/", SolidityKind::Slash),
            ("/=", SolidityKind::SlashAssign),
            ("%", SolidityKind::Percent),
            ("%=", SolidityKind::PercentAssign),
            ("&", SolidityKind::Amp),
            ("&&", SolidityKind::AmpAmp),
            ("&=", SolidityKind::AmpAssign),
            ("|", SolidityKind::Pipe),
            ("||", SolidityKind::PipePipe),
            ("|=", SolidityKind::PipeAssign),
            ("^", SolidityKind::Caret),
            ("^=", SolidityKind::CaretAssign),
            ("~", SolidityKind::Tilde),
            ("<<", SolidityKind::Shl),
            ("<<=", SolidityKind::ShlAssign),
            (">>", SolidityKind::Shr),
            (">>=", SolidityKind::ShrAssign),
            ("?", SolidityKind::Question),
            ("=>", SolidityKind::FatArrow),
        ];
        for (op, kind) in cases {
            assert_eq!(symbol_kind(op), Some(kind), "op {op}");
            assert!(!kind.is_keyword(), "flag {op}");
        }
        assert_eq!(symbol_kind(">>>"), None);
        assert_eq!(symbol_kind("@"), None);
    }

    #[test]
    fn kinds_travel_as_u16() {
        assert_eq!(
            SolidityKind::Contract.as_u16(),
            SolidityKind::Contract as u16
        );
        assert_eq!(SolidityKind::Error.as_u16(), 0);
    }

    fn kinds_of(input: &str) -> Vec<u16> {
        SolidityLexer
            .tokenize(input)
            .iter()
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn example_contract_lexes_fully() {
        let input = "pragma solidity ^0.8.0;\ncontract C {\n}";
        let kinds = kinds_of(input);
        assert_eq!(
            kinds,
            vec![
                SolidityKind::Pragma.as_u16(),
                SolidityKind::Ident.as_u16(),
                SolidityKind::Caret.as_u16(),
                SolidityKind::Number.as_u16(),
                SolidityKind::Dot.as_u16(),
                SolidityKind::Number.as_u16(),
                SolidityKind::Semicolon.as_u16(),
                SolidityKind::Contract.as_u16(),
                SolidityKind::Ident.as_u16(),
                SolidityKind::LBrace.as_u16(),
                SolidityKind::RBrace.as_u16(),
            ]
        );
    }

    #[test]
    fn spans_are_sequential_and_cover_input() {
        let inputs = [
            "pragma solidity ^0.8.0;\ncontract C {}",
            "contract C {\n  uint256 total; // supply\n  /* block */\n}",
            "function f(uint a) public returns (uint) { return a + 1; }",
            "import {A} from \"./A.sol\";",
            "mapping(address => uint) balances;",
        ];
        let lexer = SolidityLexer;
        for input in inputs {
            let tokens = lexer.tokenize(input);
            assert!(!tokens.is_empty(), "input {input}");
            assert_eq!(tokens.first().expect("first").span.start, 0);
            assert_eq!(
                tokens.last().expect("last").span.end as usize,
                input.len(),
                "input {input}"
            );
            for pair in tokens.windows(2) {
                assert!(pair[0].span.start <= pair[0].span.end, "ordered in {input}");
                assert!(
                    pair[0].span.end <= pair[1].span.start,
                    "sequential in {input}"
                );
                assert_eq!(
                    &input[pair[0].span.start as usize..pair[0].span.end as usize],
                    pair[0].text,
                    "text matches span in {input}"
                );
            }
        }
    }

    #[test]
    fn comments_strings_and_errors_lex() {
        let kinds = kinds_of("// line\n/* block */ 'a' \"b\" @");
        assert_eq!(
            kinds,
            vec![
                SolidityKind::Comment.as_u16(),
                SolidityKind::Comment.as_u16(),
                SolidityKind::StringLit.as_u16(),
                SolidityKind::StringLit.as_u16(),
                SolidityKind::Error.as_u16(),
            ]
        );
    }

    #[test]
    fn unterminated_comment_and_string_run_to_eof() {
        let input = "// open\n'open";
        let tokens = SolidityLexer.tokenize(input);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, SolidityKind::Comment.as_u16());
        assert_eq!(tokens[1].kind, SolidityKind::StringLit.as_u16());
        assert_eq!(tokens.last().expect("last").span.end as usize, input.len());
    }

    #[test]
    fn hex_and_exponent_numbers_lex() {
        let kinds = kinds_of("0xFF 1_000 1.5 1e10");
        assert_eq!(
            kinds,
            vec![
                SolidityKind::Number.as_u16(),
                SolidityKind::Number.as_u16(),
                SolidityKind::Number.as_u16(),
                SolidityKind::Number.as_u16(),
            ]
        );
    }
}
