use forgels_core::analyzer::ImportRef;
use forgels_syntax::Lexer;

use crate::{SolidityLexer, SolidityParser};

/// Collect import specifiers from Solidity source.
///
/// Parses with recovery so broken files still report their imports.
/// Entries without a path are skipped.
pub fn imports_in(text: &str) -> Vec<ImportRef> {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    file.imports()
        .iter()
        .filter_map(|directive| {
            let raw = directive.path()?;
            let span = directive
                .path_span()
                .unwrap_or(forgels_syntax::Span::new(0, 0));
            Some(ImportRef::with_span(raw, directive.symbols(), span))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_paths_and_symbols() {
        let text = "import \"./A.sol\";\nimport {B as C} from \"./B.sol\";\ncontract A {}\n";
        let found = imports_in(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].raw, "./A.sol");
        assert_eq!(found[1].raw, "./B.sol");
        assert!(found[1].symbols.contains(&"B".to_string()));
    }

    #[test]
    fn broken_file_still_reports_imports() {
        let text = "import \"./A.sol\";\ncontract ;\n";
        let found = imports_in(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].raw, "./A.sol");
    }

    #[test]
    fn file_without_imports_is_empty() {
        assert!(imports_in("contract A {}").is_empty());
    }
}
