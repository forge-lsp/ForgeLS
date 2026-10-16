//! forgels-common: shared utilities any crate may use.

pub mod fs;
pub mod http;
pub mod log;
pub mod paths;

pub use fs::{read_to_string, write_string};
pub use paths::{normalize, resolve_import, resolve_root};
