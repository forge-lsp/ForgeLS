use std::path::{Path, PathBuf};

use forgels_lint::{FileReport, Severity, report, report_json};

use super::bridge::diagnostics_for_file;
use super::config::LoadedConfig;
use super::fix::{apply_and_write, attach_fixes, ephemeral_workspace};
use super::walk::collect_files;
use crate::cli::CheckArgs;

/// Run check and return the process exit code.
///
/// Reads each file, builds diagnostics, applies safe fixes with
/// `--write`, then prints the human or JSON report to stdout.
/// Config warnings go to stderr. Exit is 0 for clean or warn-only,
/// 1 for any error, 2 for operational failures.
pub fn run(args: &CheckArgs, cwd: &Path, config: &LoadedConfig) -> i32 {
    let files = match collect_files(&args.paths, cwd) {
        Ok(files) => files,
        Err(message) => {
            eprintln!("{message}");
            return 2;
        }
    };
    for warning in &config.warnings {
        eprintln!("{warning}");
    }
    // Explicit paths must exist as *.sol. The walker already rejects
    // unreadable entries, but a vanished file still fails here.
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
    let workspace = ephemeral_workspace(&texts);
    let mut reports: Vec<FileReport> = Vec::new();
    let mut has_error = false;
    for (path, text) in &texts {
        let mut diagnostics =
            diagnostics_for_file(path, text, &config.lint, &config.root, &config.remappings);
        if args.write {
            attach_fixes(path, text, &mut diagnostics, &workspace);
            match apply_and_write(path, text, &diagnostics) {
                Ok((new_text, fixed)) => {
                    for diag in &diagnostics {
                        if diag.severity == Severity::Error {
                            has_error = true;
                        }
                    }
                    // Report against the pre-write text so ranges match
                    // the file the user saw. Fixed flags mark applied.
                    let display = path_display(cwd, path);
                    reports.push(FileReport::with_fixed(
                        display,
                        text.clone(),
                        diagnostics,
                        fixed,
                    ));
                    let _ = new_text;
                    continue;
                }
                Err(message) => {
                    eprintln!("{message}");
                    return 2;
                }
            }
        }
        for diag in &diagnostics {
            if diag.severity == Severity::Error {
                has_error = true;
            }
        }
        let display = path_display(cwd, path);
        reports.push(FileReport::new(display, text.clone(), diagnostics));
    }
    if args.json {
        println!("{}", report_json(&reports));
    } else {
        print!("{}", report(&reports));
    }
    if has_error { 1 } else { 0 }
}

fn path_display(cwd: &Path, path: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(cwd)
        && !rel.as_os_str().is_empty()
    {
        return rel.to_string_lossy().into_owned();
    }
    path.to_string_lossy().into_owned()
}
