# fls

Scope: `fls` crate. LSP server plus command line check and format for Solidity.

## Usage

```bash
fls lsp [--log-level LEVEL]
fls check [PATH...] [--write] [--json]
fls format [PATH...] [--write] [--check]
fls init [--force]
```

- `lsp` runs the LSP server on stdio with Solidity wired in.
- `check` lints Solidity files. No path scans cwd recursively for `*.sol`.
- `format` formats Solidity files. No flag prints formatted text to stdout without writing.
- `format` scope is trim plus spacing plus wrap plus blank plus indent. See `docs/formatter.md`.
- Explicit format paths accept files or dirs. Output is sorted and deduped.
- Walk skips `node_modules`, `out`, `cache`, `artifacts`, `coverage` at any depth.
- Skip applies to every scan including explicit dirs; explicit files are always honored.
- Raw excludes prune a dir when the pattern equals its walker-relative posix path or that path plus `/**`.
- Truncated walks print `fls: walk truncated, results may be incomplete` on stderr.
- Closed stdout pipes exit quiet with the computed code; other write failures are exit 2.
- `--write` applies safe fixes in place.
- `--json` emits the engine JSON report as-is.
- Only files with warn/err findings appear; clean files stay silent.
- `init` scaffolds `./fls.json` with explicit defaults.

## Flags

- `lsp [--log-level LEVEL]`: level is trace, debug, info, warn, or error. Flag wins over config.
- `check [PATH...]`: files or dirs. Empty means cwd scan.
- `--write`: write safe fixes. Only safe applicability applies.
- `--json`: machine output with 0-based ranges, fix, fixed, summary.
- `format [PATH...]`: files or dirs. Empty means cwd scan. Solidity only in V1.
- `format --write`: write formatted text back in place.
- `format --check`: list files that would change, no writes. `--write` plus `--check` together is exit 2.
- `init [--force]`: write `./fls.json`. Without `--force`, an existing file is exit 2.

## Exit codes

- `0`: clean or warnings only. Format preview and `--write` always exit 0 unless operational failure.
- `1`: any error diagnostic. Format `--check` exits 1 when any file would change.
- `2`: operational failure (unreadable file, bad write, unknown rule, conflicting format flags).

## Config

- Precedence is `./fls.json` wins, else `./forgels.toml` lint text, else defaults.
- `init` writes top-level `plugin` plus grouped `lint` and explicit
  `format` via `serde_json::to_string_pretty`.
- Top-level `plugin` is a string array, for example
  `{"plugin":["forge:correctness"]}` in JSON and
  `plugin = ["forge:correctness"]` in TOML. Absent means empty.
  Bad shape is a parse error. V1 parses only: no gating, no validation.
- Old `[lint] plugins` and `lint.plugins` are parse errors naming
  top-level `plugin`. Move entries to the top-level array.
- `init` defaults are `["forge:correctness", "forge:style",
  "forge:safety", "forge:format"]` plus empty lint `external`.
- Lint group holds `includes`, `excludes`, `rules`, and `external`.
- `lint.external` is `[{"path":"plugins/check.wasm"}]` with local
  paths only. URLs fail. TOML `external` is a parse error pointing
  to `fls.json` (JSON-only V1). See `docs/plugins.md`.
- Guest rule IDs in `lint.rules` validate against declared guest
  IDs; truly unknown stays exit 2. Guests load once at startup.
- Format group holds `includes`, `excludes`, `line_length`, `tab_width`,
  `style`, and `rules`. Defaults are 120, 4, space. Missing keys fall back.
- `quote_style` and `bracket_spacing` parse and store but stay unused.
  Follow-up adds their rules.
- Legacy `forgels.toml` yields default format only. Use `fls.json` for keys.
- Tab style emits one `\t` per indent level; width counts tabs as `tab_width`.
- CRLF bodies keep `\r\n`; EOF newline stays LF.
- Unknown format keys fail fast: bad top-level keys warn plus defaults,
  bad lint rule IDs stay operational failures.
- Empty lint `includes` matches all files. `excludes` remove after include.
- `check` filters scanned files by cwd-relative posix path before analysis.
- Invalid glob patterns warn once on stderr and are ignored.
- `lsp` enforces the same lint levels and filters on publish. Excluded files publish empty.
- `lsp` loads config once at startup. Restart required after config edits.
- No upward search: only cwd files are read. Hot reload is deferred.

## Limits

- Single-file analysis: dep-aware rules run without tracked deps.
- Import resolution uses cwd or the config root.
- V1 is Solidity only. Noir stays out to keep the binary small.
