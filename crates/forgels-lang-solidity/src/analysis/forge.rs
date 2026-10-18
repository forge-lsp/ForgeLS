use std::collections::HashSet;

use forgels_core::DiagnosticData;
use forgels_lint::RuleId;
use forgels_syntax::AstNode;

use super::Scope;
use crate::{ContractDef, SolidityFile};

/// Origin tag for forge-std builtins in completion and hover details.
pub(crate) const FORGE_ORIGIN: &str = "forge-std";

/// One forge-std builtin: display detail plus plain arity.
///
/// `arity` is `Some` for plain asserts only. Member cheatcodes (`vm.*`,
/// `console.*`) keep `None` by design: member arity skips builtins via
/// guard, so cheatcode overloads never lint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ForgeBuiltin {
    pub name: &'static str,
    pub detail: &'static str,
    pub arity: Option<usize>,
}

/// Subset of the `Vm` cheatcode interface, reached as `vm.<name>`.
pub(crate) const VM_TABLE: &[ForgeBuiltin] = &[
    ForgeBuiltin {
        name: "prank",
        detail: "function prank(address msgSender)",
        arity: None,
    },
    ForgeBuiltin {
        name: "startPrank",
        detail: "function startPrank(address msgSender)",
        arity: None,
    },
    ForgeBuiltin {
        name: "stopPrank",
        detail: "function stopPrank()",
        arity: None,
    },
    ForgeBuiltin {
        name: "warp",
        detail: "function warp(uint256 newTimestamp)",
        arity: None,
    },
    ForgeBuiltin {
        name: "roll",
        detail: "function roll(uint256 newHeight)",
        arity: None,
    },
    ForgeBuiltin {
        name: "deal",
        detail: "function deal(address who, uint256 newBalance)",
        arity: None,
    },
    ForgeBuiltin {
        name: "label",
        detail: "function label(address account, string label)",
        arity: None,
    },
    ForgeBuiltin {
        name: "mockCall",
        detail: "function mockCall(address where, bytes data)",
        arity: None,
    },
    ForgeBuiltin {
        name: "expectRevert",
        detail: "function expectRevert()",
        arity: None,
    },
    ForgeBuiltin {
        name: "expectEmit",
        detail: "function expectEmit(bool checkTopic1, bool checkTopic2, bool checkTopic3, bool checkData)",
        arity: None,
    },
];

/// Subset of `Test` asserts, reached as plain calls with `Test` heritage.
pub(crate) const ASSERT_TABLE: &[ForgeBuiltin] = &[
    ForgeBuiltin {
        name: "assertTrue",
        detail: "function assertTrue(bool condition)",
        arity: Some(1),
    },
    ForgeBuiltin {
        name: "assertFalse",
        detail: "function assertFalse(bool condition)",
        arity: Some(1),
    },
    ForgeBuiltin {
        name: "assertEq",
        detail: "function assertEq(uint256 left, uint256 right)",
        arity: Some(2),
    },
    ForgeBuiltin {
        name: "assertNotEq",
        detail: "function assertNotEq(uint256 left, uint256 right)",
        arity: Some(2),
    },
    ForgeBuiltin {
        name: "assertGt",
        detail: "function assertGt(uint256 left, uint256 right)",
        arity: Some(2),
    },
    ForgeBuiltin {
        name: "assertLt",
        detail: "function assertLt(uint256 left, uint256 right)",
        arity: Some(2),
    },
];

/// Subset of the `console` library, reached as `console.<name>`.
pub(crate) const CONSOLE_TABLE: &[ForgeBuiltin] = &[
    ForgeBuiltin {
        name: "log",
        detail: "function log(string message)",
        arity: None,
    },
    ForgeBuiltin {
        name: "logUint",
        detail: "function logUint(uint256 value)",
        arity: None,
    },
    ForgeBuiltin {
        name: "logAddress",
        detail: "function logAddress(address account)",
        arity: None,
    },
];

fn find_in(table: &'static [ForgeBuiltin], name: &str) -> Option<&'static ForgeBuiltin> {
    if name.is_empty() {
        return None;
    }
    table.iter().find(|builtin| builtin.name == name)
}

pub(crate) fn find_vm(name: &str) -> Option<&'static ForgeBuiltin> {
    find_in(VM_TABLE, name)
}

pub(crate) fn find_console(name: &str) -> Option<&'static ForgeBuiltin> {
    find_in(CONSOLE_TABLE, name)
}

pub(crate) fn find_assert(name: &str) -> Option<&'static ForgeBuiltin> {
    find_in(ASSERT_TABLE, name)
}

/// Display detail with origin, for example `function prank(...) - from forge-std`.
pub(crate) fn builtin_detail(builtin: &ForgeBuiltin) -> String {
    let mut out = String::from(builtin.detail);
    out.push_str(" - from ");
    out.push_str(FORGE_ORIGIN);
    out
}

