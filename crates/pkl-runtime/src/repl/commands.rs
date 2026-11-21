//! REPL `:` command parsing and abbreviation resolution mirroring `org.pkl.cli.repl.ReplCommands`.

/// All 7 REPL commands in exact upstream enum declaration order (`ReplCommands.kt:35-45`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReplCommandKind {
    Clear,
    Examples,
    Force,
    Help,
    Load,
    Quit,
    Reset,
}

impl ReplCommandKind {
    /// All commands in upstream enum order.
    pub const ALL: [Self; 7] = [
        Self::Clear,
        Self::Examples,
        Self::Force,
        Self::Help,
        Self::Load,
        Self::Quit,
        Self::Reset,
    ];

    /// Canonical lowercase name without leading `:`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Examples => "examples",
            Self::Force => "force",
            Self::Help => "help",
            Self::Load => "load",
            Self::Quit => "quit",
            Self::Reset => "reset",
        }
    }
}

/// Matched REPL command with its parsed prefix, separating whitespace, and trailing argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedReplCommand {
    pub kind: ReplCommandKind,
    pub cmd: String,
    pub ws: String,
    pub arg: String,
}

/// Matches a `:` command line against Pkl 0.32.0's `Regex(":(\\p{Alpha}*)(\\p{Space}*)(.*)", DOT_MATCHES_ALL)`.
///
/// Note Java's `\p{Alpha}` is ASCII `[A-Za-z]` and `\p{Space}` is ASCII `[ \t\n\x0B\f\r]`
/// (because `UNICODE_CHARACTER_CLASS` is not set in `ReplCommands.kt:18`), and matching is
/// case-sensitive against lowercase enum names (`it.toString().lowercase().startsWith(cmd)`).
pub fn match_commands(input: &str) -> Vec<ParsedReplCommand> {
    let Some(rest) = input.strip_prefix(':') else {
        return Vec::new();
    };
    let mut cmd_end = 0;
    for (idx, ch) in rest.char_indices() {
        if ch.is_ascii_alphabetic() {
            cmd_end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    let cmd = &rest[..cmd_end];
    let after_cmd = &rest[cmd_end..];
    let mut ws_end = 0;
    for (idx, ch) in after_cmd.char_indices() {
        if matches!(ch, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r') {
            ws_end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    let ws = &after_cmd[..ws_end];
    let arg = &after_cmd[ws_end..];

    ReplCommandKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.as_str().starts_with(cmd))
        .map(|kind| ParsedReplCommand {
            kind,
            cmd: cmd.to_string(),
            ws: ws.to_string(),
            arg: arg.to_string(),
        })
        .collect()
}

/// Formats the ambiguous-command disambiguation message (`Repl.kt:152-153`).
/// Notice the two spaces before and between each `` `:<cmd>` `` candidate.
pub fn format_ambiguous_commands(candidates: &[ParsedReplCommand]) -> String {
    let mut out = String::from("Which of the following did you mean?  ");
    for (idx, candidate) in candidates.iter().enumerate() {
        if idx > 0 {
            out.push_str("  ");
        }
        out.push('`');
        out.push(':');
        out.push_str(candidate.kind.as_str());
        out.push('`');
    }
    out
}

/// Formats the unknown-command message (`Repl.kt:148`).
pub fn format_unknown_command(input: &str) -> String {
    let stripped = input.strip_prefix(':').unwrap_or(input);
    format!("Unknown command: `{stripped}`")
}
