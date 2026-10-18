use super::{
    Scope, SymbolKind, address, array, bytes, forge, function, globals, inheritance, members, meta,
};
use crate::SolidityFile;
use forgels_core::DiagnosticData;
use forgels_syntax::AstNode;
use std::collections::HashMap;
use std::path::PathBuf;

/// Arity checks for plain calls: compare declared params with supplied args.
///
/// Inherited functions count via heritage order linearization, derived
/// wins and first base wins. Unknown names and non-function symbols stay
/// silent. Overloads use the first definition, matching scope order.
pub fn arity_diagnostics(file: &SolidityFile, scope: &Scope) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        let Some(contract_name) = contract.name() else {
            continue;
        };
        if contract_name.is_empty() {
            continue;
        }
        let mut params_of: HashMap<String, usize> = HashMap::new();
        for function in contract.functions() {
            let Some(name) = function.name() else {
                continue;
            };
            params_of
                .entry(name)
                .or_insert_with(|| function.params().len());
        }
        for base in inheritance::linearize(file, &contract_name) {
            for function in base.functions() {
                let Some(name) = function.name() else {
                    continue;
                };
                params_of
                    .entry(name)
                    .or_insert_with(|| function.params().len());
            }
        }
        let mut getter_of: HashMap<String, usize> = HashMap::new();
        for getter in super::getter::contract_getters(&contract) {
            getter_of
                .entry(getter.name.clone())
                .or_insert(getter.params.len());
        }
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for call in body.all_calls() {
                let Some(callee) = call.callee() else {
                    continue;
                };
                let span = call.syntax().span;
                let Some(symbol) = scope.resolve(&callee, span.start) else {
                    continue;
                };
                let expected = if symbol.kind == SymbolKind::Function {
                    params_of.get(&callee).or_else(|| getter_of.get(&callee))
                } else if symbol.kind == SymbolKind::StateVar {
                    getter_of.get(&callee)
                } else {
                    continue;
                };
                let Some(expected) = expected else {
                    continue;
                };
                let actual = call.arg_count();
                if *expected != actual {
                    out.push(DiagnosticData {
                        span,
                        message: format!(
                            "this function '{callee}' takes {expected} arguments but {actual} supplied"
                        ),
                    });
                }
            }
        }
    }
    out
}

