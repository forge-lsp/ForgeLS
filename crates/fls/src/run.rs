use std::path::{Path, PathBuf};

use forgels_lint::{FileReport, Severity, report, report_json, should_lint};

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
pub fn run(args: &CheckArgs, cwd: &Path, config: &mut LoadedConfig) -> i32 {
    let (files, truncated) = match collect_files(&args.paths, cwd, &config.lint.excludes) {
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
    let (files, filter_warnings) = filter_files(cwd, files, config);
    for warning in &filter_warnings {
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
        let mut diagnostics = diagnostics_for_file(
            path,
            text,
            &config.lint,
            &config.root,
            &config.remappings,
            &mut config.external,
        );
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
        let text = format!("{}\n", report_json(&reports));
        if write_report(&text).is_err() {
            eprintln!("fls: cannot write output");
            return 2;
        }
    } else if write_report(&report(&reports)).is_err() {
        eprintln!("fls: cannot write output");
        return 2;
    }
    if has_error { 1 } else { 0 }
}

/// Write report text to stdout.
///
/// Broken pipe stays silent so `check | head` keeps the computed exit
/// code. Other IO failures surface as an operational error.
fn write_report(text: &str) -> Result<(), String> {
    write_to(&mut std::io::stdout(), text)
}

pub(crate) fn write_to(writer: &mut dyn std::io::Write, text: &str) -> Result<(), String> {
    match writer.write_all(text.as_bytes()) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(_) => Err(String::from("fls: cannot write output")),
    }
}

pub(crate) fn path_display(cwd: &Path, path: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(cwd)
        && !rel.as_os_str().is_empty()
    {
        return rel.to_string_lossy().into_owned();
    }
    path.to_string_lossy().into_owned()
}

fn relative_posix(cwd: &Path, path: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(cwd)
        && !rel.as_os_str().is_empty()
    {
        return rel.to_string_lossy().replace('\\', "/");
    }
    path.to_string_lossy().replace('\\', "/")
}

// Keep walker output but drop files outside lint includes/excludes.
// Invalid patterns warn once and are ignored; caps and bridge stay.
fn filter_files(
    cwd: &Path,
    files: Vec<PathBuf>,
    config: &LoadedConfig,
) -> (Vec<PathBuf>, Vec<String>) {
    let mut kept = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for path in files {
        let rel = relative_posix(cwd, &path);
        let (keep, mut warns) = should_lint(&rel, &config.lint.includes, &config.lint.excludes);
        for warn in warns.drain(..) {
            if !warnings.contains(&warn) {
                warnings.push(warn);
            }
        }
        if keep {
            kept.push(path);
        }
    }
    (kept, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailWriter {
        kind: std::io::ErrorKind,
    }

    impl std::io::Write for FailWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(self.kind, "fail"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn broken_pipe_stays_silent() {
        let mut writer = FailWriter {
            kind: std::io::ErrorKind::BrokenPipe,
        };
        assert!(write_to(&mut writer, "out").is_ok());
    }

    #[test]
    fn other_write_errors_surface() {
        let mut writer = FailWriter {
            kind: std::io::ErrorKind::Other,
        };
        assert_eq!(
            write_to(&mut writer, "out"),
            Err(String::from("fls: cannot write output"))
        );
    }
}
