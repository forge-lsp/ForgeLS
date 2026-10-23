# Formatter

Scope: `forgels-fmt` crate plus `fls format` wiring. Solidity only in V1.

## Options

- Typed keys from `fls.json` `format` group: `line_length`, `tab_width`, `style`.
- Defaults: `line_length` 120, `tab_width` 4, `style` space.
- `style` is `space` or `tab`. Space repeats `tab_width` spaces per level.
  Tab emits one `\t` per level; width only counts tab display size.
- `quote_style` and `bracket_spacing` parse and store but stay unused.
  Follow-up adds quote and bracket rules.
- Legacy `forgels.toml` carries lint only and yields default format.
  Use `fls.json` for format keys.
- Unknown format keys fail fast: bad top-level keys are parse errors,
  bad `rules` keys are unknown-rule errors.

## Rules

- Trim: trailing spaces and tabs removed, file ends with exactly one `\n`.
  Whitespace-only files become one `\n`. CRLF bodies keep `\r\n`, EOF stays LF.
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
- Wrap (threshold `line_length`, continuation from options):
  - Candidates: plain calls, member calls, `emit`, `revert`, `new`,
    function params, event params, error params, `returns`.
  - Trigger: single line over `line_length` wraps; already-multiline with two
    or more args normalizes to canonical. Empty calls never wrap.
    Single-arg wraps only when over `line_length`.
  - Width counts tabs as `tab_width`. Continuation is statement indent
    plus one indent unit (spaces or one tab).
  - Canonical: `(` stays on the callee line, first arg on its own
    line, one arg per line, lone `)` on the final line at statement
    indent.
  - Options `f{value: 1}(args)` stay glued on the callee line;
    wrapping starts at `(`.
  - Any comment intersecting the arg list blocks that call.
  - Outermost wins: overlapping candidates sort outer first
    (start asc, end desc) so apply keeps the outer edit.
  - Regions of wrapped calls are returned alongside edits so indent
    can skip their inner lines.
- Blank (textual gaps, brace depth, mirrors lint counts):
  - Depth 0 gaps get 2 blanks between top-level decls.
  - Depth 1 function-pair gaps get 1 blank.
  - Single-line pair exemption: two single-line functions stay as-is.
  - Other member gaps (state vars, mixed members) stay unchecked.
  - Comments never move: only blank lines are inserted or deleted.
    Comment blocks get desired blanks on both leading and trailing sides.
  - Empty `{}` with interior newlines collapses to `{}` unless the gap
    holds a comment.
  - Newline style is picked once per file: CRLF when text holds `\r\n`,
    else LF.
- Indent (brace depth, leading whitespace only):
  - Tracks `{` and `}` outside strings and comments.
  - Lines starting with `}` dedent one level.
  - Desired indent is the options unit repeated by depth.
  - Skips lines inside wrap regions except their first lines.
  - Skips comment-only lines (stripped starts with `//`, `/*`, `*`).
  - Code with a trailing comment still reindents its leading gap.
  - Assembly stays verbatim from the `assembly` keyword through its
    matching brace.
  - CRLF kept: only leading spaces and tabs change. EOF stays with trim.

## Pipeline

- Per iteration order: comment spans, spacing, wrap (edits plus regions),
  blank, indent, trim. One apply per iteration. Regions recomputed each pass.
- Fixpoint caps at 4 passes with early breaks on no edits or stable text.
- Empty input returns empty fast.

## Plugin gate

- Top-level `plugin` controls builtins: absent or listing `forge:format`
  runs builtin convergence, unlisted or empty skips builtins.
- Guests from `lint.external` always run when present, even when
  builtins are skipped. See `docs/plugins.md` for sandbox and ABI.

## Gaps

- Custom modifiers and base args wrap as plain calls: the text
  scanner classifies by name shape only.
- Braced args (`f({a: 1})`) never wrap: brace scan aborts the candidate.
- Function types (`function (uint)`) never wrap: no name prefix.
- Control parens (`if`, `for`, `while`, `catch`) never wrap.
- `mapping(`, `type(`, `receive(`, `fallback(` never wrap: glue rule.
- Blank function detection is textual: `function` header search within a
  small window, not full AST.

## Idempotency

- Every fixture asserts double format: `format(format(x)) == format(x)`.
- Canonical wrap output is detected and left untouched on second pass.
- Spacing skips newline gaps (except brace joins) so wrapped
  newlines survive spacing.
- Blank and indent outputs are stable: second pass finds no gaps.
- CLI `format --write` twice yields identical bytes; `--check`
  exits 0 after the first write.
