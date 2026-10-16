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

## V1 limits

- Tracked only: `dep_texts` skips untracked and missing files.
- Caps: dep read cap is 16, republish fan-out cap is 32.
- Symbols: `Contract` and `Function` cross files. `StateVar`, `Param`, and `LocalVar` stay local.
- Imports: union without alias precision. `{B as C}` records both `B` and `C`, no alias mapping.
- Parse: parse on each call, no incremental cache.
