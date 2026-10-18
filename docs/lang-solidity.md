# Solidity Support

Scope: `forgels-lang-solidity` crate.

- Language id: `solidity`
- Extensions: `.sol`
- Modules: `ast`, `lexer`, `parser`
- Base traits: `forgels-syntax::Lexer`, `forgels-syntax::Parser`, `forgels-syntax::AstNode`
- Allowed deps: `forgels-core`, `forgels-protocol`, `forgels-syntax`, `forgels-common`
- Forbidden deps: any other `forgels-lang-*`

Status: beta lexer, parser, and analysis implemented.

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
- Exports: `Scope::export_symbols` returns contracts, functions, structs, enums, errors, events, modifiers, and user types. State vars, params, and locals stay local.
- Members: `this`, `super`, enum names, and user type names resolve
  across `dep_texts(ctx, 16)` with ` - from <Base> in <file>` origin and
  correct `GotoTarget` path. Receiver types in deps resolve the same way.

## Inheritance

- Scope: eager merge into `contract_scope` and `members_of`, no `Symbol` schema change. Origin uses detail suffix ` - from <Base>` same file and ` - from <Base> in <file>` cross file.
- Order: C3 with heritage order, direct bases before transitive ones. Derived wins, first base wins. Signature stays `linearize` plus `linearize_across`.
- Fallback: inconsistent hierarchies fall back to heritage DFS and emit `inconsistent linearization for 'D'` at the contract name. Same file via `is_inconsistent`, deps via `is_inconsistent_across`.
- Symbols: inherited scope covers structs, enums, errors, user types, events, modifiers, state vars, and functions. Member lookup (`members_of`) stays vars plus funcs.
- Arity: `calls::arity_diagnostics` follows the same chain, derived wins. `errors::revert_arity_diagnostics` follows the chain across deps with dep file and contract errors as fallback.
- Exports: `exports_of` stays own members only, no flatten.

## Super members

- Scope: `TypeKind::Super { enclosing }` in `members`, plus thin wiring in `hover`, `goto`, `calls`, `signature`, `inlay`, `payable`, `mod`.
- Shape: `super` resolves to the enclosing contract bases only, derived bypassed. Outside a contract it stays unknown.
- Members: base state vars plus base functions with ` - from <Base>` origin. Same file via `inherited_members`, deps via `linearize_across` with ` - from <Base> in <file>` origin.
- Hover and goto: base declarations, same rules as inherited `this` members.
- Arity, signature, inlay, payable: base signatures across files, derived excluded. Getters excluded.
- Limits: unknown `super` names stay silent everywhere.

## Chained members

- Scope: `MemberExpr::chain` plus `parser::try_parse_member` tails and `analysis::members::{chain_before, kind_of_chain, resolve_chained_kind, parent_kind_for_member}` with wiring in `hover`, `goto`, `complete`, `calls`, `signature`, `inlay`, `payable`, `references`, and `call_hierarchy`.
- Shape: `a.b.c` stores receiver plus first member plus up to `MAX_CHAIN_TAILS = 4` extra `.ident` tails. Text scan (`chain_before`) walks back through dots and skips intermediate `(...)` calls.
- Parse: tails cap at 4; beyond the cap the rest stays for `recover_statement`. Intermediate calls (`a.b().c`) are consumed but never stored; only the final segment keeps call markers. Unclosed tails fall back to plain.
- Resolve: final-segment-only. Root via `receiver_type_with_deps`, each intermediate via its declared type (state var, struct field, or first function return) with cross-file lookup per level. Intermediates stay silent, builtins terminate, unknown members stay silent.
- Limits: using members stay silent for chains, `wrap` and `unwrap` stay single level, longer builtin chains stay silent.

## Enum and user type members

- Scope: `TypeKind::Enum(name)` and `TypeKind::UserType(name)` in `members`, plus thin wiring in `hover`, `goto`, `calls`, `signature`, `mod`. New `SymbolKind::EnumMember` maps to LSP 20.
- Receivers: enum and user type names resolve via scope, same file first then dep files. Contract level and file level both work.
- Enum members: variants with `<Enum>.<Variant>` detail and real variant spans. Hover and goto jump to the variant token. Cross file adds ` - from <file>`.
- User type members: `wrap` and `unwrap`, each 1 arg. Details read `function wrap(<Underlying> value) returns (<Name>)` and `function unwrap(<Name> value) returns (<Underlying>)`.
- Synthetic spans: `wrap` and `unwrap` use `0,0` spans that never leak. Hover shows the cursor member span, goto returns `None` by design with a dep guard.
- Limits: enum variant calls and unknown member names stay silent. Value receivers of enum or user type stay unknown.

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
  element type ignored, no signature help, chained `a.b.c` resolves final only.

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
- Context: `delegatecall` runs target code in caller storage with
  caller `msg.sender` and `msg.value`. V1 checks arity only (1 arg);
  sender, storage, and reentrancy checks are deferred, documented
  here only.
