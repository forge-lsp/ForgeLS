use forgels_syntax::Span;

// Single span replacement. Empty replacement deletes, empty span inserts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatEdit {
    pub span: Span,
    pub replacement: String,
}

impl FormatEdit {
    pub fn new(span: Span, replacement: String) -> Self {
        Self { span, replacement }
    }
}

// Outcome of a pure format run: new text plus edit counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOutcome {
    pub text: String,
    pub applied: usize,
    pub skipped: usize,
}

fn overlaps(left: Span, right: Span) -> bool {
    left.start <= right.end && left.end >= right.start
}

// Apply edits to text. Sorts outermost first (start asc, end desc),
// keeps the first of any overlapping pair (touching edges count),
// skips out-of-range edits via `str::get`. Pure: input never mutates.
pub fn apply(text: &str, edits: &[FormatEdit]) -> FormatOutcome {
    if text.is_empty() || edits.is_empty() {
        return FormatOutcome {
            text: text.to_string(),
            applied: 0,
            skipped: edits.len(),
        };
    }
    let mut ordered: Vec<&FormatEdit> = edits.iter().collect();
    ordered.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then(right.span.end.cmp(&left.span.end))
    });
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut kept: Option<Span> = None;
    let mut applied = 0usize;
    let mut skipped = 0usize;
    for edit in ordered {
        if let Some(prev) = kept
            && overlaps(prev, edit.span)
        {
            skipped += 1;
            continue;
        }
        let start = edit.span.start as usize;
        let end = edit.span.end as usize;
        let (Some(head), Some(_)) = (text.get(cursor..start), text.get(start..end)) else {
            skipped += 1;
            continue;
        };
        out.push_str(head);
        out.push_str(&edit.replacement);
        cursor = end;
        kept = Some(edit.span);
        applied += 1;
    }
    out.push_str(&text[cursor..]);
    FormatOutcome {
        text: out,
        applied,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(start: u32, end: u32, replacement: &str) -> FormatEdit {
        FormatEdit::new(Span::new(start, end), replacement.to_string())
    }

    #[test]
    fn empty_input_applies_nothing() {
        let out = apply("", &[edit(0, 0, "x")]);
        assert_eq!(out.text, "");
        assert_eq!(out.applied, 0);
        assert_eq!(out.skipped, 1);
    }

    #[test]
    fn sorts_edits_before_applying() {
        let out = apply("abcd", &[edit(2, 3, "X"), edit(0, 1, "Y")]);
        assert_eq!(out.text, "YbXd");
        assert_eq!(out.applied, 2);
        assert_eq!(out.skipped, 0);
    }

    #[test]
    fn overlapping_edit_keeps_first_only() {
        let out = apply("abcd", &[edit(0, 2, "X"), edit(1, 3, "Y")]);
        assert_eq!(out.text, "Xcd");
        assert_eq!(out.applied, 1);
        assert_eq!(out.skipped, 1);
    }

    #[test]
    fn touching_edges_count_as_overlap() {
        let out = apply("abcd", &[edit(0, 2, "X"), edit(2, 2, "Y")]);
        assert_eq!(out.text, "Xcd");
        assert_eq!(out.applied, 1);
        assert_eq!(out.skipped, 1);
    }

    #[test]
    fn out_of_range_edit_is_skipped() {
        let out = apply("ab", &[edit(0, 1, "X"), edit(9, 12, "Y")]);
        assert_eq!(out.text, "Xb");
        assert_eq!(out.applied, 1);
        assert_eq!(out.skipped, 1);
    }
}
