# Solidity Support

Scope: `forgels-lang-solidity` crate.

- Language id: `solidity`
- Extensions: `.sol`
- Modules: `ast`, `lexer`, `parser`
- Base traits: `forgels-syntax::Lexer`, `forgels-syntax::Parser`, `forgels-syntax::AstNode`
- Allowed deps: `forgels-core`, `forgels-protocol`, `forgels-syntax`, `forgels-common`
- Forbidden deps: any other `forgels-lang-*`

Status: stub lexer and parser. Real grammar is planned.