- Limits: payable not distinguished, no signature help, chained `a.b.c`
  resolves final only.

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
  `a.b.c` resolves final only.

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
  functions, no signature help, chained `a.b.c` resolves final only.

## Function types

- Scope: `parser::parse_function_type` plus `ast::TypeNode::text` and
  `analysis::unknown::function_type_spans`. No new `SymbolKind`, no new
  completion kind.
- Shape: `function (params) internal|external [mutability] [returns (params)]`
  with array suffixes (`[]`, `[N]`) on the outer type.
- Visibility is required: only `internal` or `external` pass, anything
  else errors with `expected 'internal' or 'external'`.
- Mutability is optional: `pure`, `view`, `payable`, `constant`.
- Returns is optional with its own param list.
- Mapping keys reject function types with
  `function type not allowed as mapping key`; mapping values accept them.
- Text joins as `function (uint) external` with one space before `(`;
  `mapping` stays glued as `mapping(address => uint)`.
- Inner param names stay silent for unknown, references, and rename via
  `function_type_spans`; outer names resolve normally.
- Hover and member lookup reuse the type text, so external function vars
  expose `selector`, `gas`, and `value` with no extra work.
- Limits: no data location checks for function types, no signature help
  for inner params.

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
    `encodePacked` (variadic), `encodeWithSelector` (min 1),
    `encodeWithSignature` (min 1), `encodeCall` (2 args).
  - Details add ` - from solidity`, kinds reuse Field (5) and
    Function (3).
- Completion: `members_of` returns the receiver table, cross file stays empty.
- Hover: global member shows detail with origin and uses the cursor
  member span. Synthetic `0,0` spans never leak.
- Goto: returns `None` by design with a dep guard, so `sender` never
  jumps to coincidental same-name declarations.
- Arity: `block.blobhash` takes 1, `abi.decode` and `abi.encodeCall`
  take 2, same message as plain calls. `abi.encodeWithSelector` and
  `abi.encodeWithSignature` need at least 1 arg each with a
  `takes at least N` message. Fields stay silent, bare `abi.encode`
  and `abi.encodePacked` stay silent. Unknown names stay silent.
- Limits: no signature help, chained `a.b.c` resolves final only.

## Builtin arity

- Scope: `analysis::builtin` plus wiring in `mod::diagnostics_for`.
  No new `SymbolKind`, no grammar change.
- Table: fixed counts for plain globals only: `gasleft` (0),
  `keccak256` (1), `sha256` (1), `ripemd160` (1), `ecrecover` (4),
  `addmod` (3), `mulmod` (3), `blockhash` (1), `selfdestruct` (1),
  `assert` (1). Same message shape as plain calls.
- Rules: user definitions win via the scope check first. Unknown
  names stay silent. `require` and `revert` accept several shapes so
  they stay silent. Forge asserts, `vm`, `console`, `abi`, and
  `type()` members are owned by their own checks.
- Limits: contract bodies only, no free function bodies, no
  signature help.

## Unknown identifiers

- Source: `analysis::unknown::unknown_identifier_diagnostics` scans `Ident` tokens only, so comments and strings never lint.
- Skips: declaration name spans, member names after a dot, emit names (owned by `unknown_emit`), revert names (owned by `unknown_error`), deploy names (owned by deploy arity), import and pragma spans, assembly bodies.
- Resolve order: scope, dep exports, forge asserts, allowlists (`GLOBAL_RECEIVERS`, `ALWAYS_OK`), then one way prefix escape for typing (`tot` before `total` stays silent, `totalX` still errors).
- Error: `unresolved identifier '{name}'` with the token span. Unknown plain callees error, unknown member names stay silent, emit and revert names never double.
- Placeholder: bare `_` stays silent (modifier placeholder, ignored bindings). References and rename return `None` for it by design.
- Limits: reserved words used as names stay silent (`Ident` only), return param names are not tracked, free functions are out of scope.

## Block scopes

- Scope: `ast::BlockNode` plus `parser::parse_block` and
  `analysis::{mod::block_scope, unknown, calls, emits, errors}`.
  New `SolidityNode` kinds `Revert` and `New` hold revert and deploy
  rows, no new `SymbolKind`: nested blocks reuse `Block`, decls reuse
  `LocalVar`.
- Shape: `BlockNode::child_blocks` returns direct nested blocks.
  `all_locals`, `all_calls`, `all_members`, `all_emits`,
  `all_reverts`, and `all_news` recurse depth first. `locals`,
  `calls`, `members`, `emits`, `reverts`, and `news` stay direct
  children only.
