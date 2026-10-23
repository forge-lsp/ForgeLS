use std::path::{Path, PathBuf};

use forgels_fmt::{FormatOptions, format_text};

use super::config::{LoadedConfig, format_options_from};
use super::external::{ExternalHosts, format_with_guests};
use super::run::{path_display, write_to};
use super::walk::collect_files;
use crate::cli::FormatArgs;

// Format order: builtin to convergence, one guest pass, builtin
// again. Each builtin phase keeps its cap-4 fixpoint. Guests apply
// via forgels_fmt::apply. Empty hosts skip the guest pass.
fn format_with_external(
    path: &Path,
    text: &str,
    options: &FormatOptions,
    external: &mut ExternalHosts,
) -> String {
    let first = format_text(text, options).text;
    if !external.has_guests() {
        return first;
    }
    let edits = format_with_guests(external, path, &first, options);
    if edits.is_empty() {
        return first;
    }
    let after_guest = forgels_fmt::apply(&first, &edits).text;
    format_text(&after_guest, options).text
}

// Run format and return the process exit code.
//
// Default prints formatted text to stdout without writing. `--write`
// writes changed files in place. `--check` lists files that would
// change without writing. Exit is 0 for clean, 1 for check
// would-change, 2 for operational failures.
pub fn run(args: &FormatArgs, cwd: &Path, config: &mut LoadedConfig) -> i32 {
    if args.write && args.check {
        eprintln!("fls: --write and --check are mutually exclusive");
        return 2;
    }
    let (files, truncated) = match collect_files(&args.paths, cwd, &config.format.excludes) {
        Ok(found) => found,
        Err(message) => {
            eprintln!("{message}");
            return 2;
        }
    };
    if truncated {
        eprintln!("fls: walk truncated, results may be incomplete");
    }
    for warning in &config.warnings {
        eprintln!("{warning}");
    }
    let options = format_options_from(&config.format);
    for path in &args.paths {
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("sol") {
            continue;
        }
        if path.is_dir() {
            continue;
        }
        if !files.contains(path) {
            eprintln!("fls: cannot read {}", path.display());
            return 2;
        }
    }
    let mut texts: Vec<(PathBuf, String)> = Vec::new();
    for path in &files {
        match std::fs::read_to_string(path) {
            Ok(text) => texts.push((path.clone(), text)),
            Err(_) => {
                eprintln!("fls: cannot read {}", path.display());
                return 2;
            }
        }
    }
    if args.check {
        let mut changed: Vec<String> = Vec::new();
        for (path, text) in &texts {
            if format_with_external(path, text, &options, &mut config.external) != *text {
                changed.push(path_display(cwd, path));
            }
        }
        if changed.is_empty() {
            return 0;
        }
        let mut out = String::new();
        for name in &changed {
            out.push_str(name);
            out.push('\n');
        }
        if write_to(&mut std::io::stdout(), &out).is_err() {
            eprintln!("fls: cannot write output");
            return 2;
        }
        return 1;
    }
    if args.write {
        for (path, text) in &texts {
            let formatted = format_with_external(path, text, &options, &mut config.external);
            if formatted == *text {
                continue;
            }
            if let Err(err) = forgels_common::fs::write_string(path, &formatted) {
                eprintln!("fls: cannot write {}: {err}", path.display());
                return 2;
            }
        }
        return 0;
    }
    let mut out = String::new();
    for (path, text) in &texts {
        out.push_str(&format_with_external(
            path,
            text,
            &options,
            &mut config.external,
        ));
    }
    if write_to(&mut std::io::stdout(), &out).is_err() {
        eprintln!("fls: cannot write output");
        return 2;
    }
    0
}
