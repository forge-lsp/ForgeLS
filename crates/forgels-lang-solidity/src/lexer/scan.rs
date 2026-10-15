use super::kinds::{SolidityKind, keyword_kind, symbol_kind};
use forgels_syntax::{Lexer, Span, Token};

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