- Parse: bare `{ ... }` groups recurse into `parse_block` as child
  blocks. `if`, `for`, `while`, `unchecked`, and `try` bodies recurse
  the same way. `assembly` bodies stay skipped with no nodes.
- Bodies: brace bodies become child blocks. Non-brace bodies fall back
  to skipping one statement with no node; a nested control in that
  slot still recurses. `else` binds to its `if` in the same call.
- For header: `parse_for` wraps the whole `for` statement in one child
  `Block` from `for` to body end. A `Type name [= init];` init becomes
  one `LocalVar` inside that wrapper so bodies see it, but uses after
  the loop miss. Empty inits, assignments, and broken headers stop
  silently past `)`.
- Tuple: `(T a, U b) = rhs;` expands to one `LocalVar` per named
  element. Bare `(a, b) = rhs;` is a pure assignment with no symbols.
  Rows must close with `)` then `=` and end with `;`; anything else
  rewinds so the caller recovers one statement.
- Scopes: `function_scope` holds params, then one `block_scope` child
  per body. `block_scope` holds direct locals plus direct `try` params
  plus one child scope per nested block, so the deepest decl wins for
  hover, completion, goto, references, and rename.
- Diagnostics: `unknown` decl, emit, revert, new, member, and option
  spans plus plain, member, emit, revert, and deploy arity all use
  `all_*`, so nested rows lint the same as top-level rows.
- Outline: `function_children` stays params only by design. Locals
  never outline; the comment on the function states why.
- Try: `parse_try` wraps the whole `try` statement in one child `Block`
  from `try` to body end. Try `returns` params and `catch` params become
  `Param` nodes inside the wrapper so bodies resolve them. Bodies recurse
  via `parse_control_body`, so nested `try` works. `try new C(args)`
  parses the deploy as a `New` node inside the wrapper so deploy arity
  reuses the same constructor check. `Error` and `Panic` catch names stay
  silent via the allowlist.
- Limits: `do` bodies fall back to statement skips, single-statement
  bodies hold no decls. Type mismatch, inlay, payable, visibility, and
  call hierarchy still read direct children only.

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

## Rename

- Scope: `analysis::rename` plus `Analyzer::rename` and the
  `textDocument/rename` dispatch. Capability `renameProvider: true`,
  no `prepareRename` in V1.
- Shape: reuses `ReferenceData {path, span, is_declaration}`. Dispatch
  groups each entry by URI into `WorkspaceEdit {changes: {uri: [{range,
  newText}]}}`, skips out-of-bounds spans, caps at
  `MAX_REFERENCES = 100`.
- Validate: `is_valid_rename` needs ident shape `[A-Za-z_$][A-Za-z0-9_$]*`
  and rejects any `keyword_kind` hit (`contract`, `uint8`, `wei`,
  reserved words). Empty and bad shapes return `None`.
- Resolve: `find_rename` checks the new name first, then reuses
  `find_references` (member, emit, then plain with span identity).
  Builtins and unknown words return `None`. Empty lists return `None`
  so dispatch answers `InvalidParams` (-32602).
- Noir: trait default returns `None`, no noir grammar change.

## Function visibility

- Scope: `analysis::visibility` plus `FunctionDef::visibility` and parser
  storage of `public`, `private`, `internal`, `external` tokens.
- Rules:
  - Plain `f()` where `f` is external errors with
    `external function 'f' must be called via 'this.f()' or 'C.f()'`
    where `C` is the calling contract.
  - Plain `f()` where `f` is private in a base errors in the derived
    contract with
    `private function 'f' cannot be called from derived contract 'D'`.
    Own private calls stay silent, internal plain calls stay silent.
  - Instance `c.f()` where `f` is internal or private in the receiver
    contract errors with
    `internal function 'f' cannot be called via instance 'c'` or
    `private function 'f' cannot be called via instance 'c'`.
- Lookup: own functions first, then `linearize` plus `linearize_across`
  bases. Derived wins, first base wins. Mixed overloads stay silent.
- Limits: `this`, `super`, and library targets stay silent. Internal
  calls through inheritance stay silent. Chains longer than two stay
  silent. Contract-name calls stay silent.

## State mutability

- Scope: `analysis::mutability` plus `FunctionDef::mutability` with
  `constant` normalized to `view`. Parser stores `pure`, `view`,
  `payable`, and `constant` tokens.
