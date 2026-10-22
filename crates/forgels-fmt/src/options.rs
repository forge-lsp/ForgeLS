//! Format options: line length, indent width, indent style.
//!
//! Defaults are 120 columns, 4-wide indent, space style. Tab style
//! emits one `\t` per level; width only counts tab display size.

// Space indent repeats `indent_width`; tab emits one tab per level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndentStyle {
    #[default]
    Space,
    Tab,
}

impl IndentStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Space => "space",
            Self::Tab => "tab",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        if raw.is_empty() {
            return None;
        }
        match raw {
            "space" => Some(Self::Space),
            "tab" => Some(Self::Tab),
            _ => None,
        }
    }
}

// Plain formatter knobs. No file IO, no language logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    pub line_length: usize,
    pub indent_width: usize,
    pub style: IndentStyle,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            line_length: 120,
            indent_width: 4,
            style: IndentStyle::Space,
        }
    }
}

impl FormatOptions {
    pub fn new(line_length: usize, indent_width: usize, style: IndentStyle) -> Self {
        if line_length == 0 {
            return Self {
                line_length: 120,
                indent_width,
                style,
            };
        }
        if indent_width == 0 {
            return Self {
                line_length,
                indent_width: 4,
                style,
            };
        }
        Self {
            line_length,
            indent_width,
            style,
        }
    }

    // One indent level: spaces repeat width, tab is one tab.
    pub fn indent_unit(&self) -> String {
        match self.style {
            IndentStyle::Tab => "\t".to_string(),
            IndentStyle::Space => {
                if self.indent_width == 0 {
                    return String::new();
                }
                " ".repeat(self.indent_width)
            }
        }
    }

    // Display width: tab counts as `indent_width`, else one per char.
    // CRLF `\r` never counts; caller strips it before calling.
    pub fn line_width(&self, line: &str) -> usize {
        if line.is_empty() {
            return 0;
        }
        let tab_w = if self.indent_width == 0 {
            1
        } else {
            self.indent_width
        };
        let mut out = 0usize;
        for ch in line.chars() {
            if ch == '\t' {
                out += tab_w;
            } else {
                out += 1;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_120_4_space() {
        let opts = FormatOptions::default();
        assert_eq!(opts.line_length, 120);
        assert_eq!(opts.indent_width, 4);
        assert_eq!(opts.style, IndentStyle::Space);
    }

    #[test]
    fn space_unit_repeats_width() {
        let opts = FormatOptions::default();
        assert_eq!(opts.indent_unit(), "    ");
    }

    #[test]
    fn tab_unit_is_single_tab() {
        let opts = FormatOptions {
            style: IndentStyle::Tab,
            ..Default::default()
        };
        assert_eq!(opts.indent_unit(), "\t");
    }

    #[test]
    fn style_round_trips() {
        assert_eq!(IndentStyle::parse("space"), Some(IndentStyle::Space));
        assert_eq!(IndentStyle::parse("tab"), Some(IndentStyle::Tab));
        assert_eq!(IndentStyle::parse("two"), None);
        assert_eq!(IndentStyle::Space.as_str(), "space");
    }

    #[test]
    fn tab_width_counts_for_line_width() {
        let opts = FormatOptions {
            style: IndentStyle::Tab,
            indent_width: 4,
            ..Default::default()
        };
        assert_eq!(opts.line_width("\ta"), 5);
        assert_eq!(opts.line_width("ab"), 2);
    }
}
