use super::{SolidityKind, SolidityLexer, keyword_kind, symbol_kind};
use forgels_syntax::Lexer;

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
        ("is", SolidityKind::Is),
        ("abstract", SolidityKind::Abstract),
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