- Rules:
  - Token scan of `Ident` in the body resolves `StateVar` only.
    Decl names, imports, and assembly stay silent. `this.x` counts as a
    read. `delete`, assignments, and `++` or `--` count as writes. Pure
    errors on reads and writes, view errors on writes only.
  - Member writes via `all_members`: `arr.push` and `arr.pop` count as
    writes, plus direct `s.field` on the left of an assign family with
    a `StateVar` root. Pure and view report the same write messages
    with the root name. Member roots own the diagnostic once, so pure
    shows one write, not read plus write.
- Messages: `pure function 'f' cannot read state variable 'x'`,
  `pure function 'f' cannot write to state variable 'x'`,
  `view function 'f' cannot write to state variable 'x'`.
- Limits: two-segment chains only. Mappings, assembly, cross-function
  calls, call targets, and longer chains stay silent. Struct reads stay
  silent, only LHS writes lint. Params and locals shadow state vars via
  scope resolve.

## Payable call options

- Scope: `analysis::payable` plus `has_call_options` and `options_span`
  on `CallStmt` and `MemberExpr`. Parser scans balanced `{...}` then
  parens and falls back to plain on failure. Unknown lint skips option
  spans so `value` and `gas` never error.
- Rule: calls and member calls with a `value` option error when the
  target is not `payable`, with
  `non-payable function 'f' cannot receive 'value'`. Builtins, unknown
  targets, and gas-only options stay silent.
- Lookup: plain calls use own plus linearized bases, member calls use
  receiver contract plus same file and dep bases. Derived wins.
- Options: unknown keys and `salt` misuse are owned by call options
  below. This check only reads `value` against payable targets.
- Limits: no `msg.value` checks, no payable address split, no fallback
  or receive rules.

## Call options

- Scope: `analysis::options` plus wiring in `mod::diagnostics_for`.
  Reads `{...}` spans from `CallStmt`, `MemberExpr`, and `NewStmt`.
  No new `SymbolKind`, no grammar change.
- Rule: unknown keys error with `unknown call option 'x'` at the key
  span. Allowed keys are `value` and `gas` for calls, plus `salt` for
  `new` (create2). `salt` on plain or member calls errors with
  `call option 'salt' only allowed for 'new'`.
- Keys: first ident before a colon per top-level comma segment, so
  nested calls in values never split keys. Segments without that
  shape stay silent.
- Limits: `payable` owns the `value` check separately, never doubled
  here. No duplicate key checks, no value type checks.

## Type mismatch

- Scope: `analysis::type_mismatch` plus `StateVar::initializer` and
  `LocalVar::initializer`. Parser stores one literal token only when
  the initializer is exactly one literal.
- Rules:
  - Single literals against elementary types only. Message shape
    is `type mismatch: cannot assign <literal> to '<type>'` with the
    literal span. Mappings, arrays, and user types stay silent.
  - Tiny builtin inference for non-literals: `msg.sender` as `address`,
    `block.coinbase` as `address`, `block.basefee`, `block.blobbasefee`,
    `block.chainid`, `block.difficulty`, `block.gaslimit`, `block.number`,
    `block.timestamp`, `block.prevrandao` as `uint256`,
    any `abi.encode*` call as `bytes`,
    plus `*.balance` and `*.length` as `uint256`. Mismatch reports
    `type mismatch: cannot assign <inferred> to '<type>'` with the init
    span. Matches stay silent.
- Silent: `1 ether` (two tokens), hex numbers for `address`, idents,
  casts (`Type(...)` always converts), chains, binops, and other calls
  (no stored literal, no builtin shape).
- Limits: five builtin shapes only. Literal chains, binops, plain calls,
  and call returns stay silent. No range checks, no implicit conversion
  tables. Direct locals only, nested blocks stay silent.

## Using-for

- Scope: `ast::UsingDirective` plus `parser::parse_using` and
  `analysis::using`. `ContractDef::usings` and `SolidityFile::usings`
  list rows. Parser stores library ident, target type or `*`, plus
  optional `global` and `;`.
- Rule: `x.f()` where `x: T` attaches library `L` when `using L for T`
  matches exactly and `L.f` takes `T` first. Wildcard `*` matches any
  receiver by first param. Own members win by name. Detail adds
  ` - from L via using`.
- Completion, hover, goto, signature, and member arity reuse the same
  attach. Library lookup spans primary then deps. Unknown skips whole
  using rows so missing libs never lint.
- Limits: exact spelling only (`uint` never matches `uint256`),
  operator forms (`using {f as +}`) and multi-library lists keep span
  only, no `super` receivers, chained `a.b.c` stays silent for using.

## Override checks

- Scope: `analysis::override_diagnostics` plus `FunctionDef::is_virtual`,
  `is_override`, `override_bases` and parser storage of `virtual` and
  `override(A, B)` markers. `StateVar` stores `public`, `virtual`, and
  `override` for future use but never reports here.
