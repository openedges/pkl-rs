//! Structured diagnostics before product-specific rendering.
use crate::source::{Source, SourceId, Span};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticKind {
    Syntax,
    Unsupported,
    DuplicateMember,
    IntegerRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalFrame {
    pub source: SourceId,
    pub span: Span,
    pub member: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub span: Span,
    pub message: String,
    /// Synthetic JVM underline width (not a source byte length), e.g. at EOF.
    pub highlight_length_utf16: Option<usize>,
    /// Authored member path for delayed AST validation (e.g. `result` or `C.x`).
    /// The evaluation boundary inserts `#` between the module and this path.
    /// Syntax errors retain a module frame and leave this unset.
    pub member: Option<Box<str>>,
    /// Explicit module identity for AST validation after its header was parsed.
    pub module_name: Option<Box<str>>,
    pub frames: Vec<LogicalFrame>,
}

impl Diagnostic {
    pub fn new(kind: DiagnosticKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
            highlight_length_utf16: None,
            member: None,
            module_name: None,
            frames: Vec::new(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Diagnostic {}

/// A resolved logical frame at the uncolored text boundary.
///
/// The evaluator supplies source identities and authored names. The renderer does
/// not infer frames from message text or from the Rust call stack.
pub struct RenderFrame<'a> {
    pub source: &'a Source,
    pub uri: &'a str,
    pub member: &'a str,
    pub span: Span,
    pub highlight_length_utf16: Option<usize>,
}

/// Render the ordinary, uncolored Pkl exception layout.
///
/// Recursive frame compression, source transformers, and colored output remain
/// separate compatibility requirements.
pub fn render_error(message: &str, frames: &[RenderFrame<'_>], hint: Option<&str>) -> String {
    use std::fmt::Write;
    let mut output = format!("–– Pkl Error ––\n{message}\n");
    for (index, frame) in frames.iter().enumerate() {
        output.push('\n');
        render_frame(&mut output, frame);
        if index == 0
            && let Some(hint) = hint
        {
            write!(output, "\n{hint}\n").unwrap();
        }
    }
    if frames.is_empty()
        && let Some(hint) = hint
    {
        write!(output, "\n{hint}\n").unwrap();
    }
    output
}

fn render_frame(output: &mut String, frame: &RenderFrame<'_>) {
    use std::fmt::Write;
    let text = frame.source.text();
    let Some(position) = frame.source.position(frame.span.start) else {
        return;
    };
    let line_start = text[..frame.span.start]
        .rfind(['\r', '\n'])
        .map_or(0, |index| index + 1);
    let line_end = text[line_start..]
        .find(['\r', '\n'])
        .map_or(text.len(), |index| line_start + index);
    let original = &text[line_start..line_end];
    let stripped = original.trim_matches(java_whitespace);
    let leading_utf16 = original
        .chars()
        .take_while(|ch| java_whitespace(*ch))
        .map(char::len_utf16)
        .sum::<usize>();
    let column = position.column_utf16.saturating_sub(leading_utf16 + 1);
    let width = frame.highlight_length_utf16.unwrap_or_else(|| {
        let end = frame
            .source
            .position(frame.span.end)
            .filter(|end| end.line == position.line)
            .map_or_else(
                || stripped.encode_utf16().count().saturating_sub(column),
                |end| end.utf16.saturating_sub(position.utf16),
            );
        end.max(1)
    });
    let prefix = format!("{} | ", position.line);
    writeln!(output, "{prefix}{stripped}").unwrap();
    writeln!(
        output,
        "{}{}",
        " ".repeat(prefix.len() + column),
        "^".repeat(width)
    )
    .unwrap();
    writeln!(output, "at {} ({})", frame.member, frame.uri).unwrap();
}

// Java Character.isWhitespace deliberately excludes the non-breaking spaces.
fn java_whitespace(ch: char) -> bool {
    matches!(ch, '\u{0009}'..='\u{000d}' | '\u{001c}'..='\u{0020}' | '\u{1680}'
        | '\u{2000}'..='\u{2006}' | '\u{2008}'..='\u{200a}' | '\u{2028}'
        | '\u{2029}' | '\u{205f}' | '\u{3000}')
}

/// Property candidates in lookup order, with stable similarity ranking.
/// Method signatures require additional declaration metadata.
pub(crate) fn property_suggestions(
    name: &str,
    candidates: impl IntoIterator<Item = String>,
    include_base: bool,
) -> Option<String> {
    let mut seen = std::collections::BTreeSet::new();
    let base = crate::diagnostic_names::BASE_PROPERTIES
        .iter()
        .filter(move |_| include_base)
        .map(|name| (*name).to_owned());
    let mut ranked = candidates
        .into_iter()
        .chain(base)
        .filter_map(|candidate| {
            let similarity = string_similarity(&candidate, name);
            (candidate != name && similarity >= 0.77 && seen.insert(candidate.clone()))
                .then_some((candidate, similarity))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    (!ranked.is_empty()).then(|| {
        format!(
            "Did you mean any of the following?\n{}",
            ranked
                .into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

// Jaro-Winkler using UTF-16 units and the pinned StringSimilarity's integer
// transposition count and uncapped prefix rule.
fn string_similarity(first: &str, second: &str) -> f64 {
    if first == second {
        return 1.0;
    }
    let first = first.encode_utf16().collect::<Vec<_>>();
    let second = second.encode_utf16().collect::<Vec<_>>();
    let (long, short) = if first.len() > second.len() {
        (&first, &second)
    } else {
        (&second, &first)
    };
    let range = (long.len() / 2).saturating_sub(1);
    let mut matched = vec![false; long.len()];
    let mut short_matches = Vec::new();
    for (index, ch) in short.iter().enumerate() {
        if let Some(found) = (index.saturating_sub(range)..(index + range + 1).min(long.len()))
            .find(|candidate| !matched[*candidate] && long[*candidate] == *ch)
        {
            matched[found] = true;
            short_matches.push(*ch);
        }
    }
    let matches = short_matches.len();
    if matches == 0 {
        return 0.0;
    }
    let transpositions = short_matches
        .iter()
        .zip(
            long.iter()
                .zip(matched)
                .filter_map(|(ch, yes)| yes.then_some(ch)),
        )
        .filter(|(left, right)| left != right)
        .count()
        / 2;
    let count = matches as f64;
    let jaro = (count / first.len() as f64
        + count / second.len() as f64
        + (count - transpositions as f64) / count)
        / 3.0;
    if jaro > 0.7 {
        let prefix = first
            .iter()
            .zip(&second)
            .take_while(|(a, b)| a == b)
            .count();
        jaro + 0.1_f64.min(1.0 / long.len() as f64) * prefix as f64 * (1.0 - jaro)
    } else {
        jaro
    }
}

#[cfg(test)]
mod suggestion_tests {
    use super::property_suggestions;

    #[test]
    fn property_typos_use_candidates_and_base_without_fixture_specific_names() {
        assert_eq!(
            property_suggestions("missing", [], true).as_deref(),
            Some("Did you mean any of the following?\nListing")
        );
        assert_eq!(
            property_suggestions("answr", ["answer".into(), "answer".into()], false).as_deref(),
            Some("Did you mean any of the following?\nanswer")
        );
        assert_eq!(property_suggestions("missing", [], false), None);
    }
}