/// Arity checks for member calls: `recv.member(args)` against the receiver type.
///
/// Builtins skip first via `member_builtin`, so `vm.*` and `console.*`
/// stay silent. Receiver resolves with deps, structs stay silent, unknown
/// types stay silent. Arrays check `push` (1) and `pop` (0) on dynamic
/// only: `length` calls and fixed array calls stay silent, unknown names
/// stay silent. Addresses check `transfer`, `send`, `call`,
/// `delegatecall`, and `staticcall` (1 each): `balance`, `code`, and
/// `codehash` calls stay silent, unknown names stay silent. Functions
/// check `gas` and `value` (1 each): `selector` calls stay silent.
/// Globals check `block.blobhash` (1), `abi.decode` (2), and
/// `abi.encodeCall` (2): fields and variadic `abi.encode*` stay silent.
/// Bytes `length`, string members, and `type()` metadata stay silent.
/// Target functions collect own plus linear order bases
/// across files, derived wins and first base wins. Same message as plain.
pub fn member_arity_diagnostics(
    file: &SolidityFile,
    scope: &Scope,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for member in body.all_members() {
                if !member.is_call() {
                    continue;
                }
                let Some(receiver) = member.receiver() else {
                    continue;
                };
                let Some(name) = member.member() else {
                    continue;
                };
                if receiver.is_empty() || name.is_empty() {
                    continue;
                }
                let offset = member.syntax().span.start;
                if forge::member_builtin(&receiver, &name, scope, offset).is_some() {
                    continue;
                }
                let kind = members::receiver_type_with_deps(file, scope, offset, &receiver, deps);
                if let members::TypeKind::Array { dynamic } = &kind {
                    let Some(expected) = array::array_expected_arity(&name, *dynamic) else {
                        continue;
                    };
                    let actual = member.arg_count();
                    if expected != actual {
                        out.push(DiagnosticData {
                            span: member.syntax().span,
                            message: format!(
                                "this function '{name}' takes {expected} arguments but {actual} supplied"
                            ),
                        });
                    }
                    continue;
                }
                if matches!(kind, members::TypeKind::Address) {
                    let Some(expected) = address::address_expected_arity(&name) else {
                        continue;
                    };
                    let actual = member.arg_count();
                    if expected != actual {
                        out.push(DiagnosticData {
                            span: member.syntax().span,
                            message: format!(
                                "this function '{name}' takes {expected} arguments but {actual} supplied"
                            ),
                        });
                    }
                    continue;
                }
                if matches!(kind, members::TypeKind::Function) {
                    let Some(expected) = function::function_expected_arity(&name) else {
                        continue;
                    };
                    let actual = member.arg_count();
                    if expected != actual {
                        out.push(DiagnosticData {
                            span: member.syntax().span,
                            message: format!(
                                "this function '{name}' takes {expected} arguments but {actual} supplied"
                            ),
                        });
                    }
                    continue;
                }
                if let members::TypeKind::Global(ref receiver_name) = kind {
                    let Some(expected) = globals::global_expected_arity(receiver_name, &name)
                    else {
                        continue;
                    };
                    let actual = member.arg_count();
                    if expected != actual {
                        out.push(DiagnosticData {
                            span: member.syntax().span,
                            message: format!(
                                "this function '{name}' takes {expected} arguments but {actual} supplied"
                            ),
                        });
                    }
                    continue;
                }
                if matches!(kind, members::TypeKind::Bytes | members::TypeKind::String) {
                    let _ = bytes::bytes_expected_arity(&name);
                    continue;
                }
                if let members::TypeKind::Meta(ref inner) = kind {
                    let _ = meta::meta_expected_arity(inner, &name);
                    continue;
                }
                let members::TypeKind::Contract(target) = kind else {
                    if let Some(expected) = super::using::using_expected_arity(
                        file, scope, offset, &receiver, &name, deps,
                    ) {
                        let actual = member.arg_count();
                        if expected != actual {
                            out.push(DiagnosticData {
                                span: member.syntax().span,
                                message: format!(
                                    "this function '{name}' takes {expected} arguments but {actual} supplied"
                                ),
                            });
                        }
                    }
                    continue;
                };
                if target.is_empty() {
                    continue;
                }
                let Some((target_def, _)) = inheritance::find_contract_across(&target, file, deps)
                else {
                    continue;
                };
                let mut params_of: HashMap<String, usize> = HashMap::new();
                for item in target_def.functions() {
                    let Some(item_name) = item.name() else {
                        continue;
                    };
                    if item_name.is_empty() {
                        continue;
                    }
                    params_of
                        .entry(item_name)
                        .or_insert_with(|| item.params().len());
                }
                for base in inheritance::linearize(file, &target) {
                    for item in base.functions() {
                        let Some(item_name) = item.name() else {
                            continue;
                        };
                        if item_name.is_empty() {
                            continue;
                        }
                        params_of
                            .entry(item_name)
                            .or_insert_with(|| item.params().len());
                    }
                }
                for (base, _) in inheritance::linearize_across(file, &target, deps) {
                    for item in base.functions() {
                        let Some(item_name) = item.name() else {
                            continue;
                        };
                        if item_name.is_empty() {
                            continue;
                        }
                        params_of
                            .entry(item_name)
                            .or_insert_with(|| item.params().len());
                    }
                }
                for getter in super::getter::contract_getters(&target_def) {
                    params_of
                        .entry(getter.name.clone())
                        .or_insert(getter.params.len());
                }
                let Some(expected) = params_of.get(&name).copied().or_else(|| {
                    super::using::using_expected_arity(file, scope, offset, &receiver, &name, deps)
                }) else {
                    continue;
                };
                let actual = member.arg_count();
                if expected != actual {
                    out.push(DiagnosticData {
                        span: member.syntax().span,
                        message: format!(
                            "this function '{name}' takes {expected} arguments but {actual} supplied"
                        ),
                    });
                }
            }
        }
    }
    out
}
