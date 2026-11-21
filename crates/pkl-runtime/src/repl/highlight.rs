//! REPL syntax highlighting and backtick bolding matching Pkl 0.32.0 `Repl.kt`
//! and `SyntaxHighlighter.java`.

use crate::diagnostic::ansi::{AnsiCode, AnsiCodeSet, AnsiStringBuilder};
use crate::value::PklString;

/// Applies Pkl 0.32.0 `SyntaxHighlighter` ANSI styling to Pkl source / rendered value text.
pub fn syntax_highlight(source: &str, use_color: bool) -> String {
    let mut sb = AnsiStringBuilder::new(use_color);
    let pkl = PklString::from(source);
    crate::diagnostic::highlight::write_to(&mut sb, &pkl);
    sb.finish_pkl().to_java_utf8()
}

/// Formats the `pkl{n}> ` prompt, applying `FAINT` (`\x1b[2m...\x1b[0m`) only when
/// `faint_prompt` is `true` (matching JLine's `AttributedString.toAnsi(terminal)` behavior:
/// colored only when `color == true` AND the terminal is color-capable, not dumb/piped).
pub fn format_prompt(request_id: usize, continuation: bool, faint_prompt: bool) -> String {
    let text = if continuation {
        " ".repeat(format!("pkl{request_id}> ").len())
    } else {
        format!("pkl{request_id}> ")
    };
    if faint_prompt {
        let mut sb = AnsiStringBuilder::new(true);
        sb.append_styled(AnsiCodeSet::of(AnsiCode::Faint), &text);
        sb.finish_pkl().to_java_utf8()
    } else {
        text
    }
}

/// Replicates `Repl.kt:250-266`'s unconditional backtick-splitting bold highlighter.
///
/// Upstream Kotlin uses `str.split("`", "```")`, where the shorter delimiter `` ` `` is
/// checked BEFORE `` ``` `` at every index. Consequently:
/// - Single backticks `` `foo` `` wrap `foo` in `\x1b[1mfoo\x1b[22m`.
/// - Triple backticks `` ```code``` `` split into `["", "", "", "code", "", "", ""]`,
///   producing `\x1b[1m\x1b[22m\x1b[1mcode\x1b[22m\x1b[1m\x1b[22m` (matching live Pkl 0.32.0
///   byte-for-byte, even when `--color never` or piped!).
pub fn highlight_backticks(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    let mut bold = false;
    for part in input.split('`') {
        if bold {
            out.push_str("\x1b[1m");
            out.push_str(part);
            out.push_str("\x1b[22m");
        } else {
            out.push_str(part);
        }
        bold = !bold;
    }
    out
}

/// Strips the synthetic `` `THE REPL TEXT EXPR` =  `` expression preamble and its matching
/// caret-line indentation from a rendered Pkl diagnostic (`ReplServer.java:294-310`).
pub fn strip_expression_preamble(rendered: &str) -> String {
    const PREAMBLE: &str = crate::vm::repl::REPL_EXPR_PREAMBLE;
    let spaces = " ".repeat(PREAMBLE.len());
    // Java `rendered.split("\n")` drops trailing empty strings, then appends `\n` to each line.
    let mut lines: Vec<&str> = rendered.split('\n').collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    let mut out = String::with_capacity(rendered.len());
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.contains(PREAMBLE) {
            out.push_str(&line.replace(PREAMBLE, ""));
            out.push('\n');
            if i + 1 < lines.len() {
                let decoration = lines[i + 1];
                out.push_str(&decoration.replace(&spaces, ""));
                out.push('\n');
                i += 2;
                continue;
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
        i += 1;
    }
    out
}
