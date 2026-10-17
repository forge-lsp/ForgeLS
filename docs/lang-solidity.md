# Solidity Support

Scope: `forgels-lang-solidity` crate.

- Language id: `solidity`
- Extensions: `.sol`
- Modules: `ast`, `lexer`, `parser`
- Base traits: `forgels-syntax::Lexer`, `forgels-syntax::Parser`, `forgels-syntax::AstNode`
- Allowed deps: `forgels-core`, `forgels-protocol`, `forgels-syntax`, `forgels-common`
- Forbidden deps: any other `forgels-lang-*`

Status: token kinds defined. Real lexer and parser are planned.

## Beta token subset

- Keywords: `pragma`, `import`, `as`, `from`, `contract`, `function`,
  `constructor`, `modifier`, `event`, `emit`, `returns`, `return`, `if`,
  `else`, `for`, `while`, visibility and mutability (`public`, `private`,
  `internal`, `external`, `pure`, `view`, `payable`, `constant`, `virtual`,
  `override`), data location (`memory`, `storage`, `calldata`), types
  (`bool`, `address`, `string`, `bytes`, `mapping`, `uint`, `int`),
  `true`, `false`, `is`, `abstract`.
- Full grammar words: `struct`, `enum`, `interface`, `library`,
  `break`, `continue`, `do`, `try`, `catch`, `new`, `delete`, `using`,
  `type`, `unchecked`, `immutable`, `transient`, `indexed`, `anonymous`,
  `assembly`, `fallback`, `receive`, `revert`, `error`, `hex`, `unicode`,
  `global`.
- Sized types: `uint8` to `uint256` and `int8` to `int256` in steps of 8,
  `bytes1` to `bytes32`, `fixed` and `ufixed` bare plus `MxN` forms.
- Grouped words: denominations (`wei`, `gwei`, `ether`, time units) and
  28 future-reserved words share one kind each.
- Literals: decimal and hex numbers, leading-zero octals, single and
  double quoted strings, plus `hex` and `unicode` prefixed strings.
- Symbols: braces, brackets, parens, arithmetic, comparison, bitwise,
  assignment combos, `=>`, `?`, `;`, `,`, `:`, `.`.
- Comments: `//` line and `/* */` block. Whitespace is skipped.

## Imports

- Source: `analysis::imports::imports_in` tokenizes and parses with recovery, then reads `SolidityFile::imports()`.
- Shapes:
  - `import "./A.sol";` gives `raw = "./A.sol"`, `symbols = []`.
  - `import {B as C} from "./B.sol";` gives `raw = "./B.sol"`, `symbols = ["B", "C"]`.
  - `import * as NS from "./C.sol";` gives `raw = "./C.sol"`, `symbols = ["NS"]`.
- Broken files still report imports. Entries without a path are skipped.
- Core resolves each `raw` with `resolve_with_remappings` and rebuilds the `ProjectIndex` entry.

## Cross-file

- Completion: local `complete_at` plus `exports_of` from `dep_texts(ctx, 16)`. Deduped by label, local wins. Imported items add ` - from <file>`.
- Hover: local `hover_at` first. Else scan `exports_of` for the word at cursor and show `<detail> - from <file>`.
- Goto: `goto_definition` for same file. `goto_location` falls back to `resolve_in_dep` over `dep_texts(ctx, 16)` and returns `GotoTarget { path, span }`.
- Exports: `Scope::export_symbols` returns contracts plus their functions only.
- Members: `this` plus inherited members resolve across `dep_texts(ctx, 16)` with ` - from <Base> in <file>` origin and correct `GotoTarget` path. Receiver types in deps resolve the same way.

## Inheritance

- Scope: eager merge into `contract_scope` and `members_of`, no `Symbol` schema change. Origin uses detail suffix ` - from <Base>` same file and ` - from <Base> in <file>` cross file.
- Order: heritage order DFS with visited set, transitive bases before direct ones. Derived wins, first base wins. This is not C3.
- Arity: `calls::arity_diagnostics` follows the same chain, derived wins.
- Exports: `exports_of` stays own members only, no flatten.

## Forge-std cheatcodes

- Scope: `analysis::forge` tables plus thin wiring in `completion`,
  `hover`, `mod` (member completion, goto guard, diagnostics).
