use super::{
    PublishGate, offset_to_position, parse_error_to_diagnostic, parse_errors_to_diagnostics,
    position_to_offset, span_to_range,
};
use crate::analyzer::DiagnosticData;
use forgels_protocol::{Diagnostic, DiagnosticSeverity, Position, Range};
use forgels_syntax::{ParseError, Span};
use std::time::{Duration, Instant};

#[test]
fn empty_text_maps_to_origin() {
    assert_eq!(
        offset_to_position("", 0),
        Position {
            line: 0,
            character: 0
        }
    );
    assert_eq!(
        offset_to_position("", 99),
        Position {
            line: 0,
            character: 0
        }
    );
}

#[test]
fn ascii_multiline_maps_lines() {
    let text = "ab\ncde";
    assert_eq!(
        offset_to_position(text, 0),
        Position {
            line: 0,
            character: 0
        }
    );
    assert_eq!(
        offset_to_position(text, 2),
        Position {
            line: 0,
            character: 2
        }
    );
    assert_eq!(
        offset_to_position(text, 3),
        Position {
            line: 1,
            character: 0
        }
    );
    assert_eq!(
        offset_to_position(text, 5),
        Position {
            line: 1,
            character: 2
        }
    );
    assert_eq!(
        offset_to_position(text, 99),
        Position {
            line: 1,
            character: 3
        }
    );
}

#[test]
fn two_byte_char_counts_one_unit() {
    // 'e with acute' is 2 bytes in UTF-8 but 1 UTF-16 unit.
    let text = "a\u{E9}x";
    assert_eq!(
        offset_to_position(text, 1),
        Position {
            line: 0,
            character: 1
        }
    );
    assert_eq!(
        offset_to_position(text, 3),
        Position {
            line: 0,
            character: 2
        }
    );
}

#[test]
fn mid_char_offset_clamps_down() {
    let text = "a\u{E9}x";
    assert_eq!(
        offset_to_position(text, 2),
        Position {
            line: 0,
            character: 1
        }
    );
}

#[test]
fn surrogate_pair_counts_two_units() {
    // Grinning face is 4 bytes in UTF-8 and 2 UTF-16 units.
    let text = "a\u{1F600}b";
    assert_eq!(
        offset_to_position(text, 5),
        Position {
            line: 0,
            character: 3
        }
    );
    assert_eq!(
        offset_to_position(text, 6),
        Position {
            line: 0,
            character: 4
        }
    );
}

#[test]
fn unicode_on_second_line_maps() {
    let text = "ab\n\u{E9}x";
    assert_eq!(
        offset_to_position(text, 5),
        Position {
            line: 1,
            character: 1
        }
    );
}

#[test]
fn span_maps_to_range() {
    let text = "ab\ncde";
    let range = span_to_range(text, Span::new(3, 5));
    assert_eq!(
        range,
        Range {
            start: Position {
                line: 1,
                character: 0
            },
            end: Position {
                line: 1,
                character: 2
            },
        }
    );
}

#[test]
fn position_maps_to_byte_offset() {
    let text = "ab\ncde";
    let at = |line: u32, character: u32| position_to_offset(text, Position { line, character });
    assert_eq!(at(0, 0), 0);
    assert_eq!(at(0, 2), 2);
    assert_eq!(at(1, 0), 3);
    assert_eq!(at(1, 2), 5);
}

#[test]
fn position_clamps_past_the_end() {
    let text = "ab\ncde";
    let at = |line: u32, character: u32| position_to_offset(text, Position { line, character });
    assert_eq!(at(0, 99), 2);
    assert_eq!(at(9, 0), text.len() as u32);
}

#[test]
fn position_counts_utf16_units() {
    let text = "a\u{E9}x";
    let at = |character: u32| position_to_offset(text, Position { line: 0, character });
    assert_eq!(at(0), 0);
    assert_eq!(at(1), 1);
    assert_eq!(at(2), 3);
    assert_eq!(at(3), 4);
}

#[test]
fn position_mid_character_falls_back() {
    let text = "a\u{1F600}b";
    let at = |character: u32| position_to_offset(text, Position { line: 0, character });
    assert_eq!(at(1), 1);
    assert_eq!(at(2), 1);
    assert_eq!(at(3), 5);
}

