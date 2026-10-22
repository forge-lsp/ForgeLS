use forgels_syntax::Span;

use super::edit::{FormatOutcome, apply};
use super::trim::trim_edits;
use super::whitespace::space_edits;
use super::wrap::wrap_edits;

// Solidity-only V1 entry point. Noir stays out: no second grammar is
// wired until the formatter rules settle. Pipeline loops comment guard,
// whitespace edits, wrap edits, trim edits to fixpoint (cap 4) so one
// call yields spaced plus wrapped text. Width 120 and indent 4 are
// fixed with no config keys.
pub fn format_text(text: &str) -> FormatOutcome {
    if text.is_empty() {
        return FormatOutcome {
            text: String::new(),
            applied: 0,
            skipped: 0,
        };
    }
    let mut current = text.to_string();
    let mut applied_total = 0usize;
    let mut skipped_total = 0usize;
    for _ in 0..4 {
        let guard = comment_spans(&current);
        let mut edits = space_edits(&current, &guard);
        edits.extend(wrap_edits(&current, &guard));
        edits.extend(trim_edits(&current, &guard));
        if edits.is_empty() {
            break;
        }
        let outcome = apply(&current, &edits);
        if outcome.applied == 0 {
            skipped_total += outcome.skipped;
            break;
        }
        applied_total += outcome.applied;
        skipped_total += outcome.skipped;
        if outcome.text == current {
            break;
        }
        current = outcome.text;
    }
    FormatOutcome {
        text: current,
        applied: applied_total,
        skipped: skipped_total,
    }
}

