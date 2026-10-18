//! forgels-common: shared utilities any crate may use.

pub mod fs;
pub mod http;
pub mod log;
pub mod paths;

pub use fs::{
    DEFAULT_SKIP_DIRS, WalkCaps, WalkOutcome, exists, is_skipped_dir, is_trackable_file,
    list_dir_entries, list_dir_entries_matching, list_files_with_extension, read_to_string,
    write_string,
};
pub use paths::{
    MAX_NODE_MODULES_DEPTH, Remapping, node_module_candidates, node_modules_dirs, normalize,
    parse_remapping, relative_path, resolve_import, resolve_import_full, resolve_root,
    resolve_with_remappings,
};
