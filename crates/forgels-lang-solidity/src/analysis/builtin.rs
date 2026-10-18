use forgels_core::DiagnosticData;
use forgels_syntax::AstNode;

use super::Scope;
use crate::SolidityFile;

/// Fixed arity for plain Solidity globals: name plus arg count.
///
/// Counts follow the Solidity units and globals reference: `gasleft`
/// takes none, the three hashes take one, `ecrecover` takes four,
/// `addmod` and `mulmod` take three, `blockhash` and `selfdestruct`
/// take one, `assert` takes one. Only verified entries sit here;
/// anything uncertain stays out and silent.
pub(crate) const PLAIN_BUILTIN_ARITY: &[(&str, usize)] = &[
    ("gasleft", 0),
    ("keccak256", 1),
    ("sha256", 1),
    ("ripemd160", 1),
    ("ecrecover", 4),
    ("addmod", 3),
    ("mulmod", 3),
    ("blockhash", 1),
    ("selfdestruct", 1),
    ("assert", 1),
];

/// Expected arity for a plain builtin name, or `None` when not fixed.
///
/// `require` and `revert` accept several shapes so they stay silent.
/// Forge asserts, `vm`, `console`, `abi`, and `type()` members are
/// owned by their own checks and stay silent here.
pub(crate) fn plain_builtin_arity(name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    PLAIN_BUILTIN_ARITY
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, arity)| *arity)
}

/// Arity checks for plain builtin calls.
///
/// User definitions win: any scope hit skips the builtin row.
/// Unknown names stay silent via the allowlist path in `unknown`.
/// Same message shape as `calls::arity_diagnostics`.
pub fn builtin_arity_diagnostics(file: &SolidityFile, scope: &Scope) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for call in body.all_calls() {
                let Some(callee) = call.callee() else {
                    continue;
                };
                if callee.is_empty() {
                    continue;
                }
                let offset = call.syntax().span.start;
                if scope.resolve(&callee, offset).is_some() {
                    continue;
                }
                let Some(expected) = plain_builtin_arity(&callee) else {
                    continue;
                };
                let actual = call.arg_count();
                if expected != actual {
                    out.push(DiagnosticData {
                        span: call.syntax().span,
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