- Subset:
  - `vm.*` member cheatcodes (10): `prank`, `startPrank`, `stopPrank`,
    `warp`, `roll`, `deal`, `label`, `mockCall`, `expectRevert`,
    `expectEmit`.
  - `Test` plain asserts (6): `assertTrue`, `assertFalse`, `assertEq`,
    `assertNotEq`, `assertGt`, `assertLt` with fixed arity 1 or 2.
  - `console.*` members (3): `log`, `logUint`, `logAddress`.
- Rules:
  - User definitions always win: any scope hit on the receiver
    (`vm`, `console`) or the assert name hides the builtin.
  - Unknown names stay silent: no completion, no hover, no diagnostic.
  - Plain asserts need `Test` heritage: direct or transitive same file
    bases. Other files never see them.
  - Completion and hover tag builtins with ` - from forge-std` and reuse
    kind Function (3). No new completion kind.
  - Goto returns `None` for builtins by design and skips dep lookup, so
    builtin names never jump to coincidental same-name declarations.
- Limits:
  - Member arity checked: `MemberExpr` stores call markers, so
    `vault.withdraw(a, b, c)` diagnoses against own, inherited, and
    dep definitions. Builtins (`vm.*`, `console.*`) stay silent by
    priority guard. Plain asserts check arity with the same message
    shape as plain calls.
  - Message-string assert overloads (for example 3-arg `assertEq`) are
    follow-up. V1 checks the canonical fixed arity only.

## Array members

- Scope: `analysis::array` plus thin wiring in `members`, `hover`,
  `goto`, `calls`, `mod`. No new `SymbolKind`, no new completion kind.
- Shape: `TypeKind::Array { dynamic }` via `array_shape` outer suffix
  only. `uint[]` and `uint[2][]` are dynamic, `uint[2]` and `uint[][2]`
  are fixed. `string` without brackets is not an array.
- Members: dynamic has `length` (Field, `uint256 length`), `push`
  (Function, 1 arg), `pop` (Function, 0 args). Fixed has `length`
  only. Details add ` - from solidity`, kinds reuse Field (5) and
  Function (3).
- Completion: `members_of` returns array members, cross file stays empty.
- Hover: array member shows detail with origin and uses the cursor member
  span. Synthetic `0,0` spans never leak.
- Goto: returns `None` by design with a dep guard, so array names never
  jump to coincidental same-name declarations.
- Arity: `push` takes 1, `pop` takes 0, same message as plain calls.
  `length` calls and fixed `push`/`pop` stay silent. Unknown names stay
  silent.
- Limits: data location (`memory`, `storage`, `calldata`) ignored,
  element type ignored, no signature help, chained `a.b.c` untouched.

## Address members

- Scope: `analysis::address` plus thin wiring in `members`, `hover`,
  `goto`, `calls`, `mod`. No new `SymbolKind`, no new completion kind.
- Shape: `TypeKind::Address` via `is_address_type` for `address` and
  `address payable`. V1 does not distinguish payable: the parser strips
  `payable` from the stored type, so both spellings share one set.
  Array check runs first, so `address[]` stays an array.
- Members: `balance` (Field, `uint256`), `code` (Field, `bytes`),
  `codehash` (Field, `bytes32`), `transfer` (Function, 1 arg), `send`
  (Function, 1 arg), `call` (Function, 1 arg), `delegatecall`
  (Function, 1 arg), `staticcall` (Function, 1 arg). Details add
  ` - from solidity`, kinds reuse Field (5) and Function (3).
- Completion: `members_of` returns address members, cross file stays empty.
- Hover: address member shows detail with origin and uses the cursor
  member span. Synthetic `0,0` spans never leak.
- Goto: returns `None` by design with a dep guard, so address names never
  jump to coincidental same-name declarations.
- Arity: `transfer`, `send`, `call`, `delegatecall`, and `staticcall`
  take 1, same message as plain calls. `balance`, `code`, and `codehash`
  calls stay silent. Unknown names stay silent.
- Limits: payable not distinguished, no signature help, chained `a.b.c`
  untouched.

## Bytes and string members

- Scope: `analysis::bytes` plus thin wiring in `members`, `hover`,
  `goto`, `calls`, `mod`. No new `SymbolKind`, no new completion kind.