- Rules:
  - O1: derived matches a virtual base but misses `override` errors
    with `function 'f' overrides virtual base but missing 'override'`.
  - O2: derived has `override` with no matching base errors with
    `function 'f' has 'override' but no base function`.
  - O3: derived has `override` but bases are non-virtual errors with
    `function 'f' overrides non-virtual base`.
- Silent: overloads (different param types), private derived, private
  bases, `constructor`, `fallback`, `receive`, and state vars. Any
  same-name base silences O2 so overloads never report as missing.
- Limits: same file C3 `linearize` only, no `override(A, B)` list
  validation, no state-var override rules.

## Public getters

- Scope: `analysis::getter` plus `StateVar::is_public` and parser
  storage of `public`. `contract_getters` synthesizes from public vars
  only. Explicit functions with the same name win and suppress synthesis.
- Shapes:
  - Scalar `uint public x` gives `function x() returns (uint)`.
  - Array `uint[] public a` gives `function a(uint256) returns (uint)`.
  - Single mapping `mapping(K => V) public m` gives
    `function m(K) returns (V)`.
- Wiring: scope symbols plus contract members plus plain and member
  signature help. Plain and member arity check getter params (0 for
  scalar, 1 for array and mapping). No cross file export: `export_symbols`
  skips synthetic spans and dep member collection stays explicit only.
- Limits: nested mappings stay silent, no struct field getters, no
  visibility split, no cross file getter calls.

## Hover values and NatSpec

- Scope: `analysis::natspec` plus value suffix in `mod`, `members`,
  and `inheritance`. Plaintext only, no markdown.
- Values: state vars and locals with one literal initializer show
  `T N = V` (for example `uint256 total = 42`). Complex forms
  (`1 ether`, binops, calls, idents) show `T N` only. Params never
  show values.
- NatSpec: `///` line runs and `/** */` blocks directly above
  Contract, Function, StateVar, Struct, Enum, Event, and Error attach
  after a blank line. Markers are stripped, tags stay verbatim.
- Gap: only whitespace with at most one blank line between docs and
  decl, same between run lines. Plain `//` and `/* */` never attach.
  Wide gaps drop docs.
- Deps: dep docs insert before the ` - from ` origin suffix, so hover
  reads detail plus docs plus origin.
- Limits: immutable assignments inside constructors are follow-up, no
  value shown. Modifier, UserType, Field, Param, and Local docs stay
  out of scope.

## Inlay hints

- Scope: `analysis::inlay` plus `Analyzer::inlay_hints` and the
  `textDocument/inlayHint` dispatch. Capability
  `inlayHintProvider: true`.
- Shape: `InlayHintData {offset, label, kind}`. Dispatch maps `offset`
  to `position`, `label` carries the trailing colon (`a:`), kind is
  Parameter (2). Caps at `MAX_INLAY_HINTS = 100`.
- Resolve: plain calls through scope plus deps, member calls through
  receiver contract lineage. Builtins and unknown callees stay silent.
  Slots are lexical: first non-ws byte after `(` or `,` at depth zero.
  Unnamed params and already named args stay silent. Range filters by
  offset.
- Noir: trait default returns `[]`, no noir grammar change.

## Semantic tokens

- Scope: `analysis::semantic` plus `Analyzer::semantic_tokens` and the
  `textDocument/semanticTokens/full` dispatch. Capability
  `semanticTokensProvider` with frozen legend and `full: true`.
- Shape: `SemanticTokenData {span, token_type, modifiers}`. Types index
  `SEMANTIC_TOKEN_TYPES`, modifiers bitset over
  `SEMANTIC_TOKEN_MODIFIERS`. Dispatch delta-encodes sorted spans.
  Caps at `MAX_SEMANTIC_TOKENS = 5000`.
- Source: declarations from `document_symbols_in` with declaration bit,
  keywords from a lexer scan via `keyword_kind`. Contract is class,
  function is method, field is property, event is event, param is
  parameter, keyword is keyword. No delta or range in V1.
- Noir: trait default returns `[]`, no noir grammar change.

## Call hierarchy

- Scope: `analysis::call_hierarchy` plus `Analyzer::prepare_call_hierarchy`,
  `incoming_calls`, `outgoing_calls` and three dispatches
  (`prepareCallHierarchy`, `incomingCalls`, `outgoingCalls`).
  Capability `callHierarchyProvider: true`.
- Shape: `CallHierarchyItemData {name, kind, detail, path, span,
  selection}` plus `CallHierarchyCallData {item, ranges}`. Dispatch
  maps spans with tracked or disk text, skips out-of-bounds, caps at
  `MAX_CALL_HIERARCHY = 50`.
