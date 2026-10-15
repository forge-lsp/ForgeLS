use forgels_protocol::{Position, Range};
use forgels_syntax::Span;

/// Map a byte offset to an LSP position.
///
/// Character units are UTF-16 code units, the protocol default.
/// Out-of-range offsets clamp to the nearest char boundary at or before
/// the text end, so positions never point past content.
pub fn offset_to_position(text: &str, offset: u32) -> Position {
    let mut clamped = (offset as usize).min(text.len());
    while !text.is_char_boundary(clamped) {
        clamped -= 1;
    }
    let head = &text[..clamped];
    let line = head.bytes().filter(|&byte| byte == b'\n').count() as u32;
    let line_start = head.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let character = head[line_start..].encode_utf16().count() as u32;
    Position { line, character }
}

/// Map an LSP position back to a byte offset.
///
/// Character units are UTF-16 code units. Lines and characters clamp:
/// a line past the end lands at the text end, a character past the line
/// lands at the line end, and a mid-character stop lands at the boundary
/// start, so offsets never split a character.
pub fn position_to_offset(text: &str, position: Position) -> u32 {
    let mut line_start = 0;
    let mut line_index = 0;
    while line_index < position.line {
        match text[line_start..].find('\n') {
            Some(relative) => {
                line_start += relative + 1;
                line_index += 1;
            }
            None => return text.len() as u32,
        }
    }
    let line_end = text[line_start..]
        .find('\n')
        .map(|relative| line_start + relative)
        .unwrap_or(text.len());
    let line_text = &text[line_start..line_end];
    let mut units = 0u32;
    for (byte_index, ch) in line_text.char_indices() {
        if units == position.character {
            return (line_start + byte_index) as u32;
        }
        units += ch.len_utf16() as u32;
        if units > position.character {
            // Mid-character stop: fall back to this char start.
            return (line_start + byte_index) as u32;
        }
    }
    (line_start + line_text.len()) as u32
}

/// Map a span to an LSP range.
pub fn span_to_range(text: &str, span: Span) -> Range {
    Range {
        start: offset_to_position(text, span.start),
        end: offset_to_position(text, span.end),
    }
}
