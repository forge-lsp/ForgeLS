use forgels_core::{MAX_SEMANTIC_TOKENS, SemanticTokenData};
use forgels_protocol::{SEMANTIC_TOKEN_MODIFIERS, SEMANTIC_TOKEN_TYPES};
use forgels_syntax::{Lexer, Span};

use crate::{SolidityFile, SolidityLexer};

const CAP: usize = MAX_SEMANTIC_TOKENS;

fn type_index(name: &str) -> Option<u32> {
    SEMANTIC_TOKEN_TYPES
        .iter()
        .position(|known| *known == name)
        .and_then(|index| u32::try_from(index).ok())
}

fn modifier_bit(name: &str) -> u32 {
    let Some(bit) = SEMANTIC_TOKEN_MODIFIERS
        .iter()
        .position(|known| *known == name)
    else {
        return 0;
    };
    let Ok(bit) = u32::try_from(bit) else {
        return 0;
    };
    if bit >= 32 {
        return 0;
    }
    1 << bit
}

fn kind_index(kind: forgels_protocol::SymbolKind, is_param: bool) -> Option<u32> {
    if is_param {
        return type_index("parameter");
    }
    let name = match kind {
        k if k == forgels_protocol::SymbolKind::CLASS => "class",
        k if k == forgels_protocol::SymbolKind::METHOD => "method",
        k if k == forgels_protocol::SymbolKind::FIELD => "property",
        k if k == forgels_protocol::SymbolKind::STRUCT => "struct",
        k if k == forgels_protocol::SymbolKind::ENUM => "enum",
        k if k == forgels_protocol::SymbolKind::ENUM_MEMBER => "enumMember",
        k if k == forgels_protocol::SymbolKind::EVENT => "event",
        k if k == forgels_protocol::SymbolKind::VARIABLE => "variable",
        k if k == forgels_protocol::SymbolKind::TYPE_PARAMETER => "typeParameter",
        k if k == forgels_protocol::SymbolKind::FUNCTION => "function",
        _ => return None,
    };
    type_index(name)
}

fn push_symbol(
    text: &str,
    item: &forgels_core::DocumentSymbolData,
    is_param: bool,
    out: &mut Vec<SemanticTokenData>,
) {
    if out.len() >= CAP {
        return;
    }
    if let Some(index) = kind_index(item.kind, is_param)
        && text
            .get(item.selection.start as usize..item.selection.end as usize)
            .is_some()
        && item.selection.start < item.selection.end
    {
        out.push(SemanticTokenData::new(
            item.selection,
            index,
            modifier_bit("declaration"),
        ));
    }
    let child_is_param = item.kind == forgels_protocol::SymbolKind::METHOD
        || item.kind == forgels_protocol::SymbolKind::EVENT;
    for child in &item.children {
        if out.len() >= CAP {
            break;
        }
        let param = child_is_param && child.kind == forgels_protocol::SymbolKind::VARIABLE;
        push_symbol(text, child, param, out);
    }
}

/// Semantic tokens for the whole file: declarations plus keywords.
///
/// Declarations come from `document_symbols_in` so the outline and the
/// highlighting agree. Keywords come from a lexer scan via
/// `keyword_kind`. Full file only in V1, sorted by offset, capped at
/// `MAX_SEMANTIC_TOKENS`.
pub fn semantic_tokens_in(file: &SolidityFile, text: &str) -> Vec<SemanticTokenData> {
    let mut out = Vec::new();
    if text.is_empty() {
        return out;
    }
    for top in super::symbols::document_symbols_in(file) {
        if out.len() >= CAP {
            break;
        }
        push_symbol(text, &top, false, &mut out);
    }
    let Some(keyword_type) = type_index("keyword") else {
        out.sort_by(|left, right| {
            left.span
                .start
                .cmp(&right.span.start)
                .then(left.span.end.cmp(&right.span.end))
        });
        out.truncate(CAP);
        return out;
    };
    for token in SolidityLexer.tokenize(text) {
        if out.len() >= CAP {
            break;
        }
        if token.text.is_empty() {
            continue;
        }
        if crate::lexer::keyword_kind(token.text.as_str()).is_none() {
            continue;
        }
        if text
            .get(token.span.start as usize..token.span.end as usize)
            .is_none()
        {
            continue;
        }
        if token.span.start >= token.span.end {
            continue;
        }
        if out
            .iter()
            .any(|known| known.span.start == token.span.start && known.span.end == token.span.end)
        {
            continue;
        }
        out.push(SemanticTokenData::new(
            Span::new(token.span.start, token.span.end),
            keyword_type,
            0,
        ));
    }
    out.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then(left.span.end.cmp(&right.span.end))
    });
    out.truncate(CAP);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens_of(text: &str) -> Vec<SemanticTokenData> {
        let lexed = SolidityLexer.tokenize(text);
        let (file, _) = crate::SolidityParser.parse_with_errors(&lexed);
        semantic_tokens_in(&file, text)
    }

    fn has_type(text: &str, needle: &str, want: &str) -> bool {
        let index = type_index(want).expect("legend has type");
        for token in tokens_of(text) {
            let Some(covered) = text.get(token.span.start as usize..token.span.end as usize) else {
                continue;
            };
            if covered == needle && token.token_type == index {
                return true;
            }
        }
        false
    }

    #[test]
    fn contract_and_function_classify() {
        let text = "contract C {\nfunction f() public {}\n}";
        assert!(has_type(text, "C", "class"));
        assert!(has_type(text, "f", "method"));
    }

    #[test]
    fn keywords_classify() {
        let text = "contract C {\nfunction f() public {}\n}";
        assert!(has_type(text, "contract", "keyword"));
        assert!(has_type(text, "function", "keyword"));
    }

    #[test]
    fn decls_carry_declaration_modifier() {
        let text = "contract C {\n}";
        let found = tokens_of(text);
        let class = type_index("class").expect("legend");
        let hit = found
            .iter()
            .find(|token| token.token_type == class)
            .expect("class token");
        assert_eq!(hit.modifiers, modifier_bit("declaration"));
    }

    #[test]
    fn tokens_sort_by_offset() {
        let text = "contract C {\nfunction f() public {}\n}";
        let found = tokens_of(text);
        let mut sorted = found.clone();
        sorted.sort_by_key(|left| left.span.start);
        assert_eq!(found, sorted);
    }
}