- Resolve: prepare only for functions, member then plain with builtin
  guards. Incoming reuses `find_references` identity and groups usages
  by enclosing function. Outgoing scans the target body for plain and
  member calls and resolves through scope and lineage. Cross file
  prepare answers, outgoing lists empty for dep targets in V1.
- Noir: trait defaults return `None`, no noir grammar change.

## Error arity

- Scope: `ast::RevertStmt` plus `parser::try_parse_revert` and
  `analysis::errors`. `BlockNode::reverts` lists direct rows,
  `all_reverts` recurses. No new `SymbolKind`.
- Parse: `revert Name(args)` stores the name plus `(` and `,` markers.
  Bare `revert`, `revert Name`, and unclosed rows keep partial nodes.
  String reasons (`revert("msg")`) keep keyword only so they stay
  silent. Brace rows return `None` so named args never lint.
- Defs: `parse_error` stores `Param` nodes, `ErrorDef::params` counts
  them. Same shape as events.
- Rules:
  - Arity: `this error 'E' takes N arguments but M supplied` with the
    full revert span. Own contract first, then same file bases, then
    file level. Derived wins, first base wins. Unknown names stay
    silent.
  - Unknown: `unresolved error 'E'` with the name span. Bare reverts
    stay silent. Deps resolve in index order, untracked still errors.
- Hover and goto reuse the emit pattern: `resolve_local_error` and
  `find_error_in_dep` with `error` detail. Unknown skips revert name
  spans so names never double.
- Limits: same file arity only, no completion, no signature help.

## Struct constructors

- Scope: `analysis::calls::arity_diagnostics` struct branch. Same file
  only, no new node: struct calls already parse as `CallStmt`.
- Rule: `this struct 'S' takes N arguments but M supplied` with the
  full call span. Fields count via `StructDef::fields`.
- Lookup: own contract structs first, then same file bases in linear
  order, then file level structs. First hit wins. Unrelated contracts
  stay silent via scope miss, unknown structs report as
  `unresolved identifier` once.
- Limits: no dep structs, no named args, initializers stay skipped so
  only top-level calls lint.

## Constructor args

- Scope: `ast::NewStmt` plus `parser::try_parse_new` and
  `analysis::calls::deploy_arity_diagnostics`. `BlockNode::news`
  lists direct rows, `all_news` recurses. `ContractDef::constructors`
  lists `constructor` functions.
- Parse: `new C(args)` stores the name plus `(` and `,` markers.
  Option rows (`new C{value: 1}(...)`) keep `{` and `}` markers so
  unknown skips `value`. Named args (`new C({a: 1})`), `super`, and
  `constructor` return `None` so they stay silent.
- Rule: `this constructor for 'C' takes N arguments but M supplied`
  with the full deploy span. Target resolves across files with
  `find_contract_across`. Missing targets stay silent. No explicit
  constructor means zero args.
- Unknown: deploy name spans are skipped so unknown never doubles.
  Unknown targets stay silent by design, like unknown members.
- Limits: top-level deploys only, initializers stay skipped, no
  heritage args, no payable checks for deploys.

## Shadowing

- Scope: `analysis::shadow` plus `Analyzer::diagnostics` and
  `diagnostics_ctx`. No new `SymbolKind`, no grammar change.
- Rule: inner `Param` or `LocalVar` that matches an outer `StateVar`,
  `Param`, or ancestor `LocalVar` errors with
  `parameter 'x' shadows outer declaration` or
  `variable 'x' shadows outer declaration` at the inner name span.
  Sibling blocks never clash, same block duplicates report.
- Silent: heritage, functions, contracts, structs, enums, errors,
  events, modifiers, user types, and builtins (`msg`, `block`, `vm`,
  `console`, forge tables, allowlists). State to state duplicates
  stay silent, only nested shadows report.
- Severity: Error in V1. Solc treats shadowing as warning. Deviation
  is documented here. Follow-up Warning severity infra is proposed
  but not created as a task in this change.

## Unused

- Scope: `analysis::unused` plus `Analyzer::diagnostics` and
  `diagnostics_ctx`. Reuses `find_references` for variables, token
  scan for imports. No new `SymbolKind`.
- Rule: named params (with body) and locals in `all_locals` with no
  non-declaration reference error with `unused parameter 'x'` or
  `unused variable 'x'` at the name span. Named imports with no use
  outside the import span error once per directive with
  `unused import './B.sol'` at the path span.
- Silent: state vars, functions, structs, return params, catch
  params, assembly, builtins, empty names. Plain
  `import "./A.sol";` stays silent. Results dedupe by span so one
  decl gives one diagnostic.
- Severity: Error in V1, same deviation and follow-up note as above.

## Data location

