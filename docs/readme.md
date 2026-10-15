# ForgeLS Docs Index

Scope: whole workspace. Entry point for official docs.

ForgeLS is a Language Server Protocol for smart contract languages such as Solidity and Noir. It is built in Rust as a monorepo with shared crates and independent language crates.

## Map

- `architecture.md`: workspace layout, crate roles, dependency rules.
- `syntax-toolkit.md`: shared AST, lexer, and parser toolkit in `forgels-syntax`.
- `lang-solidity.md`: Solidity crate scope and status.
- `lang-noir.md`: Noir crate scope and status.
- `caches-guide.md`: how to use `.caches/` for plan, eval, mem, and temp.

## Rules for all docs

- Language: English only.
- Style: direct and compact. Prefer bullets and code.
- Use hyphen `-` or colon `:` for breaks. Do not use em dash.
- Use plain ASCII only. Avoid symbols that search cannot find.