#[test]
fn offset_roundtrips_through_position() {
    let text = "ab\n\u{E9}x \u{1F600}\nend";
    for offset in [0, 1, 2, 3, 5, 6, 7, 11, 12, 13, 14, 15] {
        let back = position_to_offset(text, offset_to_position(text, offset));
        assert_eq!(back, offset, "offset {offset}");
    }
}

#[test]
fn parse_error_becomes_error_diagnostic() {
    let text = "contract A {\nnope";
    let error = ParseError::new(Span::new(13, 17), "expected type name");
    let diagnostic = parse_error_to_diagnostic(text, &error);
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::Error));
    assert_eq!(diagnostic.message, "expected type name");
    assert_eq!(
        diagnostic.range,
        Range {
            start: Position {
                line: 1,
                character: 0
            },
            end: Position {
                line: 1,
                character: 4
            },
        }
    );
}

#[test]
fn unicode_error_points_at_right_column() {
    let text = "a\u{E9}x oops";
    let error = ParseError::new(Span::new(4, 8), "expected semicolon");
    let diagnostic = parse_error_to_diagnostic(text, &error);
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::Error));
    assert_eq!(
        diagnostic.range.start,
        Position {
            line: 0,
            character: 3
        }
    );
}

#[test]
fn gate_holds_until_quiet_period_passes() {
    let mut gate = PublishGate::new(Duration::from_millis(200));
    let start = Instant::now();
    gate.mark_dirty("file:///a.sol", 1, start);
    assert!(gate.take_due(start).is_empty());
    assert_eq!(
        gate.take_due(start + Duration::from_millis(200)),
        vec![("file:///a.sol".to_string(), 1)]
    );
    assert!(gate.take_due(start + Duration::from_millis(500)).is_empty());
}

#[test]
fn rapid_typing_coalesces_to_latest_version() {
    let mut gate = PublishGate::new(Duration::from_millis(200));
    let start = Instant::now();
    for version in 1..=5 {
        gate.mark_dirty(
            "file:///a.sol",
            version,
            start + Duration::from_millis(50 * version),
        );
    }
    let due = gate.take_due(start + Duration::from_millis(450));
    assert_eq!(due, vec![("file:///a.sol".to_string(), 5)]);
}

#[test]
fn gate_tracks_each_uri_independently() {
    let mut gate = PublishGate::new(Duration::from_millis(200));
    let start = Instant::now();
    gate.mark_dirty("file:///b.sol", 1, start);
    gate.mark_dirty("file:///a.sol", 7, start);
    let due = gate.take_due(start + Duration::from_millis(200));
    assert_eq!(
        due,
        vec![
            ("file:///a.sol".to_string(), 7),
            ("file:///b.sol".to_string(), 1),
        ]
    );
}

#[test]
fn two_errors_become_two_diagnostics() {
    let text = "contract A {\nnope\nalsono";
    let errors = vec![
        ParseError::new(Span::new(13, 17), "expected type name"),
        ParseError::new(Span::new(18, 24), "expected semicolon"),
    ];
    let diagnostics = parse_errors_to_diagnostics(text, &errors);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].message, "expected type name");
    assert_eq!(diagnostics[1].message, "expected semicolon");
    assert_eq!(
        diagnostics[0].range.start,
        Position {
            line: 1,
            character: 0
        }
    );
    assert_eq!(
        diagnostics[1].range.start,
        Position {
            line: 2,
            character: 0
        }
    );
    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::Error));
    }
}

#[test]
fn no_errors_means_no_diagnostics() {
    let diagnostics = parse_errors_to_diagnostics("contract A {}", &[]);
    assert!(diagnostics.is_empty());
}

#[test]
fn warning_constructors_carry_warning_severity() {
    let range = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 0,
            character: 1,
        },
    };
    let diagnostic = Diagnostic::warning(range, "caution");
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::Warning));
    let item = DiagnosticData::warning(Span::new(0, 1), "caution".to_string());
    assert_eq!(item.severity, crate::Severity::Warning);
    assert_eq!(item.severity.to_protocol(), DiagnosticSeverity::Warning);
    assert_eq!(item.severity.to_number(), 2);
}
