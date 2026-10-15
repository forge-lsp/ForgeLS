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
  `true`, `false`.
- Sized types: `uint8` to `uint256` and `int8` to `int256` in steps of 8,
  `bytes1` to `bytes32`.
- Literals: decimal and hex numbers, single and double quoted strings.
- Symbols: braces, brackets, parens, arithmetic, comparison, bitwise,
  assignment combos, `=>`, `?`, `;`, `,`, `:`, `.`.
- Comments: `//` line and `/* */` block. Whitespace is skipped.
