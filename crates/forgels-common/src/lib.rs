//! forgels-common: shared utilities any crate may use.

pub mod fs;
pub mod http;
pub mod log;
pub mod paths;

pub use fs::{WalkCaps, exists, list_files_with_extension, read_to_string, write_string};
pub use paths::{
    Remapping, normalize, parse_remapping, relative_path, resolve_import, resolve_root,
    resolve_with_remappings,
};
