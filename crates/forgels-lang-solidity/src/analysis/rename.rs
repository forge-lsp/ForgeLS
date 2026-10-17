use std::path::{Path, PathBuf};

use forgels_core::{ProjectContext, ReferenceData};

use super::references::find_references;
use crate::{SolidityFile, lexer::keyword_kind};

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$'
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// True when `new_name` is a usable rename target.
///
/// Rejects empty names, bad shapes (`1abc`, `a-b`), and any keyword
/// (`contract`, `uint8`, `wei`, reserved words) via `keyword_kind`.
/// Keywords would break the file if written to every reference slot.
pub fn is_valid_rename(new_name: &str) -> bool {
    if new_name.is_empty() {
        return false;
    }
    let bytes = new_name.as_bytes();
    if !is_ident_start(bytes[0]) {
        return false;
    }
    for byte in &bytes[1..] {
        if !is_ident_continue(*byte) {
            return false;
        }
    }
    if keyword_kind(new_name).is_some() {
        return false;
    }
    true
}

/// Rename targets for the symbol at `offset`.
///
/// Validates `new_name` first, then reuses `find_references` for the
/// edit list. Returns `None` for invalid names, builtins, and unknown
/// words. Empty reference lists also map to `None` so dispatch can
/// answer `InvalidParams` with one check.
pub fn find_rename(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    new_name: &str,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
    ctx: &ProjectContext,
) -> Option<Vec<ReferenceData>> {
    if !is_valid_rename(new_name) {
        return None;
    }
    let found = find_references(file, text, offset, current, deps, ctx)?;
    if found.is_empty() {
        return None;
    }
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgels_core::Analyzer;
    use forgels_core::routing::Language;
    use forgels_syntax::Lexer;

    fn parse_text(text: &str) -> SolidityFile {
        let tokens = crate::SolidityLexer.tokenize(text);
        let (file, _) = crate::SolidityParser.parse_with_errors(&tokens);
        file
    }

    fn workspace_with(files: &[(&str, &str)]) -> forgels_core::Workspace {
        let mut workspace = forgels_core::Workspace::new();
        for (name, text) in files {
            workspace
                .open(
                    PathBuf::from(name),
                    Language::Solidity,
                    (*text).to_string(),
                    1,
                )
                .expect("open works");
        }
        workspace
    }

    fn rename_in(
        workspace: &forgels_core::Workspace,
        path: &str,
        text: &str,
        needle: &str,
        new_name: &str,
    ) -> Option<Vec<ReferenceData>> {
        let buf = PathBuf::from(path);
        let at = text.rfind(needle).expect("needle") as u32;
        let ctx = ProjectContext::new(buf.as_path(), text, at, workspace);
        let file = parse_text(text);
        let mut deps = Vec::new();
        for (dep_path, dep_text) in forgels_core::dep_texts(&ctx, 16) {
            deps.push((dep_path.clone(), parse_text(&dep_text)));
        }
        find_rename(&file, text, at, new_name, buf.as_path(), &deps, &ctx)
    }

    #[test]
    fn valid_names_pass_and_keywords_fail() {
        assert!(is_valid_rename("total"));
        assert!(is_valid_rename("_x"));
        assert!(is_valid_rename("$a"));
        assert!(!is_valid_rename(""));
        assert!(!is_valid_rename("1abc"));
        assert!(!is_valid_rename("a-b"));
        assert!(!is_valid_rename("a b"));
        assert!(!is_valid_rename("contract"));
        assert!(!is_valid_rename("function"));
        assert!(!is_valid_rename("uint8"));
        assert!(!is_valid_rename("wei"));
        assert!(!is_valid_rename("var"));
    }

    #[test]
    fn local_var_lists_decl_and_uses() {
        let text = "contract C {\nfunction f() public {\nuint x = 1;\nx;\nx;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let found = rename_in(&workspace, "/A.sol", text, "x;\nx;", "total").expect("rename");
        assert_eq!(found.len(), 3);
        assert_eq!(found.iter().filter(|item| item.is_declaration).count(), 1);
    }

    #[test]
    fn keyword_new_name_returns_none() {
        let text = "contract C {\nfunction f() public {\nuint x = 1;\nx;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        assert!(rename_in(&workspace, "/A.sol", text, "x;", "contract").is_none());
        assert!(rename_in(&workspace, "/A.sol", text, "x;", "").is_none());
        assert!(rename_in(&workspace, "/A.sol", text, "x;", "1abc").is_none());
    }

    #[test]
    fn unknown_word_returns_none() {
        let text = "contract C {\nfunction f() public {\nmissing;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let at = text.rfind("missing").expect("word") as u32;
        let buf = PathBuf::from("/A.sol");
        let ctx = ProjectContext::new(buf.as_path(), text, at, &workspace);
        let file = parse_text(text);
        assert!(find_rename(&file, text, at, "total", buf.as_path(), &[], &ctx).is_none());
    }

    #[test]
    fn builtin_member_returns_none() {
        let text = "contract C {\nfunction f() public {\naddress a;\na.balance;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        assert!(rename_in(&workspace, "/A.sol", text, "balance", "total").is_none());
    }

    #[test]
    fn cross_file_function_spans_two_files() {
        let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
        let a =
            "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\nwithdraw;\n}\n}";
        let mut workspace = workspace_with(&[("/A.sol", a), ("/B.sol", b)]);
        workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
        let found = rename_in(&workspace, "/A.sol", a, "withdraw;", "settle").expect("rename");
        assert!(found.len() >= 3, "found: {found:?}");
        assert!(
            found
                .iter()
                .any(|item| item.path.as_path() == std::path::Path::new("/B.sol"))
        );
        assert!(
            found
                .iter()
                .any(|item| item.path.as_path() == std::path::Path::new("/A.sol"))
        );
    }

    #[test]
    fn member_field_lists_decl_and_dot_uses() {
        let text = "struct Person {\nuint age;\n}\ncontract C {\nfunction f() public {\nPerson p;\np.age;\np.age;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let found = rename_in(&workspace, "/A.sol", text, "p.age;", "ageNew").expect("rename");
        assert_eq!(found.len(), 3, "found: {found:?}");
    }

    #[test]
    fn emit_event_lists_decl_and_emit_uses() {
        let text = "contract C {\nevent Transfer(address a);\nfunction f() public {\nemit Transfer(a);\nemit Transfer(a);\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let found = rename_in(&workspace, "/A.sol", text, "Transfer(a);", "Sent").expect("rename");
        assert_eq!(found.len(), 3, "found: {found:?}");
    }

    #[test]
    fn analyzer_rejects_keyword_and_accepts_ident() {
        let text = "contract C {\nfunction f() public {\nuint x = 1;\nx;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let buf = PathBuf::from("/A.sol");
        let at = text.rfind("x;").expect("use") as u32;
        let ctx = ProjectContext::new(buf.as_path(), text, at, &workspace);
        assert!(
            crate::analysis::SolidityAnalyzer
                .rename(&ctx, "contract")
                .is_none()
        );
        assert!(
            crate::analysis::SolidityAnalyzer
                .rename(&ctx, "total")
                .is_some()
        );
    }
}
