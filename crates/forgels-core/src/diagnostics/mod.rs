use std::time::Duration;

mod gate;
mod mapping;
mod positions;

#[cfg(test)]
mod tests;

pub use gate::PublishGate;
pub use mapping::{
    parse_error_to_diagnostic, parse_errors_to_diagnostics, unresolved_import_to_diagnostic,
};
pub use positions::{offset_to_position, position_to_offset, span_to_range};

/// Quiet period after the last keystroke before a publish fires.
pub const DEFAULT_PUBLISH_DELAY: Duration = Duration::from_millis(200);
