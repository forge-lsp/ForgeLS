use std::collections::HashMap;
use std::path::PathBuf;

use forgels_core::DiagnosticData;
use forgels_syntax::{AstNode, Span};

use super::{Symbol, SymbolKind, inheritance};
use crate::SolidityFile;

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn is_emit_word(line: &str, start: usize) -> bool {
    let bytes = line.as_bytes();
    if line.get(start..start + 4) != Some("emit") {
        return false;
    }
    if start > 0 && is_word_byte(bytes[start - 1]) {
        return false;
    }
    if let Some(next) = bytes.get(start + 4)
        && is_word_byte(*next)
    {
        return false;
    }
    true
}

/// True when `offset` sits in the event name slot after `emit`.
///
/// Line scan only: finds the last standalone `emit` on the line and
/// rejects positions past `(` or `;`. Keywords stay hidden here, only
/// events complete.
pub fn is_emit_name_context(text: &str, offset: u32) -> bool {
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
    let mut last_emit: Option<usize> = None;
    let mut index = 0;
    while index + 4 <= line.len() {
        if is_emit_word(line, index) {
            last_emit = Some(index);
        }
        index += 1;
    }
    let Some(found) = last_emit else {
        return false;
    };
    let after = match line.get(found + 4..) {
        Some(part) => part,
        None => return false,
    };
    if after.contains('(') || after.contains(';') {
        return false;
    }
    true
}

