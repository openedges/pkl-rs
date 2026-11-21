//! Native CLI execution engine mirroring `org.pkl.cli.CliFormatterCommand`.

use crate::{Formatter, GrammarVersion};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_VIOLATION: i32 = 11;

#[derive(Clone, Debug, Default)]
pub struct FormatCliOptions {
    pub paths: Vec<PathBuf>,
    pub grammar_version: GrammarVersion,
    pub write: bool,
    pub diff_name_only: bool,
    pub silent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Status(i32);

impl Status {
    fn update(&mut self, new_status: i32) {
        self.0 = if self.0 == EXIT_ERROR || new_status == EXIT_ERROR {
            EXIT_ERROR
        } else {
            self.0.max(new_status)
        };
    }
}

pub fn run_cli(
    opts: &FormatCliOptions,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let mut status = Status(EXIT_SUCCESS);
    let formatter = Formatter::new(opts.grammar_version);

    let all_paths = match collect_all_paths(&opts.paths) {
        Ok(paths) => paths,
        Err((path, err)) => {
            let path_str = path.to_string_lossy();
            let msg = io_error_message(&path_str, &err);
            write_err_line(
                stderr,
                &format!("IO error while reading `{path_str}`: {msg}"),
            );
            if !opts.silent {
                write_err_line(stderr, "An error occurred during formatting.");
            }
            return EXIT_ERROR;
        }
    };

    for path in all_paths {
        let path_str = path.to_string_lossy().into_owned();
        let contents = if path_str == "-" {
            let mut buf = String::new();
            match stdin.read_to_string(&mut buf) {
                Ok(_) => buf,
                Err(err) => {
                    let msg = io_error_message(&path_str, &err);
                    write_err_line(
                        stderr,
                        &format!("IO error while reading `{path_str}`: {msg}"),
                    );
                    status.update(EXIT_ERROR);
                    continue;
                }
            }
        } else {
            match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(err) => {
                    let msg = io_error_message(&path_str, &err);
                    write_err_line(
                        stderr,
                        &format!("IO error while reading `{path_str}`: {msg}"),
                    );
                    status.update(EXIT_ERROR);
                    continue;
                }
            }
        };

        if path_str == "-" && opts.write {
            write_err_line(stderr, "Cannot write to stdin");
            return EXIT_ERROR;
        }

        let formatted = match formatter.format(&contents) {
            Ok(formatted) => formatted,
            Err(err) => {
                write_err_line(stderr, &format!("Could not format `{path_str}`: {err}"));
                status.update(EXIT_ERROR);
                continue;
            }
        };

        if contents != formatted {
            if (opts.diff_name_only || opts.write) && !opts.silent {
                write_line(stdout, &path_str);
            }

            if opts.write {
                if let Err(err) = fs::write(&path, formatted.as_bytes()) {
                    let msg = io_error_message(&path_str, &err);
                    write_err_line(
                        stderr,
                        &format!("IO error while reading `{path_str}`: {msg}"),
                    );
                    status.update(EXIT_ERROR);
                    continue;
                }
            } else {
                status.update(EXIT_VIOLATION);
            }
        }

        if !opts.diff_name_only && !opts.write {
            let _ = stdout.write_all(formatted.as_bytes());
            let _ = stdout.flush();
        }
    }

    match status.0 {
        EXIT_VIOLATION => EXIT_VIOLATION,
        EXIT_ERROR => {
            if !opts.silent {
                write_err_line(stderr, "An error occurred during formatting.");
            }
            EXIT_ERROR
        }
        other => other,
    }
}

fn write_line(out: &mut dyn Write, message: &str) {
    let _ = writeln!(out, "{message}");
    let _ = out.flush();
}

fn write_err_line(err: &mut dyn Write, message: &str) {
    let _ = writeln!(err, "{message}");
    let _ = err.flush();
}

fn io_error_message(path_str: &str, err: &io::Error) -> String {
    match err.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => path_str.to_string(),
        _ => err.to_string(),
    }
}

fn collect_all_paths(input_paths: &[PathBuf]) -> Result<Vec<PathBuf>, (PathBuf, io::Error)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in input_paths {
        if !seen.insert(path.clone()) {
            continue;
        }
        if path.as_os_str() == OsStr::new("-") {
            out.push(path.clone());
        } else if path.is_dir() {
            walk_dir(path, &mut out)?;
        } else {
            out.push(normalize_path(path));
        }
    }
    Ok(out)
}

/// Depth-first recursive walk mirroring `java.nio.file.Files.walk(path)` (does NOT follow nested
/// symlinks) filtered by `extension == "pkl" || name == "PklProject"`.
fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), (PathBuf, io::Error)> {
    if matches_pkl_file(dir) {
        out.push(normalize_path(dir));
    }
    let read_dir = fs::read_dir(dir).map_err(|err| (dir.to_path_buf(), err))?;
    for entry_res in read_dir {
        let entry = entry_res.map_err(|err| (dir.to_path_buf(), err))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| (path.clone(), err))?;
        if file_type.is_dir() {
            walk_dir(&path, out)?;
        } else if matches_pkl_file(&path) {
            out.push(normalize_path(&path));
        }
    }
    Ok(())
}

/// Matches Kotlin `path.extension == "pkl" || path.name == "PklProject"`.
fn matches_pkl_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    if name == "PklProject" {
        return true;
    }
    matches!(name.rsplit_once('.'), Some((_, "pkl")))
}

/// Lexical normalization matching `java.nio.file.Path.normalize()`.
fn normalize_path(path: &Path) -> PathBuf {
    let mut prefix_or_root = Vec::new();
    let mut normal_stack: Vec<Component<'_>> = Vec::new();
    let mut has_root = false;

    for comp in path.components() {
        match comp {
            Component::Prefix(_) => prefix_or_root.push(comp),
            Component::RootDir => {
                has_root = true;
                prefix_or_root.push(comp);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(normal_stack.last(), Some(Component::Normal(_))) {
                    normal_stack.pop();
                } else if !has_root {
                    normal_stack.push(comp);
                }
            }
            Component::Normal(_) => normal_stack.push(comp),
        }
    }

    let mut result = PathBuf::new();
    for comp in prefix_or_root {
        result.push(comp.as_os_str());
    }
    for comp in normal_stack {
        result.push(comp.as_os_str());
    }
    result
}