/// True when `contract` inherits `Test` directly or through same file bases.
///
/// `Test` itself lives in forge-std outside the file, so the walk ends at
/// unknown bases. Diamonds and cycles collapse via the visited set.
pub(crate) fn inherits_test(file: &SolidityFile, contract: &ContractDef) -> bool {
    let mut seen: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = contract.heritage();
    while let Some(base) = stack.pop() {
        if base == "Test" {
            return true;
        }
        if base.is_empty() || !seen.insert(base.clone()) {
            continue;
        }
        if let Some(def) = super::inheritance::find_contract(file, &base) {
            stack.extend(def.heritage());
        }
    }
    false
}

/// True when `offset` sits inside a contract that inherits `Test`.
pub(crate) fn test_context(file: &SolidityFile, offset: u32) -> bool {
    for contract in file.contracts() {
        let span = contract.syntax().span;
        if span.contains(offset) || span.end == offset {
            return inherits_test(file, &contract);
        }
    }
    false
}

/// Member builtin for `receiver.member`, or `None` when shadowed or unknown.
///
/// A scope entry named `receiver` means user code owns it, so the builtin
/// stays hidden and normal member lookup runs instead.
pub(crate) fn member_builtin(
    receiver: &str,
    member: &str,
    scope: &Scope,
    offset: u32,
) -> Option<&'static ForgeBuiltin> {
    if receiver.is_empty() || member.is_empty() {
        return None;
    }
    if scope.resolve(receiver, offset).is_some() {
        return None;
    }
    match receiver {
        "vm" => find_vm(member),
        "console" => find_console(member),
        _ => None,
    }
}

/// Plain assert builtin for `name`, or `None` when shadowed, ungated, or unknown.
///
/// Local definitions win: any scope hit blocks the builtin. `Test` heritage
/// gates the rest, so plain files never see forge asserts.
pub(crate) fn plain_builtin(
    file: &SolidityFile,
    scope: &Scope,
    offset: u32,
    name: &str,
) -> Option<&'static ForgeBuiltin> {
    if name.is_empty() {
        return None;
    }
    if scope.resolve(name, offset).is_some() {
        return None;
    }
    if !test_context(file, offset) {
        return None;
    }
    find_assert(name)
}

/// Member builtins matching `prefix`, gated the same way as `member_builtin`.
pub(crate) fn member_builtin_list(
    receiver: &str,
    prefix: &str,
    scope: &Scope,
    offset: u32,
) -> Vec<&'static ForgeBuiltin> {
    let table = match receiver {
        "vm" => VM_TABLE,
        "console" => CONSOLE_TABLE,
        _ => return Vec::new(),
    };
    table
        .iter()
        .filter(|builtin| {
            builtin.name.starts_with(prefix)
                && member_builtin(receiver, builtin.name, scope, offset).is_some()
        })
        .collect()
}

/// Plain assert builtins matching `prefix`, gated the same way as `plain_builtin`.
pub(crate) fn plain_builtin_list(
    file: &SolidityFile,
    prefix: &str,
    scope: &Scope,
    offset: u32,
) -> Vec<&'static ForgeBuiltin> {
    ASSERT_TABLE
        .iter()
        .filter(|builtin| {
            builtin.name.starts_with(prefix)
                && plain_builtin(file, scope, offset, builtin.name).is_some()
        })
        .collect()
}

/// Arity checks for plain asserts inside `Test` contracts.
///
/// Same message shape as `calls::arity_diagnostics`. Local definitions win:
/// any scope hit skips the builtin row. Unknown names stay silent.
pub(crate) fn assert_arity_diagnostics(file: &SolidityFile, scope: &Scope) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        if !inherits_test(file, &contract) {
            continue;
        }
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for call in body.all_calls() {
                let Some(callee) = call.callee() else {
                    continue;
                };
                if scope.resolve(&callee, call.syntax().span.start).is_some() {
                    continue;
                }
                let Some(builtin) = find_assert(&callee) else {
                    continue;
                };
                let Some(expected) = builtin.arity else {
                    continue;
                };
                let actual = call.arg_count();
                if expected != actual {
                    out.push(DiagnosticData::error(
                        call.syntax().span,
                        format!(
                            "this function '{callee}' takes {expected} arguments but {actual} supplied"
                        ),
                        RuleId::CALL_ARITY,
                    ));
                }
            }
        }
    }
    out
}

/// True when the cursor sits on a forge builtin.
///
/// The goto guard uses this to return `None` before dep lookup, so builtin
/// names never jump to coincidental same-name declarations in imports.
pub(crate) fn is_builtin_at(file: &SolidityFile, text: &str, offset: u32, scope: &Scope) -> bool {
    if let Some(ctx) = super::members::member_context_at(text, offset)
        && (ctx.member_span.contains(offset) || ctx.member_span.end == offset)
        && member_builtin(&ctx.receiver, &ctx.member, scope, offset).is_some()
    {
        return true;
    }
    if let Some((word, _)) = super::word_at(text, offset)
        && plain_builtin(file, scope, offset, &word).is_some()
    {
        return true;
    }
    false
}