/// Event name under the cursor when inside an emit name slot.
pub fn emit_name_at(text: &str, offset: u32) -> Option<(String, Span)> {
    if !is_emit_name_context(text, offset) {
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

fn file_event_symbol(file: &SolidityFile, name: &str) -> Option<Symbol> {
    for item in file.events() {
        if item.name().as_deref() == Some(name) {
            let mut detail = String::from("event ");
            detail.push_str(name);
            return Some(Symbol::new(
                name.to_string(),
                SymbolKind::Event,
                item.syntax().span,
                detail,
            ));
        }
    }
    None
}

/// Local event for `name` visible at `offset`: own contract first,
/// then same file bases in linear order, then file level events.
pub fn resolve_local_emit(file: &SolidityFile, offset: u32, name: &str) -> Option<Symbol> {
    if name.is_empty() {
        return None;
    }
    if let Some(contract) = contract_at(file, offset) {
        for item in contract.events() {
            if item.name().as_deref() == Some(name) {
                let mut detail = String::from("event ");
                detail.push_str(name);
                return Some(Symbol::new(
                    name.to_string(),
                    SymbolKind::Event,
                    item.syntax().span,
                    detail,
                ));
            }
        }
        let Some(contract_name) = contract.name() else {
            return file_event_symbol(file, name);
        };
        if contract_name.is_empty() {
            return file_event_symbol(file, name);
        }
        for base in inheritance::linearize(file, &contract_name) {
            let Some(base_name) = base.name() else {
                continue;
            };
            if base_name.is_empty() {
                continue;
            }
            for item in base.events() {
                if item.name().as_deref() == Some(name) {
                    let mut detail = String::from("event ");
                    detail.push_str(name);
                    detail.push_str(" - from ");
                    detail.push_str(&base_name);
                    return Some(Symbol::new(
                        name.to_string(),
                        SymbolKind::Event,
                        item.syntax().span,
                        detail,
                    ));
                }
            }
        }
        return file_event_symbol(file, name);
    }
    file_event_symbol(file, name)
}

/// First event named `name` inside a dep file: file level first,
/// then each contract in order. Same file bases inside the dep count
/// because export order already covers them via scope.
pub fn find_emit_in_dep(file: &SolidityFile, name: &str) -> Option<Symbol> {
    if name.is_empty() {
        return None;
    }
    if let Some(found) = file_event_symbol(file, name) {
        return Some(found);
    }
    for contract in file.contracts() {
        for item in contract.events() {
            if item.name().as_deref() == Some(name) {
                let mut detail = String::from("event ");
                detail.push_str(name);
                return Some(Symbol::new(
                    name.to_string(),
                    SymbolKind::Event,
                    item.syntax().span,
                    detail,
                ));
            }
        }
    }
    None
}

/// Event names visible at `offset` for completion: own contract, same
/// file bases, then file level. Derived wins, first base wins.
pub fn emit_candidates(file: &SolidityFile, offset: u32) -> Vec<Symbol> {
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<Symbol> = Vec::new();
    let mut push = |name: String, span: Span, detail: String| {
        if name.is_empty() || seen.contains(&name) {
            return;
        }
        seen.push(name.clone());
        out.push(Symbol::new(name, SymbolKind::Event, span, detail));
    };
    let Some(contract) = contract_at(file, offset) else {
        for item in file.events() {
            let Some(name) = item.name() else {
                continue;
            };
            let mut detail = String::from("event ");
            detail.push_str(&name);
            push(name, item.syntax().span, detail);
        }
        return out;
    };
    for item in contract.events() {
        let Some(name) = item.name() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let mut detail = String::from("event ");
        detail.push_str(&name);
        push(name, item.syntax().span, detail);
    }
    let Some(contract_name) = contract.name() else {
        return out;
    };
    if contract_name.is_empty() {
        return out;
    }
    for base in inheritance::linearize(file, &contract_name) {
        let Some(base_name) = base.name() else {
            continue;
        };
        if base_name.is_empty() {
            continue;
        }
        for item in base.events() {
            let Some(name) = item.name() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let mut detail = String::from("event ");
            detail.push_str(&name);
            detail.push_str(" - from ");
            detail.push_str(&base_name);
            push(name, item.syntax().span, detail);
        }
    }
    for item in file.events() {
        let Some(name) = item.name() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let mut detail = String::from("event ");
        detail.push_str(&name);
        push(name, item.syntax().span, detail);
    }
    out
}

/// Dep event candidates with ` - from file` origin, deduped against
/// local names. Deps come from `dep_texts` in index order.
pub fn dep_emit_candidates(
    local: &[Symbol],
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<(Symbol, String)> {
    let mut seen: Vec<String> = local.iter().map(|item| item.name.clone()).collect();
    let mut out: Vec<(Symbol, String)> = Vec::new();
    for (path, file) in deps {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        for item in file.events() {
            let Some(name) = item.name() else {
                continue;
            };
            if name.is_empty() || seen.contains(&name) {
                continue;
            }
            seen.push(name.clone());
            let mut detail = String::from("event ");
            detail.push_str(&name);
            detail.push_str(" - from ");
            detail.push_str(file_name);
            out.push((
                Symbol::new(
                    name.clone(),
                    SymbolKind::Event,
                    item.syntax().span,
                    detail.clone(),
                ),
                path.to_string_lossy().into_owned(),
            ));
        }
        for contract in file.contracts() {
            for item in contract.events() {
                let Some(name) = item.name() else {
                    continue;
                };
                if name.is_empty() || seen.contains(&name) {
                    continue;
                }
                seen.push(name.clone());
                let mut detail = String::from("event ");
                detail.push_str(&name);
                detail.push_str(" - from ");
                detail.push_str(file_name);
                out.push((
                    Symbol::new(
                        name.clone(),
                        SymbolKind::Event,
                        item.syntax().span,
                        detail.clone(),
                    ),
                    path.to_string_lossy().into_owned(),
                ));
            }
        }
    }
    out
}

/// Unknown event errors: named emits must resolve locally or in deps.
///
/// Same resolve order as hover: `resolve_local_emit` first (own contract,
/// same file bases, file level), then `find_emit_in_dep` in index order.
/// Bare `emit` stays silent so completion can offer names. Untracked deps
/// stay missing by construction: dep lists only carry tracked files, so
/// untracked names still error here while the import reports separately.
pub fn unknown_emit_diagnostics(
    file: &SolidityFile,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for emit in body.emits() {
                let Some(name) = emit.name() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let Some(name_span) = emit.name_span() else {
                    continue;
                };
                if resolve_local_emit(file, name_span.start, &name).is_some() {
                    continue;
                }
                let mut found = false;
                for (_, dep_file) in deps {
                    if find_emit_in_dep(dep_file, &name).is_some() {
                        found = true;
                        break;
                    }
                }
                if found {
                    continue;
                }
                out.push(DiagnosticData {
                    span: name_span,
                    message: format!("unresolved event '{name}'"),
                });
            }
        }
    }
    out
}

/// Arity checks for emit statements: declared event params against
/// supplied args.
///
/// File level events plus own contract plus same file bases in linear
/// order. Derived wins, first base wins. Unknown names stay silent.
pub fn emit_arity_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    let mut file_params: HashMap<String, usize> = HashMap::new();
    for item in file.events() {
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
        for item in contract.events() {
            let Some(name) = item.name() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            params_of.entry(name).or_insert(item.params().len());
        }
        for base in inheritance::linearize(file, &contract_name) {
            for item in base.events() {
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
            for emit in body.emits() {
                let Some(name) = emit.name() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let Some(expected) = params_of.get(&name) else {
                    continue;
                };
                let actual = emit.arg_count();
                if *expected != actual {
                    out.push(DiagnosticData {
                        span: emit.syntax().span,
                        message: format!(
                            "this event '{name}' takes {expected} arguments but {actual} supplied"
                        ),
                    });
                }
            }
        }
    }
    out
}