- Shape: `TypeKind::Bytes` via `is_bytes_type` for `bytes` and `bytes1`
  to `bytes32`. Array check runs first, so `bytes[]` stays an array.
  `TypeKind::String` via `is_string_type` for `string` only. `string`
  without brackets is not an array.
- Members: bytes has `length` (Field, `uint256 length`) only. Fixed
  `bytes32` shares the same set in V1: no push or pop. String has no
  members by design: length needs `bytes(s).length`. Details add
  ` - from solidity`, kind reuses Field (5).
- Completion: `members_of` returns bytes members, string returns empty,
  cross file stays empty.
- Hover: bytes member shows detail with origin and uses the cursor member
  span. String has no hover. Synthetic `0,0` spans never leak.
- Goto: returns `None` by design with dep guards, so `length` never
  jumps to coincidental same-name declarations.
- Arity: all silent. `length` is a field so calls stay silent.
  Unknown names stay silent.
- Limits: fixed versus dynamic not split, no signature help, chained
  `a.b.c` untouched.

## Function type members

- Scope: `analysis::function` plus thin wiring in `members`, `hover`,
  `goto`, `calls`, `mod`. No new `SymbolKind`, no new completion kind.
- Shape: `TypeKind::Function` via `is_external_function_type` (prefix
  `function` plus `external`) for function-typed vars. Internal function
  types stay unknown. Named functions resolve as `Function` too, so
  `f.selector` works without a declared function type.
- Members: `selector` (Field, `bytes4`), `gas` (Function, 1 arg),
  `value` (Function, 1 arg). Details add ` - from solidity`, kinds
  reuse Field (5) and Function (3).
- Completion: `members_of` returns function members, cross file stays empty.
- Hover: function member shows detail with origin and uses the cursor
  member span. Synthetic `0,0` spans never leak.
- Goto: returns `None` by design with a dep guard, so `selector` never
  jumps to coincidental same-name declarations.
- Arity: `gas` and `value` take 1, same message as plain calls.
  `selector` calls stay silent. Unknown names stay silent.
- Limits: no `address` member in V1, no visibility split for named
  functions, no signature help, chained `a.b.c` untouched.

## Type metadata

- Scope: `analysis::meta` plus thin wiring in `members`, `hover`,
  `goto`, `completion`, `mod`. No new `SymbolKind`, no new completion kind.
- Shape: `type_context_at` scans `type(Inner).member` separately because
  the normal receiver scan fails on `)`: no word precedes the dot. Inner
  is the trimmed text between parens, nested parens return `None`. Empty
  members (`type(X).`) return a context so completion still lists.
- Dispatch: integer inners (`uint`, `int`, plus `8` to `256` in steps
  of 8) yield `min` and `max` typed as the inner. All other inners yield
  `name` (`string`), `creationCode` (`bytes`), `runtimeCode` (`bytes`),
  and `interfaceId` (`bytes4`). V1 does not split contracts from
  interfaces; enum inners fall into the contract set as follow-up.
- Members: `TypeKind::Meta(inner)` via `members_of`, cross file stays
  empty. Details add ` - from solidity`, kind reuses Field (5).
- Completion: `type` branch in `complete_at` and `mod` lists the inner
  set filtered by prefix.
- Hover: meta member shows detail with origin and uses the cursor member
  span. Synthetic `0,0` spans never leak.
- Goto: returns `None` by design with a dep guard. `type()` calls are
  not parsed as `Member` nodes, so member arity stays silent by absence.
- Limits: no nested inner types, no enum split, no signature help.

## Global members

- Scope: `analysis::globals` plus thin wiring in `members`, `hover`,
  `goto`, `calls`, `mod`. No new `SymbolKind`, no new completion kind.
- Shape: `TypeKind::Global(name)` for undeclared `msg`, `block`, `tx`,
  and `abi`. User declarations shadow globals via the scope check first.
- Members:
  - `msg` (4 fields): `sender` (`address`), `value` (`uint256`),
    `data` (`bytes`), `sig` (`bytes4`).
  - `block` (9 fields plus 1 method): `basefee`, `blobbasefee`,
    `chainid`, `coinbase` (`address`), `difficulty`, `prevrandao`,
    `gaslimit`, `number`, `timestamp` (all `uint256` except `coinbase`),
    plus `blobhash` (Function, 1 arg). `blockhash` stays a plain global
    function, not a block member.
  - `tx` (2 fields): `gasprice` (`uint256`), `origin` (`address`).
  - `abi` (6 methods): `decode` (2 args), `encode` (variadic),
    `encodePacked` (variadic), `encodeWithSelector` (variadic),
    `encodeWithSignature` (variadic), `encodeCall` (2 args).
  - Details add ` - from solidity`, kinds reuse Field (5) and
    Function (3).
