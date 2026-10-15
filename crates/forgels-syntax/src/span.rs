#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// Build a span. Inverted ends normalize to an empty span at `start`,
    /// so `len` always equals `end - start` and positions never go backwards.
    pub fn new(start: u32, end: u32) -> Self {
        Self {
            start,
            end: end.max(start),
        }
    }

    /// Zero-width span at a cursor offset.
    pub fn empty_at(offset: u32) -> Self {
        Self::new(offset, offset)
    }

    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// True when `offset` hits the span. Empty spans contain only their
    /// own offset. Non-empty spans are half open: end stays exclusive so
    /// adjacent tokens never share a hit.
    pub fn contains(&self, offset: u32) -> bool {
        if self.is_empty() {
            return offset == self.start;
        }
        self.start <= offset && offset < self.end
    }

    /// Clamp both ends into `[0, file_len]`. Keeps order.
    pub fn clamp(&self, file_len: u32) -> Self {
        let start = self.start.min(file_len);
        Self::new(start, self.end.min(file_len).max(start))
    }

    /// Smallest span covering both.
    pub fn join(&self, other: &Span) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_span_is_empty() {
        let span = Span::new(0, 0);
        assert!(span.is_empty());
        assert_eq!(span.len(), 0);
        assert!(span.contains(0));
        assert!(!span.contains(1));
    }

    #[test]
    fn start_of_file_contains_from_zero() {
        let span = Span::new(0, 3);
        assert!(span.contains(0));
        assert!(span.contains(2));
        assert!(!span.contains(3));
    }

    #[test]
    fn end_of_file_span_stays_inside() {
        let text_len = 12;
        let span = Span::new(9, 12).clamp(text_len);
        assert_eq!(span, Span::new(9, 12));
        assert!(span.contains(11));
        assert!(!span.contains(12));
    }

    #[test]
    fn inverted_ends_normalize_to_empty() {
        let span = Span::new(5, 3);
        assert_eq!(span, Span::new(5, 5));
        assert!(span.is_empty());
        assert_eq!(span.len(), 0);
    }

    #[test]
    fn clamp_keeps_order_inside_file() {
        assert_eq!(Span::new(0, 99).clamp(10), Span::new(0, 10));
        assert_eq!(Span::new(8, 99).clamp(10), Span::new(8, 10));
        assert_eq!(Span::new(20, 30).clamp(10), Span::new(10, 10));
    }

    #[test]
    fn len_matches_end_minus_start() {
        for (start, end) in [(0, 0), (0, 5), (3, 3), (7, 42)] {
            let span = Span::new(start, end);
            assert_eq!(span.len(), span.end - span.start);
        }
    }

    #[test]
    fn join_covers_both() {
        assert_eq!(Span::new(1, 3).join(&Span::new(5, 7)), Span::new(1, 7));
        assert_eq!(Span::new(5, 7).join(&Span::new(1, 3)), Span::new(1, 7));
    }

    #[test]
    fn empty_at_marks_cursor_only() {
        let span = Span::empty_at(4);
        assert!(span.is_empty());
        assert!(span.contains(4));
        assert!(!span.contains(3));
        assert!(!span.contains(5));
    }
}
