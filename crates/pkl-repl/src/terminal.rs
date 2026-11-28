//! Terminal and line-reading abstraction for `pkl repl`.
//!
//! Pure safe Rust over `std::io::{BufRead, Write, IsTerminal}` with zero extra crates:
//! - Preserves canonical TTY/PTY behavior (including `\r\n` and kernel `^D` echo under `TERM=dumb`).
//! - Suppresses ANSI prompt fainting and `:clear` escapes on pipes and `TERM=dumb` terminals,
//!   matching JLine 3's `DumbTerminal` behavior in Pkl 0.32.0.
//! - Persists `~/.pkl/repl-history` and removes legacy `~/.pkl/repl-history.bin` when configured.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// Minimal line-editor interface for driving the REPL loop over stdio or in-memory streams.
pub trait LineEditor {
    /// Writes `prompt`, flushes output, and reads the next line (`Ok(None)` on EOF).
    fn read_line(&mut self, prompt: &str) -> io::Result<Option<String>>;
    /// Writes raw text to the terminal output stream.
    fn write_out(&mut self, text: &str) -> io::Result<()>;
    /// Flushes the terminal output stream.
    fn flush_out(&mut self) -> io::Result<()>;
    /// Clears the terminal screen when attached to a color-capable TTY (no-op on pipe/`TERM=dumb`).
    fn clear_screen(&mut self) -> io::Result<()>;
    /// Whether prompts should use ANSI faint styling (`color && is_tty && TERM != "dumb"`).
    fn supports_faint_prompt(&self, color: bool) -> bool;
    /// Persists session history if a history file is configured.
    fn save_history(&mut self);
}

/// Zero-dependency `BufRead`/`Write` line editor matching Pkl 0.32.0 pipe and `TERM=dumb` PTY I/O.
pub struct BufReadLineEditor<R: BufRead, W: Write> {
    input: R,
    output: W,
    is_tty: bool,
    dumb_term: bool,
    history: Vec<String>,
    history_path: Option<PathBuf>,
}

const MAX_HISTORY_ENTRIES: usize = 10_000;
const MAX_HISTORY_BYTES: u64 = 4 * 1024 * 1024;

fn path_is_symlink_or_junction(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|meta| pkl_host_native::is_symlink_or_junction(path, &meta))
}

impl<R: BufRead, W: Write> BufReadLineEditor<R, W> {
    pub fn new(input: R, output: W, is_tty: bool, dumb_term: bool) -> Self {
        Self {
            input,
            output,
            is_tty,
            dumb_term,
            history: Vec::new(),
            history_path: None,
        }
    }

    /// Configures persistent history at `path` (e.g. `~/.pkl/repl-history`) and removes
    /// any stale JLine 2 `repl-history.bin` sibling (`Repl.kt:83`).
    pub fn with_history_file(mut self, path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let legacy = parent.join("repl-history.bin");
            if !path_is_symlink_or_junction(&legacy) {
                let _ = fs::remove_file(legacy);
            }
        }
        if fs::symlink_metadata(&path).is_ok_and(|m| m.len() <= MAX_HISTORY_BYTES)
            && let Ok(bytes) = pkl_host_native::read_path_nofollow_bounded(&path)
            && let Ok(existing) = String::from_utf8(bytes)
        {
            for line in existing.lines() {
                if !line.is_empty() {
                    self.history.push(line.to_string());
                }
            }
            if self.history.len() > MAX_HISTORY_ENTRIES {
                let excess = self.history.len() - MAX_HISTORY_ENTRIES;
                self.history.drain(..excess);
            }
        }
        self.history_path = Some(path);
        self
    }

    /// Returns recorded history lines for this session.
    pub fn history(&self) -> &[String] {
        &self.history
    }
}

impl<R: BufRead, W: Write> LineEditor for BufReadLineEditor<R, W> {
    fn read_line(&mut self, prompt: &str) -> io::Result<Option<String>> {
        self.output.write_all(prompt.as_bytes())?;
        self.output.flush()?;
        let mut line = String::new();
        let n = self.input.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        while line.ends_with(['\n', '\r']) {
            line.pop();
        }
        if !line.trim().is_empty() {
            self.history.push(line.clone());
            if self.history.len() > MAX_HISTORY_ENTRIES {
                let excess = self.history.len() - MAX_HISTORY_ENTRIES;
                self.history.drain(..excess);
            }
        }
        Ok(Some(line))
    }

    fn write_out(&mut self, text: &str) -> io::Result<()> {
        self.output.write_all(text.as_bytes())
    }

    fn flush_out(&mut self) -> io::Result<()> {
        self.output.flush()
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        if self.is_tty && !self.dumb_term {
            self.output.write_all(b"\x1b[H\x1b[2J")?;
            self.output.flush()?;
        }
        Ok(())
    }

    fn supports_faint_prompt(&self, color: bool) -> bool {
        color && self.is_tty && !self.dumb_term
    }

    fn save_history(&mut self) {
        let Some(ref path) = self.history_path else {
            return;
        };
        if path_is_symlink_or_junction(path) {
            return;
        }
        if let Some(parent) = path.parent() {
            if path_is_symlink_or_junction(parent) {
                return;
            }
            let _ = fs::create_dir_all(parent);
        }
        let start = self.history.len().saturating_sub(MAX_HISTORY_ENTRIES);
        let mut buf = String::new();
        for entry in &self.history[start..] {
            buf.push_str(entry);
            buf.push('\n');
        }
        let _ = pkl_host_native::write_file_nofollow(path, buf.as_bytes());
    }
}

/// Resolves the default Pkl home directory (`$PKL_HOME` or `$HOME/.pkl`) matching `IoUtils.getPklHomeDir`.
pub fn default_pkl_home_dir() -> Option<PathBuf> {
    if let Some(pkl_home) = std::env::var_os("PKL_HOME")
        && !pkl_home.is_empty()
    {
        return Some(PathBuf::from(pkl_home));
    }
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|h| !h.is_empty()))
        .map(|home| PathBuf::from(home).join(".pkl"))
}

/// Returns `true` when `TERM` is `"dumb"` (or unset/empty on Unix).
pub fn is_dumb_terminal() -> bool {
    match std::env::var("TERM") {
        Ok(val) => val.eq_ignore_ascii_case("dumb") || val.trim().is_empty(),
        Err(_) => !cfg!(windows),
    }
}

/// Completes filesystem path entries relative to `working_dir` for `:load <partial>`
/// (mirrors `FileCompleter` in `ReplCompleters.kt`).
pub fn complete_file_paths(working_dir: &Path, partial: &str) -> Vec<String> {
    let split_idx = if cfg!(windows) {
        partial.rfind(['/', '\\'])
    } else {
        partial.rfind('/')
    };
    let (dir_prefix, file_prefix, sep) = match split_idx {
        Some(idx) => {
            let sep = partial.as_bytes()[idx] as char;
            (&partial[..=idx], &partial[idx + 1..], sep)
        }
        None => ("", partial, '/'),
    };
    let search_dir = if dir_prefix.is_empty() {
        working_dir.to_path_buf()
    } else {
        working_dir.join(dir_prefix)
    };
    let Ok(entries) = fs::read_dir(&search_dir) else {
        return Vec::new();
    };
    let mut results = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(file_prefix) {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let mut candidate = format!("{dir_prefix}{name}");
        if is_dir {
            candidate.push(sep);
        }
        results.push(candidate);
    }
    results.sort();
    results
}