// Minimal comment spans (`//` to newline, `/*` to close or EOF).
// Guards trim edits only; full re-lex stays P2. Strings are skipped
// so quoted markers never count as comments.
pub(crate) fn comment_spans(text: &str) -> Vec<Span> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            let start = index;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            out.push(Span::new(start as u32, index as u32));
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let start = index;
            index += 2;
            while index < bytes.len() {
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    index += 2;
                    break;
                }
                index += 1;
            }
            out.push(Span::new(start as u32, index as u32));
        } else if byte == b'"' || byte == b'\'' {
            let quote = byte;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else if bytes[index] == quote {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
        } else {
            let width = text[index..]
                .chars()
                .next()
                .map(|first| first.len_utf8())
                .unwrap_or(1);
            index += width;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_has_no_edits() {
        let out = format_text("contract A {}\n");
        assert_eq!(out.text, "contract A {}\n");
        assert_eq!(out.applied, 0);
    }

    #[test]
    fn trims_and_ends_with_single_newline() {
        let out = format_text("contract A {}   ");
        assert_eq!(out.text, "contract A {}\n");
    }

    #[test]
    fn comment_line_keeps_trailing_spaces() {
        let out = format_text("// hi   \n");
        assert_eq!(out.text, "// hi   \n");
    }

    #[test]
    fn code_line_still_trims() {
        let out = format_text("uint x;   \n");
        assert_eq!(out.text, "uint x;\n");
    }

    #[test]
    fn format_is_idempotent() {
        for sample in [
            "contract A {}   \n\n",
            "a;\r\nb;  \r\n",
            "a;\n   \n",
            "",
            "   \n",
            "// hi   \n",
            "uint x;\t\n",
        ] {
            let once = format_text(sample).text;
            let twice = format_text(&once).text;
            assert_eq!(once, twice, "idempotent for {sample:?}");
        }
    }

    #[test]
    fn block_comment_guards_covered_lines() {
        let text = "/* a;   \n   b; */\nuint x;   \n";
        let out = format_text(text);
        assert_eq!(out.text, "/* a;   \n   b; */\nuint x;\n");
    }

    #[test]
    fn quoted_markers_are_not_comments() {
        assert!(comment_spans("\"//\";\n").is_empty());
    }

    fn golden(input: &str) -> String {
        format_text(input).text
    }

    fn assert_golden_idempotent(input: &str) {
        let once = golden(input);
        let twice = golden(&once);
        assert_eq!(once, twice, "format idempotent for {input:?}");
    }

    #[test]
    fn pipeline_trims_and_spaces_simple_call() {
        let out = golden("contract C { function f() public { greet(a,b); } }   \n");
        assert_eq!(out, "contract C { function f() public { greet(a, b); } }\n");
        assert_golden_idempotent("contract C { function f() public { greet(a,b); } }   \n");
    }

    #[test]
    fn pipeline_wraps_long_plain_call() {
        let input = "contract C { function f() public { greet(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp); } }\n";
        let out = golden(input);
        assert!(out.contains("greet(\n"), "out: {out}");
        assert_golden_idempotent(input);
        assert_golden_idempotent(&out);
    }

    #[test]
    fn pipeline_wraps_long_member_call() {
        let input = "contract C { function f() public { obj.greet(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp); } }\n";
        let out = golden(input);
        assert!(out.contains("obj.greet(\n"), "out: {out}");
        assert_golden_idempotent(input);
    }

    #[test]
    fn pipeline_wraps_emit_revert_new() {
        for input in [
            "contract C { function f() public { emit Transfer(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo); } }\n",
            "contract C { function f() public { revert Insufficient(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm); } }\n",
            "contract C { function f() public { new Vault(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn); } }\n",
        ] {
            let out = golden(input);
            assert!(out.contains("(\n"), "out: {out}");
            assert_golden_idempotent(input);
        }
    }

    #[test]
    fn pipeline_wraps_params_and_returns() {
        for input in [
            "contract C { function add(uint256 aaaa, uint256 bbbb, uint256 cccc, uint256 dddd, uint256 eeee, uint256 ffff, uint256 gggg) public {} }\n",
            "contract C { event Transfer(address aaaa, address bbbb, address cccc, address dddd, address eeee, address ffff, address gggg, address hhhh); }\n",
            "contract C { error Insufficient(uint256 aaaa, uint256 bbbb, uint256 cccc, uint256 dddd, uint256 eeee, uint256 ffff, uint256 gggg); }\n",
            "contract C { function f() public returns (uint256 aaaa, uint256 bbbb, uint256 cccc, uint256 dddd, uint256 eeee, uint256 ffff, uint256 gggg) {} }\n",
        ] {
            let out = golden(input);
            assert!(out.contains("(\n"), "out: {out}");
            assert_golden_idempotent(input);
        }
    }

    #[test]
    fn pipeline_single_arg_short_stays() {
        let input = "contract C { function f() public { greet(a); } }\n";
        assert_eq!(golden(input), input);
        assert_golden_idempotent(input);
    }

    #[test]
    fn pipeline_comment_blocks_wrap() {
        let input = "contract C { function f() public { greet(aaaa, /* note */ bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn); } }\n";
        assert_eq!(golden(input), input);
        assert_golden_idempotent(input);
    }

    #[test]
    fn pipeline_whitespace_rules_apply() {
        assert_eq!(golden("uint a=1;\n"), "uint a = 1;\n");
        assert_eq!(golden("a ** b;\n"), "a**b;\n");
        assert_eq!(golden("if(a){}\n"), "if (a) {}\n");
        assert_eq!(
            golden("mapping (address => uint) m;\n"),
            "mapping(address => uint) m;\n"
        );
        assert_eq!(golden("receive () {}\n"), "receive() {}\n");
        for sample in [
            "uint a=1;\n",
            "a ** b;\n",
            "if(a){}\n",
            "mapping (address => uint) m;\n",
            "receive () {}\n",
        ] {
            assert_golden_idempotent(sample);
        }
    }

    #[test]
    fn pipeline_format_is_idempotent_on_all_variants() {
        for sample in [
            "contract C { function f() public { greet(a, b); } }\n",
            "contract C { function f() public { greet(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp); } }\n",
            "contract C { function f() public { obj.greet(a, b); } }\n",
            "contract C { function f() public { emit Transfer(a, b); } }\n",
            "contract C { function f() public { revert Insufficient(a, b); } }\n",
            "contract C { function f() public { new Vault(a, b); } }\n",
            "uint a=1;\n",
            "a ** b;\n",
            "if(a){}\n",
            "}else {\n",
            "receive () {}\n",
            "mapping (address => uint) m;\n",
        ] {
            assert_golden_idempotent(sample);
        }
    }

    #[test]
    fn pipeline_spacing_inside_wrap_is_stable_in_one_call() {
        let input = "contract C { function f() public { greet(a=1, b=2, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp); } }\n";
        let once = golden(input);
        assert!(once.contains("greet(\n"), "out: {once}");
        assert!(once.contains("a = 1,"), "out: {once}");
        assert!(once.contains("b = 2,"), "out: {once}");
        let twice = golden(&once);
        assert_eq!(once, twice, "one call converges for {input:?}");
    }

    #[test]
    fn pipeline_single_arg_boundary_120_stays_121_wraps() {
        let prefix = "contract C { function f() public { greet(";
        let suffix = "); } }";
        let at_120 = format!("{prefix}{}{suffix}\n", "a".repeat(73));
        let over_120 = format!("{prefix}{}{suffix}\n", "a".repeat(74));
        assert_eq!(at_120.lines().next().unwrap_or("").chars().count(), 120);
        assert_eq!(over_120.lines().next().unwrap_or("").chars().count(), 121);
        assert_eq!(golden(&at_120), at_120);
        let wrapped = golden(&over_120);
        assert!(wrapped.contains("greet(\n"), "out: {wrapped}");
        assert_golden_idempotent(&at_120);
        assert_golden_idempotent(&over_120);
    }

    #[test]
    fn pipeline_heritage_and_modifier_wrap_as_plain_calls() {
        let heritage = "contract C is Base(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp, qqqq, rrrr) {}\n";
        let modifier_def = "contract C { modifier myMod(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp, qqqq) {} }\n";
        let heritage_out = golden(heritage);
        assert!(heritage_out.contains("Base(\n"), "out: {heritage_out}");
        let modifier_out = golden(modifier_def);
        assert!(modifier_out.contains("myMod(\n"), "out: {modifier_out}");
        assert_golden_idempotent(heritage);
        assert_golden_idempotent(modifier_def);
    }
}
