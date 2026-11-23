//! Wasm-pure Clikt 5 / Mordant 3 ANSI level detection and CLI help/error colorizer
//! matching Apple Pkl 0.32.0 (`org.pkl.commons.cli.commands.extensions.kt` and
//! `org.pkl.thirdparty.mordant.terminal.TerminalDetection`).

/// ANSI color capability levels emitted by Mordant 3 in Apple Pkl 0.32.0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnsiLevel {
    None,
    Ansi16,
    Ansi256,
    TrueColor,
}

/// Detects the active Mordant [`AnsiLevel`] from the current process environment.
pub fn detect_ansi_level() -> AnsiLevel {
    detect_ansi_level_with(|key| std::env::var(key).ok())
}

/// Pure, testable port of `org.pkl.thirdparty.mordant.terminal.TerminalDetection.ansiLevel`
/// as shipped in Apple Pkl 0.32.0 (`pkl` and `pkldoc`).
pub fn detect_ansi_level_with(get_env: impl Fn(&str) -> Option<String>) -> AnsiLevel {
    let term_lower = get_env("TERM").map(|v| v.to_ascii_lowercase());
    let term_program_lower = get_env("TERM_PROGRAM").map(|v| v.to_ascii_lowercase());

    // 1. forcedColor()
    if term_lower.as_deref() == Some("dumb") {
        return AnsiLevel::None;
    }
    if get_env("NO_COLOR").is_some() {
        return AnsiLevel::None;
    }
    if let Some(force) = get_env("FORCE_COLOR") {
        match force.to_ascii_lowercase().as_str() {
            "0" | "false" | "none" => return AnsiLevel::None,
            "" | "1" | "true" | "16color" => return AnsiLevel::Ansi16,
            "2" | "256color" => return AnsiLevel::Ansi256,
            "3" | "truecolor" => return AnsiLevel::TrueColor,
            _ => {}
        }
    }

    // 2. VS Code terminal
    if term_program_lower.as_deref() == Some("vscode") {
        return AnsiLevel::TrueColor;
    }

    // 3. Windows Terminal / DomTerm / JetBrains JediTerm
    if get_env("WT_SESSION").is_some_and(|v| !v.is_empty())
        || get_env("DOMTERM").is_some_and(|v| !v.is_empty())
        || get_env("TERMINAL_EMULATOR").as_deref() == Some("JetBrains-JediTerm")
    {
        return AnsiLevel::TrueColor;
    }

    // 4. COLORTERM
    if let Some(colorterm) = get_env("COLORTERM") {
        match colorterm.to_ascii_lowercase().as_str() {
            "24bit" | "24bits" | "truecolor" => return AnsiLevel::TrueColor,
            _ => {}
        }
    }

    // 5. CI detection
    if get_env("CI").is_some() {
        const CI_COLOR_VARS: &[&str] = &[
            "APPVEYOR",
            "BUILDKITE",
            "CIRCLECI",
            "DRONE",
            "GITHUB_ACTIONS",
            "GITLAB_CI",
            "TRAVIS",
        ];
        if CI_COLOR_VARS.iter().any(|k| get_env(k).is_some()) {
            return AnsiLevel::Ansi256;
        }
        return AnsiLevel::None;
    }

    // 6. TERM_PROGRAM
    if let Some(tp) = term_program_lower.as_deref() {
        match tp {
            "wezterm" | "hyper" | "mintty" => return AnsiLevel::TrueColor,
            "apple_terminal" => return AnsiLevel::Ansi256,
            "iterm.app" => {
                let major = get_env("TERM_PROGRAM_VERSION")
                    .and_then(|v| v.split('.').next()?.parse::<i32>().ok());
                return if major.is_some_and(|m| m >= 3) {
                    AnsiLevel::TrueColor
                } else {
                    AnsiLevel::Ansi256
                };
            }
            _ => {}
        }
    }

    // 7. TERM first/last hyphen-delimited segments
    if let Some(term) = term_lower.as_deref() {
        let mut parts = term.split('-');
        let first = parts.next();
        let last = parts.next_back().or(first);
        if let Some(suffix) = last {
            match suffix {
                "256color" | "256colors" | "256" => return AnsiLevel::Ansi256,
                "24bit" | "24bits" | "truecolor" | "direct" => return AnsiLevel::TrueColor,
                _ => {}
            }
        }
        if first == Some("xterm") {
            if let Some(ver_str) = get_env("XTERM_VERSION")
                && let Some(ver) = extract_paren_int(&ver_str)
            {
                if ver >= 331 {
                    return AnsiLevel::TrueColor;
                }
                if ver >= 122 {
                    return AnsiLevel::Ansi256;
                }
            }
            return AnsiLevel::Ansi16;
        }
        if let Some(prefix) = first {
            match prefix {
                "cygwin" => return AnsiLevel::TrueColor,
                "rxvt" | "color" | "vt220" | "vt100" | "linux" | "konsole" | "screen" | "tmux"
                | "ansi" => return AnsiLevel::Ansi16,
                _ => {}
            }
        }
    }

    AnsiLevel::None
}

