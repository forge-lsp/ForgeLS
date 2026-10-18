use forgels_syntax::Span;

use super::diagnostic::{Applicability, LintFix};

// Outcome of a pure fix run: new text plus applied/skipped counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixOutcome {
    pub text: String,
    pub applied: usize,
    pub skipped: usize,
}

// Same overlap test as the Solidity code actions: touching edges count
// as overlap, so adjacent inserts never merge silently.
fn overlaps(left: Span, right: Span) -> bool {
    left.start <= right.end && left.end >= right.start
}

// Apply safe fixes to text. Sorts by span, keeps the first of any
// overlapping pair, skips unsafe and out-of-range fixes. Pure: input
// text is never mutated, LSP code actions are untouched.
pub fn apply(text: &str, fixes: &[LintFix]) -> FixOutcome {
    if text.is_empty() || fixes.is_empty() {
        return FixOutcome {
            text: text.to_string(),
            applied: 0,
            skipped: fixes.len(),
        };
    }
    let mut ordered: Vec<&LintFix> = fixes
        .iter()
        .filter(|fix| fix.applicability == Applicability::Safe)
        .collect();
    let mut skipped = fixes.len() - ordered.len();
    ordered.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then(left.span.end.cmp(&right.span.end))
    });
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut kept: Option<Span> = None;
    let mut applied = 0usize;
    for fix in ordered {
        if let Some(prev) = kept
            && overlaps(prev, fix.span)
        {
            skipped += 1;
            continue;
        }
        let start = fix.span.start as usize;
        let end = fix.span.end as usize;
        let (Some(head), Some(_)) = (text.get(cursor..start), text.get(start..end)) else {
            skipped += 1;
            continue;
        };
        out.push_str(head);
        out.push_str(&fix.replacement);
        cursor = end;
        kept = Some(fix.span);
        applied += 1;
    }
    out.push_str(&text[cursor..]);
    FixOutcome {
        text: out,
        applied,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn safe(start: u32, end: u32, replacement: &str) -> LintFix {
        LintFix::safe(Span::new(start, end), replacement.to_string())
    }

    #[test]
    fn empty_input_applies_nothing() {
        let out = apply("", &[safe(0, 0, "x")]);
        assert_eq!(out.text, "");
        assert_eq!(out.applied, 0);
        assert_eq!(out.skipped, 1);
    }

    #[test]
    fn sorts_fixes_before_applying() {
        let out = apply("abcd", &[safe(2, 3, "X"), safe(0, 1, "Y")]);
        assert_eq!(out.text, "YbXd");
        assert_eq!(out.applied, 2);
        assert_eq!(out.skipped, 0);
    }

    #[test]
    fn overlapping_fix_keeps_first_only() {
        let out = apply("abcd", &[safe(0, 2, "X"), safe(1, 3, "Y")]);
        assert_eq!(out.text, "Xcd");
        assert_eq!(out.applied, 1);
        assert_eq!(out.skipped, 1);
    }

    #[test]
    fn unsafe_fix_is_skipped() {
        let fix = LintFix {
            span: Span::new(0, 1),
            replacement: "X".to_string(),
            applicability: Applicability::Unsafe,
        };
        let out = apply("abcd", &[fix]);
        assert_eq!(out.text, "abcd");
        assert_eq!(out.applied, 0);
        assert_eq!(out.skipped, 1);
    }

    #[test]
    fn out_of_range_fix_is_skipped() {
        let out = apply("ab", &[safe(0, 1, "X"), safe(9, 12, "Y")]);
        assert_eq!(out.text, "Xb");
        assert_eq!(out.applied, 1);
        assert_eq!(out.skipped, 1);
    }
}
