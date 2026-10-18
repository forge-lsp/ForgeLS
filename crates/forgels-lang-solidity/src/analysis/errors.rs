use std::collections::HashMap;
use std::path::PathBuf;

use forgels_core::DiagnosticData;
use forgels_syntax::{AstNode, Span};

use super::{Symbol, SymbolKind, inheritance};
use crate::SolidityFile;

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn is_revert_word(line: &str, start: usize) -> bool {
    let bytes = line.as_bytes();
    if line.get(start..start + 6) != Some("revert") {
        return false;
    }
    if start > 0 && is_word_byte(bytes[start - 1]) {
        return false;
    }
    if let Some(next) = bytes.get(start + 6)
        && is_word_byte(*next)
    {
        return false;
    }
    true
}

/// True when `offset` sits in the error name slot after `revert`.
pub fn is_revert_name_context(text: &str, offset: u32) -> bool {
    let at = (offset as usize).min(text.len());
    let head = match text.get(..at) {
        Some(part) => part,
        None => return false,
    };
    let line_start = head.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let line = match text.get(line_start..at) {
        Some(part) => part,
        None => return false,
    };
    let mut last_revert: Option<usize> = None;
    let mut index = 0;
    while index + 6 <= line.len() {
        if is_revert_word(line, index) {
            last_revert = Some(index);
        }
        index += 1;
    }
    let Some(found) = last_revert else {
        return false;
    };
    let after = match line.get(found + 6..) {
        Some(part) => part,
        None => return false,
    };
    if after.contains('(') || after.contains(';') {
        return false;
    }
    true
}

/// Error name under the cursor when inside a revert name slot.
pub fn revert_name_at(text: &str, offset: u32) -> Option<(String, Span)> {
    if !is_revert_name_context(text, offset) {
        return None;
    }
    super::word_at(text, offset)
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

fn file_error_symbol(file: &SolidityFile, name: &str) -> Option<Symbol> {
    for item in file.errors() {
        if item.name().as_deref() == Some(name) {
            let mut detail = String::from("error ");
            detail.push_str(name);
            return Some(Symbol::new(
                name.to_string(),
                SymbolKind::Error,
                item.syntax().span,
                detail,
            ));
        }
    }
    None
}

/// Local error for `name` visible at `offset`: own contract first,
/// then same file bases in linear order, then file level errors.
pub fn resolve_local_error(file: &SolidityFile, offset: u32, name: &str) -> Option<Symbol> {
    if name.is_empty() {
        return None;
    }
    if let Some(contract) = contract_at(file, offset) {
        for item in contract.errors() {
            if item.name().as_deref() == Some(name) {
                let mut detail = String::from("error ");
                detail.push_str(name);
                return Some(Symbol::new(
                    name.to_string(),
                    SymbolKind::Error,
                    item.syntax().span,
                    detail,
                ));
            }
        }
        let Some(contract_name) = contract.name() else {
            return file_error_symbol(file, name);
        };
        if contract_name.is_empty() {
            return file_error_symbol(file, name);
        }
        for base in inheritance::linearize(file, &contract_name) {
            let Some(base_name) = base.name() else {
                continue;
            };
            if base_name.is_empty() {
                continue;
            }
            for item in base.errors() {
                if item.name().as_deref() == Some(name) {
                    let mut detail = String::from("error ");
                    detail.push_str(name);
                    detail.push_str(" - from ");
                    detail.push_str(&base_name);
                    return Some(Symbol::new(
                        name.to_string(),
                        SymbolKind::Error,
                        item.syntax().span,
                        detail,
                    ));
                }
            }
        }
        return file_error_symbol(file, name);
    }
    file_error_symbol(file, name)
}

/// First error named `name` inside a dep file: file level first,
/// then each contract in order.
pub fn find_error_in_dep(file: &SolidityFile, name: &str) -> Option<Symbol> {
    if name.is_empty() {
        return None;
    }
    if let Some(found) = file_error_symbol(file, name) {
        return Some(found);
    }
    for contract in file.contracts() {
        for item in contract.errors() {
            if item.name().as_deref() == Some(name) {
                let mut detail = String::from("error ");
                detail.push_str(name);
                return Some(Symbol::new(
                    name.to_string(),
                    SymbolKind::Error,
                    item.syntax().span,
                    detail,
                ));
            }
        }
    }
    None
}

/// Unknown error errors: named reverts must resolve locally or in deps.
///
/// Same resolve order as hover: `resolve_local_error` first, then
/// `find_error_in_dep` in index order. Bare `revert` stays silent so
/// string reasons never lint. Untracked deps stay missing by design.
pub fn unknown_error_diagnostics(
    file: &SolidityFile,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for revert in body.all_reverts() {
                let Some(name) = revert.name() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let Some(name_span) = revert.name_span() else {
                    continue;
                };
                if resolve_local_error(file, name_span.start, &name).is_some() {
                    continue;
                }
                let mut found = false;
                for (_, dep_file) in deps {
                    if find_error_in_dep(dep_file, &name).is_some() {
                        found = true;
                        break;
                    }
                }
                if found {
                    continue;
                }
                out.push(DiagnosticData {
                    span: name_span,
                    message: format!("unresolved error '{name}'"),
                });
            }
        }
    }
    out
}

/// Arity checks for revert statements: declared error params against
/// supplied args.
///
/// File level errors plus own contract plus same file bases in linear
/// order. Derived wins, first base wins. Unknown names stay silent.
pub fn revert_arity_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    let mut file_params: HashMap<String, usize> = HashMap::new();
    for item in file.errors() {
        let Some(name) = item.name() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        file_params.entry(name).or_insert(item.params().len());
    }
    for contract in file.contracts() {
        let Some(contract_name) = contract.name() else {
            continue;
        };
        if contract_name.is_empty() {
            continue;
        }
        let mut params_of: HashMap<String, usize> = HashMap::new();
        for item in contract.errors() {
            let Some(name) = item.name() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            params_of.entry(name).or_insert(item.params().len());
        }
        for base in inheritance::linearize(file, &contract_name) {
            for item in base.errors() {
                let Some(name) = item.name() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                params_of.entry(name).or_insert(item.params().len());
            }
        }
        for (name, count) in &file_params {
            params_of.entry(name.clone()).or_insert(*count);
        }
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for revert in body.all_reverts() {
                let Some(name) = revert.name() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let Some(expected) = params_of.get(&name) else {
                    continue;
                };
                let actual = revert.arg_count();
                if *expected != actual {
                    out.push(DiagnosticData {
                        span: revert.syntax().span,
                        message: format!(
                            "this error '{name}' takes {expected} arguments but {actual} supplied"
                        ),
                    });
                }
            }
        }
    }
    out
}