- Scope: `analysis::data_location` plus `Param::data_location`,
  `StateVar::data_location`, `LocalVar::data_location` and spans.
  Parser stores `memory`, `storage`, `calldata` tokens between type
  and name. Name lookup skips them so `memory` never reads as name.
  Audit: `Param::name`, `var_name_and_type`, and both `name_span`
  helpers skip location kinds. `TypeNode::text` is unchanged since
  location lives outside the type node.
- Rules (sound subset):
  - State vars with any location error with
    `data location 'memory' not allowed for state variable`.
  - Value params and locals (`bool`, `address`, `uint`, `int`,
    sized `bytes`, `fixed`) with any location error with
    `data location 'memory' not allowed for 'uint'`.
  - Reference params (`string`, `bytes`, arrays, mappings) without
    location error with `data location must be specified for 'string'`.
    Mappings need `storage`, other mappings error with
    `mapping can only be 'storage', found 'memory'`.
  - Locals with `calldata` error with
    `data location 'calldata' not allowed for local variable`.
    Other reference locals without location error the same as params.
- Silent: user types (struct versus contract cannot be told from
  text, except arrays which still check), struct fields, event and
  error params.
- Severity: Error in V1, same deviation and follow-up note as above.

## Interface rules

- Scope: `analysis::interface` plus `ContractDef::kind_label`. No new
  `SymbolKind`, no grammar change.
- Rules:
  - State variables in an `interface` error with
    `interface 'I' cannot have state variable 'x'`.
  - Constructors in an `interface` error with
    `interface 'I' cannot have constructor`.
- Silent: plain contracts and libraries stay silent here. Clean
  interfaces with only functions stay silent.
- Deferred: library rules fully deferred. Constant versus immutable
  needs parser support that does not exist yet, so no library check
  runs. No half implementation.

## Event indexed limit

- Scope: `analysis::event` plus `Param::is_indexed`,
  `Param::indexed_span`, `EventDef::is_anonymous`, and
  `EventDef::anonymous_span`. Parser stores `indexed` in `Param` and
  `anonymous` in `Event`. Name lookup skips `indexed` so param names
  stay correct.
- Rule: non-anonymous events with more than 3 indexed params error
  with `event 'E' has N indexed params but max is 3` at the event
  name. File level and contract events both lint.
- Silent: 3 or fewer indexed params stay silent. Anonymous events
  stay silent here.
- Deferred: mapping indexed, string or bytes indexed, anonymous
  limits and combos. Spec is unverified, so no check runs for them.

## Pragma version (deferred)

- Status: no check in V1 by design, documented here only.
- Rationale: without a configured compiler version the check has no
  meaning. A pragma such as `pragma solidity ^0.8.0` cannot pass or
  fail without knowing which compiler runs. Flagging it would be
  noise, so the analyzer stays silent on all pragmas.
- Follow-up needs a compiler version source (config, toolchain file,
  or editor setting) before any range check makes sense. No task
  created in this change.

## Code actions

- Scope: `analysis::code_action` plus `Analyzer::code_actions` and the
  `textDocument/codeAction` dispatch. Capability
  `codeActionProvider: true`.
- Shape: `CodeActionData {title, kind, edit}`. Dispatch maps `edit` to
  `WorkspaceEdit` changes. Kind is `quickfix`, no `command`, no
  `resolve` in V1. Caps at `MAX_CODE_ACTIONS = 10`.
- Import: unresolved name in `range` with exactly one tracked provider
  proposes `Import 'X' from "./File.sol"` inserting at offset 0.
  Ambiguous names, missing names, dep-available names, and already
  imported files stay silent.
- Override: O1 diagnostic in `range` proposes `Add 'override' to 'f'`
  inserting `override` before the function body. Other override
  diagnostics and bodyless functions stay silent.
- Noir: trait default returns `[]`, no noir grammar change.

## Folding and selection

- Scope: `analysis::folding` plus `Analyzer::folding_ranges` and the
  `textDocument/foldingRange` dispatch, plus `analysis::selection`
  plus `Analyzer::selection_range` and the
  `textDocument/selectionRange` dispatch. Capabilities
  `foldingRangeProvider: true` and `selectionRangeProvider: true`.
- Folding shape: `FoldingRangeData {span}`. Dispatch maps `span` to
  `startLine` and `endLine`. Caps at `MAX_FOLDING_RANGES = 200`.
- Folding source: nested contract, function, and block spans in source
  order. Single line spans drop. Empty text returns empty.
- Selection shape: `Vec<Span>` innermost first. Dispatch nests each
  entry into `parent` links. Empty chains fall back to a zero length
  range at the cursor. Caps at `MAX_SELECTION_DEPTH = 20`.
