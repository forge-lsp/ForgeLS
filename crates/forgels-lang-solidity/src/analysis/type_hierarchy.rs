use std::path::{Path, PathBuf};

use forgels_core::{MAX_TYPE_HIERARCHY, ProjectContext, TypeHierarchyItemData, routing::Language};
use forgels_syntax::{AstNode, Lexer};

use super::inheritance;
use crate::{SolidityFile, SolidityLexer, SolidityParser};

const CAP: usize = MAX_TYPE_HIERARCHY;

fn parse_text(text: &str) -> SolidityFile {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    file
}

fn contract_at(file: &SolidityFile, offset: u32) -> Option<crate::ContractDef> {
    for contract in file.contracts() {
        let span = contract.syntax().span;
        if span.contains(offset) || span.end == offset {
            return Some(contract);
        }
    }
    None
}

fn item_for(path: PathBuf, contract: &crate::ContractDef) -> Option<TypeHierarchyItemData> {
    let name = contract.name()?;
    if name.is_empty() {
        return None;
    }
    let full = contract.syntax().span;
    let selection = contract.name_span().unwrap_or(full);
    let mut detail = String::from(contract.kind_label());
    detail.push(' ');
    detail.push_str(&name);
    if !contract.heritage().is_empty() {
        detail.push_str(" is ");
        detail.push_str(&contract.heritage().join(", "));
    }
    Some(TypeHierarchyItemData::new(
        name,
        forgels_protocol::SymbolKind::CLASS,
        detail,
        path,
        full,
        selection,
    ))
}

/// Contract at `offset` as a type hierarchy item.
///
/// Returns `None` outside any contract or for nameless contracts.
/// `span` is the full contract range, `selection` is the name span.
pub fn prepare_type_in(
    file: &SolidityFile,
    offset: u32,
    current: &Path,
) -> Option<TypeHierarchyItemData> {
    let contract = contract_at(file, offset)?;
    item_for(current.to_path_buf(), &contract)
}

/// Base contracts of the contract at `offset`, heritage order.
///
/// Uses `linearize` for same file bases plus `linearize_across` for
/// dep bases. Dedupes by name, first base wins. Returns `Some` with a
/// possibly empty list when the cursor sits on a contract, `None`
/// otherwise. Caps at `MAX_TYPE_HIERARCHY`.
pub fn supertypes_in(
    file: &SolidityFile,
    offset: u32,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<Vec<TypeHierarchyItemData>> {
    let contract = contract_at(file, offset)?;
    let name = contract.name()?;
    if name.is_empty() {
        return None;
    }
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<TypeHierarchyItemData> = Vec::new();
    for base in inheritance::linearize(file, &name) {
        if out.len() >= CAP {
            break;
        }
        let Some(base_name) = base.name() else {
            continue;
        };
        if base_name.is_empty() || seen.contains(&base_name) {
            continue;
        }
        seen.push(base_name);
        let Some(item) = item_for(current.to_path_buf(), &base) else {
            continue;
        };
        out.push(item);
    }
    for (base, path) in inheritance::linearize_across(file, &name, deps) {
        if out.len() >= CAP {
            break;
        }
        let Some(base_name) = base.name() else {
            continue;
        };
        if base_name.is_empty() || seen.contains(&base_name) {
            continue;
        }
        seen.push(base_name);
        let owner = path.unwrap_or_else(|| current.to_path_buf());
        let Some(item) = item_for(owner, &base) else {
            continue;
        };
        out.push(item);
    }
    Some(out)
}

/// Derived contracts of the contract at `offset` across tracked files.
///
/// Scans tracked Solidity files path-sorted and keeps contracts whose
/// heritage holds the exact target name. Exact match only: prefixes
/// and case variants never count. Skips the target itself and
/// out-of-bounds spans. Caps at `MAX_TYPE_HIERARCHY`.
pub fn subtypes_in(
    file: &SolidityFile,
    offset: u32,
    current: &Path,
    ctx: &ProjectContext,
) -> Option<Vec<TypeHierarchyItemData>> {
    let contract = contract_at(file, offset)?;
    let target = contract.name()?;
    if target.is_empty() {
        return None;
    }
    let target_span = contract.syntax().span;
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    for (path, doc) in ctx.workspace.iter_files() {
        if doc.language != Language::Solidity {
            continue;
        }
        if path.as_path() == current {
            files.push((path.clone(), ctx.text.to_string()));
        } else {
            files.push((path.clone(), doc.text.clone()));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut out: Vec<TypeHierarchyItemData> = Vec::new();
    for (path, text) in &files {
        if out.len() >= CAP {
            break;
        }
        if text
            .get(target_span.start as usize..target_span.end as usize)
            .is_none()
            && path.as_path() == current
        {
            continue;
        }
        let candidate = parse_text(text);
        for item in candidate.contracts() {
            if out.len() >= CAP {
                break;
            }
            if !item.heritage().iter().any(|base| base == &target) {
                continue;
            }
            if path.as_path() == current && item.syntax().span == target_span {
                continue;
            }
            let full = item.syntax().span;
            if text.get(full.start as usize..full.end as usize).is_none() {
                continue;
            }
            let Some(entry) = item_for(path.clone(), &item) else {
                continue;
            };
            if out.iter().any(|known: &TypeHierarchyItemData| {
                known.path == entry.path && known.span == entry.span
            }) {
                continue;
            }
            out.push(entry);
        }
    }
    out.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.span.start.cmp(&right.span.start))
    });
    out.truncate(CAP);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn prepare_resolves_contract() {
        let text = "contract Vault {\nuint x;\n}";
        let file = parse_text(text);
        let at = text.find("Vault").expect("name") as u32;
        let found = prepare_type_in(&file, at, Path::new("/A.sol")).expect("item");
        assert_eq!(found.name, "Vault");
        assert_eq!(found.detail, "contract Vault");
    }

    #[test]
    fn prepare_outside_returns_none() {
        let text = "// note\ncontract Vault {\nuint x;\n}";
        let file = parse_text(text);
        assert!(prepare_type_in(&file, 2, Path::new("/A.sol")).is_none());
    }

    #[test]
    fn supertypes_linearize_heritage_order() {
        let text = "contract A {}\ncontract B is A {}\ncontract C is B {}\n";
        let file = parse_text(text);
        let at = text.rfind("C is").expect("derived") as u32;
        let found = supertypes_in(&file, at, Path::new("/A.sol"), &[]).expect("bases");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "B");
        assert_eq!(found[1].name, "A");
    }

    #[test]
    fn subtypes_scan_tracked_exact_match() {
        let base = "contract Vault {\nuint x;\n}";
        let derived = "contract Safe is Vault {\nuint y;\n}";
        let other = "contract Vaulted {\nuint z;\n}";
        let workspace = workspace_with(&[("/A.sol", base), ("/B.sol", derived), ("/C.sol", other)]);
        let buf = PathBuf::from("/A.sol");
        let ctx = ProjectContext::new(buf.as_path(), base, 10, &workspace);
        let file = parse_text(base);
        let found = subtypes_in(&file, 10, buf.as_path(), &ctx).expect("derived");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Safe");
        assert_eq!(found[0].path, PathBuf::from("/B.sol"));
    }
}
