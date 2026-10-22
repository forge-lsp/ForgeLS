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
- `format` V1 scope is trim plus spacing plus 120-column call wrap. See `docs/formatter.md`.
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
- `init` writes grouped `lint` plus reserved `format` via `serde_json::to_string_pretty`.
- Lint group holds `includes`, `excludes`, and `rules`. Format group is parsed and stored only.
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