- Selection source: word at cursor, then inner blocks out to the body,
  then function, contract, and file span. Dedupes exact duplicates.
- Noir: trait defaults return `[]`, no noir grammar change.

## Type hierarchy

- Scope: `analysis::type_hierarchy` plus `Analyzer::prepare_type_hierarchy`,
  `supertypes`, `subtypes` and three dispatches
  (`prepareTypeHierarchy`, `supertypes`, `subtypes`).
  Capability `typeHierarchyProvider: true`.
- Shape: `TypeHierarchyItemData {name, kind, detail, path, span,
  selection}`. Dispatch maps spans with tracked or disk text, skips
  out-of-bounds, caps at `MAX_TYPE_HIERARCHY = 50`.
- Prepare: contract at the cursor only. Nameless contracts and outside
  positions return `None`.
- Supertypes: `linearize` same file bases plus `linearize_across` dep
  bases in heritage order, transitive first. Dedupes by name.
- Subtypes: scans tracked Solidity files path-sorted for heritage with
  the exact target name. Prefix and case variants never count. Target
  itself is skipped.
- Noir: trait defaults return `None`, no noir grammar change.

## Assembly (Yul)

- Scope: `analysis::yul` plus thin wiring in `unknown`, `completion`,
  `hover`, `goto`, `references`, and `parser::skip_assembly`. No new
  `SymbolKind`, no grammar change.
- Shape: flat token scan inside `assembly_ranges`. `let x, y` collects
  each name as `Let`, `function f(params) -> rets` collects the name as
  `Function` plus each param and return name as `Param`. Assignment uses
  (`x := ...`) never count as decls.
- Builtins: fixed opcode subset (`add`, `sstore`, `keccak256`, and rest)
  with `function <name> - from yul` detail. `is_yul_builtin` gates
  completion, hover, and goto. Arity stays deferred, so calls never lint.
- Order: `find_yul_decl` keeps only decls with `start <= offset`, latest
  wins. Forward uses never resolve to later decls.
- Parse: `skip_assembly` takes `errors` and reports `unclosed assembly`
  when `{` never closes. Balanced bodies stay silent, surrounding locals
  keep parsing.
- Unknown: assembly idents resolve Yul decl, then Yul builtin, then outer
  scope and dep exports. Remaining idents stay silent: unknown opcodes
  are deferred.
- Completion: assembly positions suggest flat Yul decls plus Yul builtins
  plus outer scope, filtered by prefix. Yul decls win over outer names.
- Hover: Yul decl shows `let x - from yul` with the decl span, builtin
  shows `function add - from yul` with the cursor span, outer names reuse
  scope hover with docs.
- Goto: Yul decl jumps to its decl span. Yul builtin returns `None` by
  design with a dep guard, so opcodes never jump to coincidental matches.
- References: plain scan keeps outer uses inside assembly. Yul builtins,
  Yul decl tokens, and Yul shadowed names stay skipped. Member and emit
  scans still skip assembly.
- Limits: nested scoping stays flat, so `let x := x` may resolve to
  itself. Yul arity, unknown opcodes, Yul decl targets for references
  and rename, plus outline, semantic, and inlay for Yul stay deferred.

## V1 limits

- Tracked only: `dep_texts` skips untracked and missing files.
- Caps: dep read cap is 16, republish fan-out cap is `MAX_DEPENDENT_REPUBLISH = 32`, workspace symbols cap is `MAX_WORKSPACE_SYMBOLS = 50`, references cap is `MAX_REFERENCES = 100`, inlay cap is `MAX_INLAY_HINTS = 100`, semantic cap is `MAX_SEMANTIC_TOKENS = 5000`, call hierarchy cap is `MAX_CALL_HIERARCHY = 50`, code action cap is `MAX_CODE_ACTIONS = 10`, folding cap is `MAX_FOLDING_RANGES = 200`, selection depth cap is `MAX_SELECTION_DEPTH = 20`, type hierarchy cap is `MAX_TYPE_HIERARCHY = 50`, chain tails cap is `MAX_CHAIN_TAILS = 4`.
- Symbols: `Contract`, `Function`, `Struct`, `Enum`, `Error`, `Event`, `Modifier`, and `UserType` cross files. `StateVar`, `Param`, and `LocalVar` stay local.
- Imports: union without alias precision. `{B as C}` records both `B` and `C`, no alias mapping.
- Parse: parse on each call, no incremental cache.
- Inheritance: C3 with DFS fallback plus `inconsistent linearization` diagnostic, unknown bases ignored, cycles end via visited set, `exports_of` stays own members.
- Chained: `a.b.c` resolves final only with cross-file lookup per level, intermediate calls skipped, builtins terminate, intermediates stay silent.