fn extract_paren_int(s: &str) -> Option<i32> {
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'(' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > start && end < bytes.len() && bytes[end] == b')' {
                return s[start..end].parse::<i32>().ok();
            }
        }
    }
    None
}

impl AnsiLevel {
    fn header(self, s: &str) -> String {
        match self {
            Self::None => s.to_string(),
            Self::Ansi16 => format!("\x1b[33m{s}\x1b[39m"),
            Self::Ansi256 => format!("\x1b[38;5;186m{s}\x1b[39m"),
            Self::TrueColor => format!("\x1b[38;2;229;192;123m{s}\x1b[39m"),
        }
    }

    fn flag(self, s: &str) -> String {
        match self {
            Self::None => s.to_string(),
            Self::Ansi16 => format!("\x1b[36m{s}\x1b[39m"),
            Self::Ansi256 => format!("\x1b[38;5;111m{s}\x1b[39m"),
            Self::TrueColor => format!("\x1b[38;2;97;175;239m{s}\x1b[39m"),
        }
    }

    fn metavar(self, s: &str) -> String {
        match self {
            Self::None => s.to_string(),
            Self::Ansi16 => format!("\x1b[33;2m{s}\x1b[39;22m"),
            Self::Ansi256 => format!("\x1b[38;5;186;2m{s}\x1b[39;22m"),
            Self::TrueColor => format!("\x1b[38;2;229;192;123;2m{s}\x1b[39;22m"),
        }
    }

    fn error(self, s: &str) -> String {
        match self {
            Self::None => s.to_string(),
            Self::Ansi16 => format!("\x1b[31m{s}\x1b[39m"),
            Self::Ansi256 => format!("\x1b[38;5;174m{s}\x1b[39m"),
            Self::TrueColor => format!("\x1b[38;2;224;108;117m{s}\x1b[39m"),
        }
    }

    fn dim(self, s: &str) -> String {
        match self {
            Self::None => s.to_string(),
            _ => format!("\x1b[2m{s}\x1b[22m"),
        }
    }

    fn bold(self, s: &str) -> String {
        match self {
            Self::None => s.to_string(),
            _ => format!("\x1b[1m{s}\x1b[22m"),
        }
    }
}

/// Colorizes a Clikt `Usage:` line (and any wrapped continuation lines) for `level`.
pub fn colorize_usage_block(usage: &str, level: AnsiLevel) -> String {
    if level == AnsiLevel::None {
        return usage.to_string();
    }
    let mut out = String::with_capacity(usage.len() + 64);
    for (idx, line) in usage.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if let Some(rest) = line.strip_prefix("Usage:") {
            out.push_str(&level.header("Usage:"));
            colorize_usage_tokens(rest, level, &mut out);
        } else {
            colorize_usage_tokens(line, level, &mut out);
        }
    }
    out
}

