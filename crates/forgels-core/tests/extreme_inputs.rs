use std::io::Cursor;
use std::path::PathBuf;

use forgels_core::diagnostics::{offset_to_position, parse_error_to_diagnostic, span_to_range};
use forgels_core::routing::Language;
use forgels_core::transport::{read_message, write_message};
use forgels_core::workspace::Workspace;
use forgels_protocol::Position;
use forgels_syntax::{ParseError, Span};

fn xorshift(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    value
}

fn mixed_text() -> String {
    "ab\n\u{E9}x {}\r\nfn \u{1F600}() {}\ncontract C {}\n".to_string()
}

#[test]
fn empty_document_tracks_empty_text() {
    let mut workspace = Workspace::new();
    let path = PathBuf::from("empty.sol");
    workspace
        .open(path.clone(), Language::Solidity, String::new(), 1)
        .expect("open works");
    assert_eq!(workspace.get(&path).expect("tracked").text, "");
}

#[test]
fn empty_text_positions_clamp_to_origin() {
    let origin = Position {
        line: 0,
        character: 0,
    };
    assert_eq!(offset_to_position("", 0), origin);
    assert_eq!(offset_to_position("", 99), origin);
    assert_eq!(offset_to_position("", u32::MAX), origin);
    let range = span_to_range("", Span::new(0, 99));
    assert_eq!(range.start, origin);
    assert_eq!(range.end, origin);
}

#[test]
fn empty_frame_roundtrips() {
    let mut output = Vec::new();
    write_message(&mut output, "").expect("write works");
    let mut input = Cursor::new(output);
    assert_eq!(
        read_message(&mut input).expect("read works").as_deref(),
        Some("")
    );
}

#[test]
fn huge_ascii_positions_stay_correct() {
    let line_count = 200_000;
    let text = "x\n".repeat(line_count);
    let end = offset_to_position(&text, text.len() as u32);
    assert_eq!(
        end,
        Position {
            line: line_count as u32,
            character: 0
        }
    );
    assert_eq!(
        offset_to_position(&text, 0),
        Position {
            line: 0,
            character: 0
        }
    );
}

#[test]
fn huge_body_frame_roundtrips() {
    let body = "y".repeat(1_000_000);
    let mut output = Vec::new();
    write_message(&mut output, &body).expect("write works");
    let mut input = Cursor::new(output);
    assert_eq!(
        read_message(&mut input).expect("read works").as_deref(),
        Some(body.as_str())
    );
}

#[test]
fn huge_document_syncs() {
    let text = "contract C {}\n".repeat(20_000);
    let mut workspace = Workspace::new();
    let path = PathBuf::from("big.sol");
    workspace
        .open(path.clone(), Language::Solidity, text.clone(), 1)
        .expect("open works");
    let changed = format!("{text}// tail");
    workspace
        .change(&path, changed.clone(), 2)
        .expect("change works");
    assert_eq!(workspace.get(&path).expect("tracked").text, changed);
    workspace.close(&path).expect("close works");
}

#[test]
fn random_spans_never_panic_and_stay_ordered() {
    let text = mixed_text();
    let byte_len = text.len() as u64;
    let mut state = 0x1234_5678_9abc_def1;
    for _ in 0..500 {
        let start = (xorshift(&mut state) % (byte_len + 50)) as u32;
        let end = (xorshift(&mut state) % (byte_len + 50)) as u32;
        let range = span_to_range(&text, Span::new(start, end));
        assert!((range.start.line, range.start.character) <= (range.end.line, range.end.character));
    }
    let full = span_to_range(&text, Span::new(0, text.len() as u32));
    assert_eq!(
        full.start,
        Position {
            line: 0,
            character: 0
        }
    );
    let expected_line = text.bytes().filter(|&byte| byte == b'\n').count() as u32;
    let tail = text.rsplit('\n').next().unwrap_or("");
    assert_eq!(
        full.end,
        Position {
            line: expected_line,
            character: tail.encode_utf16().count() as u32,
        }
    );
}

#[test]
fn random_bodies_roundtrip() {
    let mut state = 0xdead_beef_cafe_f00d;
    for _ in 0..100 {
        let len = (xorshift(&mut state) % 300) as usize;
        let body: String = (0..len)
            .map(|_| ((xorshift(&mut state) % 90) as u8 + 32) as char)
            .collect();
        let mut output = Vec::new();
        write_message(&mut output, &body).expect("write works");
        let mut input = Cursor::new(output);
        assert_eq!(
            read_message(&mut input).expect("read works").as_deref(),
            Some(body.as_str())
        );
    }
}

#[test]
fn garbage_spans_convert_without_panic() {
    let text = mixed_text();
    for span in [
        Span::new(0, 0),
        Span::new(5, 3),
        Span::new(u32::MAX - 10, u32::MAX),
        Span::new(3, u32::MAX),
    ] {
        let diagnostic = parse_error_to_diagnostic(&text, &ParseError::new(span, "broken"));
        assert_eq!(diagnostic.message, "broken");
    }
}

#[test]
fn span_clamp_handles_u32_max() {
    assert_eq!(Span::new(u32::MAX - 10, u32::MAX).clamp(5), Span::new(5, 5));
    assert_eq!(Span::new(0, u32::MAX).clamp(0), Span::new(0, 0));
}