- Completion: `members_of` returns the receiver table, cross file stays empty.
- Hover: global member shows detail with origin and uses the cursor
  member span. Synthetic `0,0` spans never leak.
- Goto: returns `None` by design with a dep guard, so `sender` never
  jumps to coincidental same-name declarations.
- Arity: `block.blobhash` takes 1, `abi.decode` and `abi.encodeCall`
  take 2, same message as plain calls. Fields stay silent and variadic
  `abi.encode*` stays silent by design. Unknown names stay silent.
- Limits: no signature help, chained `a.b.c` untouched.

## Unknown identifiers

- Source: `analysis::unknown::unknown_identifier_diagnostics` scans `Ident` tokens only, so comments and strings never lint.
- Skips: declaration name spans, member names after a dot, emit names (owned by `unknown_emit`), import and pragma spans, assembly bodies.
- Resolve order: scope, dep exports, forge asserts, allowlists (`GLOBAL_RECEIVERS`, `ALWAYS_OK`), then one way prefix escape for typing (`tot` before `total` stays silent, `totalX` still errors).
- Error: `unresolved identifier '{name}'` with the token span. Unknown plain callees error, unknown member names stay silent, emit names never double.
- Limits: reserved words used as names stay silent (`Ident` only), return param names are not tracked, free functions are out of scope.

## Document symbols

- Scope: `analysis::symbols` plus `Analyzer::document_symbols` and the
  `textDocument/documentSymbol` dispatch. Capability
  `documentSymbolProvider: true`.
- Shape: `DocumentSymbolData {name, detail, kind, range, selection,
  children}`. Dispatch maps `range` to `range` and `selection` to
  `selectionRange`, kinds as LSP 3.17 numbers.
- Hierarchy: contracts nest functions, state vars, structs, enums,
  errors, events, modifiers, and user types in source order.
  Functions nest params, structs nest fields, events nest params,
  enums nest variants. File level lists structs, enums, errors, user
  types, events, then contracts.
- Kinds: contract is Class (5), function and modifier are Method (6),
  struct field and state var are Field (8), enum is Enum (10),
  error is Variable (13), enum variant is EnumMember (22), struct is
  Struct (23), event is Event (24), user type is TypeParameter (26).
  Constructor stays Method (6) in V1.
- Spans: `range` is the full node span, `selection` is the name span
  with fallback to the full span. Enum variants use the variant span
  for both. Nameless nodes are skipped, so broken rows still leave a
  partial outline. Unknown files answer `null`.
- Noir: trait default returns `[]`, no noir grammar change.

## References

- Scope: `analysis::references` plus `Analyzer::references` and the
  `textDocument/references` dispatch. Capability
  `referencesProvider: true`.
- Shape: `ReferenceData {path, span, is_declaration}`. Dispatch maps
  each entry to `Location`, honors `context.includeDeclaration`, skips
  out-of-bounds spans, caps at `MAX_REFERENCES = 100`.
- Resolve: member, emit, then plain. Builtins and unknown words return
  `None`. Identity is span based: each usage must resolve to the same
  `(path, span)` as the target, so shadowed names never mix.
- Scan: `Ident` tokens per file only. Imports, pragmas, and assembly
  never count. Plain skips member and emit slots, member needs a dot,
  emit needs an emit slot. Local vars use a fast path that scans the
  current file only. Others scan tracked Solidity files path-sorted.
- Noir: trait default returns `None`, no noir grammar change.

## V1 limits

- Tracked only: `dep_texts` skips untracked and missing files.
- Caps: dep read cap is 16, republish fan-out cap is 32, references cap is 100.
- Symbols: `Contract` and `Function` cross files. `StateVar`, `Param`, and `LocalVar` stay local.
- Imports: union without alias precision. `{B as C}` records both `B` and `C`, no alias mapping.
- Parse: parse on each call, no incremental cache.
- Inheritance: no C3, no `super`, no chained `a.b.c`, unknown bases ignored, cycles end via visited set, `exports_of` stays own members.