fn colorize_usage_tokens(rest: &str, level: AnsiLevel, out: &mut String) {
    let mut i = 0usize;
    let bytes = rest.as_bytes();
    while i < bytes.len() {
        // Clikt dims `<command> [<args>]...` as a single span (or `<command>` when wrapped).
        if rest[i..].starts_with("<command> [<args>]...") {
            out.push_str(&level.dim("<command> [<args>]..."));
            i += "<command> [<args>]...".len();
            continue;
        }
        if rest[i..].starts_with("<command>") {
            out.push_str(&level.dim("<command>"));
            i += "<command>".len();
            continue;
        }
        // Optional usage elements wrapped in `[...]` (with optional trailing `...`).
        if bytes[i] == b'[' {
            let start = i;
            let mut end = i + 1;
            while end < bytes.len() && bytes[end] != b']' {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b']' {
                end += 1;
                if rest[end..].starts_with("...") {
                    end += 3;
                }
                out.push_str(&level.dim(&rest[start..end]));
                i = end;
                continue;
            }
        }
        let ch = rest[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HelpSection {
    Usage,
    Prolog,
    Options,
    Arguments,
    Commands,
}

/// Colorizes a plain Clikt 5 help screen matching Apple Pkl 0.32.0's Mordant theme.
pub fn colorize_clikt_help(plain: &str, level: AnsiLevel) -> String {
    if level == AnsiLevel::None {
        return plain.to_string();
    }
    let had_trailing_nl = plain.ends_with('\n');
    let body = plain.strip_suffix('\n').unwrap_or(plain);
    let mut section = HelpSection::Prolog;
    let mut out = String::with_capacity(plain.len() + 1024);

    for (idx, line) in body.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if let Some(rest) = line.strip_prefix("Usage:") {
            section = HelpSection::Usage;
            out.push_str(&level.header("Usage:"));
            colorize_usage_tokens(rest, level, &mut out);
            continue;
        }
        if section == HelpSection::Usage {
            if line.starts_with("    ") && !line.trim().is_empty() {
                colorize_usage_tokens(line, level, &mut out);
                continue;
            }
            section = HelpSection::Prolog;
        }
        match line {
            "Options:" => {
                section = HelpSection::Options;
                out.push_str(&level.header("Options:"));
                continue;
            }
            "Arguments:" => {
                section = HelpSection::Arguments;
                out.push_str(&level.header("Arguments:"));
                continue;
            }
            "Commands:" => {
                section = HelpSection::Commands;
                out.push_str(&level.header("Commands:"));
                continue;
            }
            "For more information, visit" => {
                section = HelpSection::Prolog;
                out.push_str(line);
                continue;
            }
            _ => {}
        }

        if line.starts_with("https://") {
            out.push_str(&level.flag(line));
            continue;
        }

        match section {
            HelpSection::Prolog | HelpSection::Usage => {
                // Fenced markdown code block box rendered by Mordant (`  │...│`).
                if let Some(inner) = line
                    .strip_prefix("  \u{2502}")
                    .and_then(|s| s.strip_suffix('\u{2502}'))
                {
                    out.push_str("  \u{2502}");
                    out.push_str(&level.flag(inner));
                    out.push('\u{2502}');
                } else {
                    out.push_str(&apply_known_markdown_bolds(line, level));
                }
            }
            HelpSection::Options => {
                if let Some(rest) = line.strip_prefix("  ")
                    && rest.starts_with('-')
                {
                    out.push_str("  ");
                    colorize_option_line(rest, level, &mut out);
                } else {
                    out.push_str(&apply_known_markdown_bolds(line, level));
                }
            }
            HelpSection::Arguments => {
                if let Some(rest) = line.strip_prefix("  ")
                    && rest.starts_with('<')
                    && let Some(close) = rest.find('>')
                {
                    out.push_str("  ");
                    out.push_str(&level.flag(&rest[..=close]));
                    out.push_str(&apply_known_markdown_bolds(&rest[close + 1..], level));
                } else {
                    out.push_str(&apply_known_markdown_bolds(line, level));
                }
            }
            HelpSection::Commands => {
                if let Some(rest) = line.strip_prefix("  ")
                    && !rest.is_empty()
                    && !rest.starts_with(' ')
                {
                    out.push_str("  ");
                    match rest.find(' ') {
                        Some(sp) => {
                            out.push_str(&level.flag(&rest[..sp]));
                            out.push_str(&rest[sp..]);
                        }
                        None => out.push_str(&level.flag(rest)),
                    }
                } else {
                    out.push_str(line);
                }
            }
        }
    }

    if had_trailing_nl {
        out.push('\n');
    }
    out
}

/// Splits an option line (after the leading `"  "`) into `(spec, spacing_and_desc)`.
/// Metavars can contain single spaces (e.g. `<header name>:<header value>`), while Clikt
/// always separates the option spec from its description with at least two spaces (`"  "`).
fn split_option_spec_and_desc(rest: &str) -> (&str, &str) {
    if let Some(pos) = rest.find("  ") {
        (&rest[..pos], &rest[pos..])
    } else {
        (rest, "")
    }
}

fn colorize_option_line(rest: &str, level: AnsiLevel, out: &mut String) {
    let (spec, desc) = split_option_spec_and_desc(rest);
    colorize_option_spec(spec, level, out);
    if !desc.is_empty() {
        out.push_str(&apply_known_markdown_bolds(desc, level));
    }
}

fn colorize_option_spec(spec: &str, level: AnsiLevel, out: &mut String) {
    // Handle boolean toggle pairs like `--power-assertions / --no-power-assertions`
    // or `-v, --verbose / --no-verbose`.
    if let Some((left, right)) = spec.split_once(" / ") {
        colorize_option_spec(left, level, out);
        out.push_str(" / ");
        colorize_option_spec(right, level, out);
        return;
    }
    // Split `, ` separated aliases (`-w, --working-dir=<path>`), ignoring commas inside
    // angle-bracketed metavars such as `--mode=<fast, safe>`.
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut angle_depth = 0usize;
    let bytes = spec.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'<' => angle_depth += 1,
            b'>' => angle_depth = angle_depth.saturating_sub(1),
            b',' if angle_depth == 0 && bytes.get(i + 1) == Some(&b' ') => {
                parts.push(&spec[start..i]);
                i += 2;
                start = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&spec[start..]);

    for (idx, part) in parts.into_iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        if let Some((flag_name, mv)) = part.split_once('=') {
            out.push_str(&level.flag(flag_name));
            out.push('=');
            out.push_str(&level.metavar(mv));
        } else {
            out.push_str(&level.flag(part));
        }
    }
}

/// Applies Apple Pkl 0.32.0's inline markdown code bold spans (`Theme { styles["markdown.code.span"] = TextStyle(bold = true) }`).
fn apply_known_markdown_bolds(line: &str, level: AnsiLevel) -> String {
    if level == AnsiLevel::None {
        return line.to_string();
    }
    // Exact markdown code spans present in Apple Pkl 0.32.0 CLI help texts.
    let mut s = line.to_string();
    if s.contains("resolving modulepath: URIs.") {
        s = s.replace(
            "resolving modulepath: URIs.",
            &format!("resolving {} URIs.", level.bold("modulepath:")),
        );
    }
    if s.contains("1: 0.25 - 0.29") {
        s = s.replace(
            "1: 0.25 - 0.29",
            &format!("{}: 0.25 - 0.29", level.bold("1")),
        );
    }
    if s.contains("2: 0.30+ (default)") {
        s = s.replace(
            "2: 0.30+ (default)",
            &format!("{}: 0.30+ {}", level.bold("2"), level.bold("(default)")),
        );
    }
    if s.contains("---diff-name-only.") {
        s = s.replace(
            "---diff-name-only.",
            &format!("{}.", level.bold("---diff-name-only")),
        );
    }
    if s.contains("with --diff-name-only.") {
        s = s.replace(
            "with --diff-name-only.",
            &format!("with {}.", level.bold("--diff-name-only")),
        );
    }
    if s.contains("Use - to read from stdin.") {
        s = s.replace(
            "Use - to read from stdin.",
            &format!("Use {} to read from stdin.", level.bold("-")),
        );
    }
    if s.contains("takes the dependencies of PklProjects") {
        s = s.replace(
            "takes the dependencies of PklProjects",
            &format!(
                "takes the {} of {}s",
                level.bold("dependencies"),
                level.bold("PklProject")
            ),
        );
    }
    if s.contains("versions to PklProject.deps.json files.") {
        s = s.replace(
            "versions to PklProject.deps.json files.",
            &format!("versions to {} files.", level.bold("PklProject.deps.json")),
        );
    }
    if s.contains("as defined by apiTests in PklProject") {
        s = s.replace(
            "as defined by apiTests in PklProject",
            &format!(
                "as defined by {} in {}",
                level.bold("apiTests"),
                level.bold("PklProject")
            ),
        );
    }
    for bullet_code in [
        "name@version.zip.sha256",
        "name@version.sha256",
        "name@version.zip",
        "name@version",
    ] {
        let needle = format!(" \u{2022} {bullet_code} - ");
        if s.contains(&needle) {
            s = s.replace(
                &needle,
                &format!(" \u{2022} {} - ", level.bold(bullet_code)),
            );
            break;
        }
    }
    s
}

/// Colorizes a Clikt 5 usage/parse error block (`Usage: ...\n\nError: ...`) for `level`.
pub fn colorize_clikt_usage_error(plain: &str, level: AnsiLevel) -> String {
    if level == AnsiLevel::None {
        return plain.to_string();
    }
    let had_trailing_nl = plain.ends_with('\n');
    let body = plain.strip_suffix('\n').unwrap_or(plain);
    let mut out = String::with_capacity(plain.len() + 128);
    let mut in_usage = false;

    for (idx, line) in body.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if let Some(rest) = line.strip_prefix("Usage:") {
            in_usage = true;
            out.push_str(&level.header("Usage:"));
            colorize_usage_tokens(rest, level, &mut out);
            continue;
        }
        if in_usage {
            if line.starts_with("    ") && !line.trim().is_empty() {
                colorize_usage_tokens(line, level, &mut out);
                continue;
            }
            in_usage = false;
        }
        if let Some(rest) = line.strip_prefix("Error: ") {
            out.push_str(&level.error("Error:"));
            out.push(' ');
            out.push_str(&colorize_clikt_error_message(rest, level));
        } else {
            out.push_str(line);
        }
    }

    if had_trailing_nl {
        out.push('\n');
    }
    out
}

fn colorize_clikt_error_message(msg: &str, level: AnsiLevel) -> String {
    if let Some(rest) = msg.strip_prefix("no such option ") {
        return colorize_named_with_suggestions("no such option ", rest, level);
    }
    if let Some(rest) = msg.strip_prefix("no such subcommand ") {
        return colorize_named_with_suggestions("no such subcommand ", rest, level);
    }
    if let Some(rest) = msg.strip_prefix("missing argument ") {
        return format!("missing argument {}", level.flag(rest));
    }
    if let Some(rest) = msg.strip_prefix("missing option ") {
        return format!("missing option {}", level.flag(rest));
    }
    if let Some(rest) = msg.strip_prefix("option ") {
        for suffix in [" requires a value", " does not take a value"] {
            if let Some(name) = rest.strip_suffix(suffix) {
                return format!("option {}{suffix}", level.flag(name));
            }
        }
    }
    if let Some(rest) = msg.strip_prefix("invalid value for ")
        && let Some((name, detail)) = rest.split_once(": ")
    {
        return format!("invalid value for {}: {detail}", level.flag(name));
    }
    msg.to_string()
}

fn colorize_named_with_suggestions(prefix: &str, rest: &str, level: AnsiLevel) -> String {
    for (marker, open) in [
        (". (Possible options: ", ". (Possible options: "),
        (". (Possible subcommands: ", ". (Possible subcommands: "),
    ] {
        if let Some((name, tail)) = rest.split_once(marker)
            && let Some(items_str) = tail.strip_suffix(')')
        {
            let colored_items: Vec<String> =
                items_str.split(", ").map(|item| level.flag(item)).collect();
            return format!(
                "{prefix}{}{open}{})",
                level.flag(name),
                colored_items.join(", ")
            );
        }
    }
    if let Some((name, suggestion)) = rest.split_once(". Did you mean ")
        && let Some(target) = suggestion.strip_suffix('?')
    {
        return format!(
            "{prefix}{}. Did you mean {}?",
            level.flag(name),
            level.flag(target)
        );
    }
    format!("{prefix}{}", level.flag(rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn mordant_detection_matches_pkl_0_32_0() {
        assert_eq!(detect_ansi_level_with(env_from(&[])), AnsiLevel::None);
        assert_eq!(
            detect_ansi_level_with(env_from(&[("TERM", "dumb")])),
            AnsiLevel::None
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[
                ("TERM", "dumb"),
                ("COLORTERM", "truecolor"),
                ("FORCE_COLOR", "3"),
            ])),
            AnsiLevel::None
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[
                ("TERM", "xterm-256color"),
                ("COLORTERM", "truecolor"),
                ("NO_COLOR", ""),
            ])),
            AnsiLevel::None
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[("FORCE_COLOR", "1")])),
            AnsiLevel::Ansi16
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[("FORCE_COLOR", "2")])),
            AnsiLevel::Ansi256
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[("FORCE_COLOR", "3")])),
            AnsiLevel::TrueColor
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[("TERM", "xterm")])),
            AnsiLevel::Ansi16
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[("TERM", "xterm-256color")])),
            AnsiLevel::Ansi256
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[
                ("TERM", "xterm-256color"),
                ("COLORTERM", "truecolor"),
            ])),
            AnsiLevel::TrueColor
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[("TERM", "alacritty")])),
            AnsiLevel::None
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[("TERM", "xterm-256color"), ("CI", "true"),])),
            AnsiLevel::None
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[
                ("TERM", "xterm-256color"),
                ("CI", "true"),
                ("GITHUB_ACTIONS", "true"),
            ])),
            AnsiLevel::Ansi256
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[
                ("TERM", "xterm"),
                ("TERM_PROGRAM", "iTerm.app"),
                ("TERM_PROGRAM_VERSION", "3.5.0"),
            ])),
            AnsiLevel::TrueColor
        );
        assert_eq!(
            detect_ansi_level_with(env_from(&[
                ("TERM", "xterm"),
                ("TERM_PROGRAM", "iTerm.app"),
                ("TERM_PROGRAM_VERSION", "2.9.0"),
            ])),
            AnsiLevel::Ansi256
        );
    }

    #[test]
    fn usage_and_error_colorize_matches_truecolor_palette() {
        let plain =
            "Usage: pkl eval [<options>] <modules>...\n\nError: missing argument <modules>\n";
        assert_eq!(
            colorize_clikt_usage_error(plain, AnsiLevel::TrueColor),
            "\x1b[38;2;229;192;123mUsage:\x1b[39m pkl eval \x1b[2m[<options>]\x1b[22m <modules>...\n\n\x1b[38;2;224;108;117mError:\x1b[39m missing argument \x1b[38;2;97;175;239m<modules>\x1b[39m\n"
        );
        assert_eq!(colorize_clikt_usage_error(plain, AnsiLevel::None), plain);
    }

    #[test]
    fn option_metavar_commas_and_wrapped_usage_command_stay_intact() {
        let help = "Usage: tool [<options>] <command>\n    [<args>]...\n\nOptions:\n  -m, --mode=<fast, safe>  Mode.\n";
        let colored = colorize_clikt_help(help, AnsiLevel::Ansi16);
        assert_eq!(
            colored,
            "\x1b[33mUsage:\x1b[39m tool \x1b[2m[<options>]\x1b[22m \x1b[2m<command>\x1b[22m\n    \x1b[2m[<args>]...\x1b[22m\n\n\x1b[33mOptions:\x1b[39m\n  \x1b[36m-m\x1b[39m, \x1b[36m--mode\x1b[39m=\x1b[33;2m<fast, safe>\x1b[39;22m  Mode.\n"
        );
    }
}
