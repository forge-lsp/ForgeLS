# Formatter

Scope: `forgels-fmt` crate plus `fls format` wiring. Solidity only in V1.

## Rules

- Trim: trailing spaces and tabs removed, file ends with exactly one `\n`.
  Whitespace-only files become one `\n`. CRLF bodies keep `\r\n`.
- Spacing (token gaps, never inside strings, comments block adjacent gaps):
  - Zero space after `(` and `[`, before `)`, `]`, `,`, `;`.
  - Single space after `,` and after `;` unless next is `)`, `]`,
    newline, or EOF. `; }` keeps one space.
  - Single space around assignment family only: `=`, `+=`, `-=`,
    `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`, `>>>=`.
    Other ops (`==`, `+`, `-`, `*`, `/`, etc) stay untouched.
  - `**` stays glued with no spaces.
  - `if`, `for`, `while` take one space before `(`.
  - `) {` shares one line, `} else {` shares one line.
  - `receive(`, `fallback(`, `mapping(`, `type(` stay glued.
- Wrap (fixed width 120, indent 4, no config keys):
  - Candidates: plain calls, member calls, `emit`, `revert`, `new`,
    function params, event params, error params, `returns`.
  - Trigger: single line over 120 wraps; already-multiline with two
    or more args normalizes to canonical. Empty calls never wrap.
    Single-arg wraps only when over 120.
  - Canonical: `(` stays on the callee line, first arg on its own
    line, one arg per line, lone `)` on the final line at statement
    indent. Continuation is statement indent plus 4 spaces.
  - Options `f{value: 1}(args)` stay glued on the callee line;
    wrapping starts at `(`.
  - Any comment intersecting the arg list blocks that call.
  - Outermost wins: overlapping candidates sort outer first
    (start asc, end desc) so apply keeps the outer edit.

## Gaps

- Custom modifiers and base args wrap as plain calls: the text
  scanner classifies by name shape only.
- Braced args (`f({a: 1})`) never wrap: brace scan aborts the candidate.
- Function types (`function (uint)`) never wrap: no name prefix.
- Control parens (`if`, `for`, `while`, `catch`) never wrap.
- `mapping(`, `type(`, `receive(`, `fallback(` never wrap: glue rule.

## Idempotency

- Every fixture asserts double format: `format(format(x)) == format(x)`.
- Canonical wrap output is detected and left untouched on second pass.
- Spacing skips newline gaps (except brace joins) so wrapped
  newlines survive spacing.
- CLI `format --write` twice yields identical bytes; `--check`
  exits 0 after the first write.
