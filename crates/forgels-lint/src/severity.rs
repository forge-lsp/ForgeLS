use forgels_protocol::DiagnosticSeverity;

// Lint severity: Error blocks, Warning advises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    #[default]
    Error,
    Warning,
}

impl Severity {
    pub fn to_protocol(self) -> DiagnosticSeverity {
        match self {
            Self::Error => DiagnosticSeverity::Error,
            Self::Warning => DiagnosticSeverity::Warning,
        }
    }

    pub fn to_number(self) -> u8 {
        match self {
            Self::Error => 1,
            Self::Warning => 2,
        }
    }
}
