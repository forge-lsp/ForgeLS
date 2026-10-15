# Noir Support

Scope: `forgels-lang-noir` crate.

- Language id: `noir`
- Extensions: `.nr`
- Modules: `ast`, `lexer`, `parser`
- Base traits: `forgels-syntax::Lexer`, `forgels-syntax::Parser`, `forgels-syntax::AstNode`
- Allowed deps: `forgels-core`, `forgels-protocol`, `forgels-syntax`, `forgels-common`
- Forbidden deps: any other `forgels-lang-*`

Status: stub lexer and parser. Real grammar is planned.
