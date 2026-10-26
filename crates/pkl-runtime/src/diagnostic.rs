//! Structured diagnostics before product-specific rendering.
mod frames;
mod highlight;

use crate::pkl_string::PklStringBuilder;
use crate::source::{Source, SourceId, Span};
use crate::value::PklString;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticKind {
    Syntax,
    Unsupported,
    DuplicateMember,
    IntegerRange,
}

/// The frontend component that produced a diagnostic, independent of its topic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticOrigin {
    Lexer,
    Parser,
    DeferredAstValidation,
    Resolver,
    Unclassified,
}

/// A failure's meaning, independent of its rendered bytes and compatibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureDisposition {
    LanguageError,
    Unsupported,
    LimitExceeded,
    InternalFailure,
}

impl FailureDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LanguageError => "language-error",
            Self::Unsupported => "unsupported",
            Self::LimitExceeded => "limit-exceeded",
            Self::InternalFailure => "internal-failure",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalFrame {
    pub source: SourceId,
    pub span: Span,
    pub member: PklString,
}

/// A detached observation from a completed constraint trace.
///
/// Runtime tracking owns and traces the original values until the check ends.
/// Diagnostics contain rendered snapshots, so retaining an error never keeps
/// evaluator heap handles alive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerAssertionValue {
    pub span: Span,
    pub value: PklString,
    pub executions: usize,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnionTypeDiagnostic {
    pub type_span: Span,
    pub nontrivial: bool,
    pub diagnostic: Box<TypeDiagnostic>,
}

/// Source-dependent type mismatch details, rendered at the common boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeDiagnostic {
    Simple {
        span: Span,
        message: PklString,
    },
    Constraint {
        span: Span,
        value: PklString,
        body_span: Span,
        observations: Option<Vec<PowerAssertionValue>>,
    },
    Union {
        span: Span,
        value: PklString,
        actual_type: String,
        is_null: bool,
        children: Vec<UnionTypeDiagnostic>,
    },
}

impl TypeDiagnostic {
    pub fn span(&self) -> Span {
        match self {
            Self::Simple { span, .. }
            | Self::Constraint { span, .. }
            | Self::Union { span, .. } => *span,
        }
    }

    pub fn render(&self, source: &Source) -> (PklString, Option<PklString>) {
        self.render_indented(source, "", true)
    }

    pub(crate) fn render_without_power(&self, source: &Source) -> PklString {
        self.render_indented(source, "", false).0
    }

    fn render_indented(
        &self,
        source: &Source,
        indent: &str,
        power: bool,
    ) -> (PklString, Option<PklString>) {
        let mut message_text = PklStringBuilder::default();
        match self {
            Self::Simple { message, .. } => {
                let units: Vec<_> = message.encode_utf16().collect();
                for (index, line) in units.split(|unit| *unit == 10).enumerate() {
                    if index > 0 {
                        message_text.push('\n');
                    }
                    message_text.push_str(indent);
                    message_text.push_value(&PklString::from_utf16(line.to_vec()));
                }
                (message_text.finish(), None)
            }
            Self::Constraint {
                span,
                value,
                body_span,
                observations,
            } => {
                let expression = source.slice(*span).unwrap_or("");
                message_text.push_str(&format!(
                    "{indent}Type constraint `{expression}` violated.\n{indent}Value: "
                ));
                message_text.push_value(value);
                if power && let Some(observations) = observations {
                    message_text.push_str("\n\n");
                    message_text.push_value(&crate::power_assertions::render(
                        source,
                        *body_span,
                        observations,
                        &format!("{indent}    "),
                    ));
                }
                (message_text.finish(), None)
            }
            Self::Union {
                span,
                value,
                actual_type,
                is_null,
                children,
            } => {
                let expected = source.slice(*span).unwrap_or("");
                let nontrivial: Vec<_> = children.iter().filter(|child| child.nontrivial).collect();
                let actual = if !nontrivial.is_empty() {
                    format!("a different `{actual_type}`")
                } else if *is_null {
                    "`null`".to_owned()
                } else {
                    format!("type `{actual_type}`")
                };
                message_text.push_str(&format!("{indent}Expected value of type `{expected}`, but got {actual}.\n{indent}Value: "));
                message_text.push_value(value);
                let hints: Vec<_> = nontrivial
                    .into_iter()
                    .map(|child| {
                        let expected = source.slice(child.type_span).unwrap_or("");
                        let (message, hint) =
                            child
                                .diagnostic
                                .render_indented(source, &format!("{indent}  "), power);
                        let mut text = PklStringBuilder::default();
                        text.push_str(&format!(
                            "{indent}* Value is not of type `{expected}` because:\n"
                        ));
                        text.push_value(&message);
                        if let Some(hint) = hint {
                            text.push_str("\n\n");
                            text.push_value(&hint);
                        }
                        text.finish()
                    })
                    .collect();
                (
                    message_text.finish(),
                    (!hints.is_empty()).then(|| PklString::join(&hints, "\n\n")),
                )
            }
        }
    }
}

/// Cold-path failure metadata, boxed so parser continuation results stay small.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticProvenance {
    pub origin: DiagnosticOrigin,
    pub disposition: FailureDisposition,
    pub reason: Option<&'static str>,
    /// Synthetic JVM underline width (not a source byte length), e.g. at EOF.
    pub highlight_length_utf16: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub provenance: Box<DiagnosticProvenance>,
    pub span: Span,
    pub message: PklString,
    /// Authored member path for delayed AST validation (e.g. `result` or `C.x`).
    /// The evaluation boundary inserts `#` between the module and this path.
    /// Syntax errors retain a module frame and leave this unset.
    pub member: Option<Box<PklString>>,
    /// Explicit module identity for AST validation after its header was parsed.
    pub module_name: Option<Box<str>>,
    pub frames: Box<[LogicalFrame]>,
    details: Option<Box<DiagnosticDetails>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiagnosticDetails {
    hint: PklString,
}

impl std::ops::Deref for Diagnostic {
    type Target = DiagnosticProvenance;
    fn deref(&self) -> &Self::Target {
        &self.provenance
    }
}
impl std::ops::DerefMut for Diagnostic {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.provenance
    }
}

impl Diagnostic {
    pub fn new(kind: DiagnosticKind, span: Span, message: impl Into<PklString>) -> Self {
        Self {
            kind,
            provenance: Box::new(DiagnosticProvenance {
                origin: DiagnosticOrigin::Unclassified,
                disposition: FailureDisposition::InternalFailure,
                reason: Some("unclassified-legacy-diagnostic"),
                highlight_length_utf16: None,
            }),
            span,
            message: message.into(),
            member: None,
            module_name: None,
            frames: Box::default(),
            details: None,
        }
    }

    pub fn language(
        origin: DiagnosticOrigin,
        kind: DiagnosticKind,
        span: Span,
        message: impl Into<PklString>,
    ) -> Self {
        let mut diagnostic = Self::new(kind, span, message);
        diagnostic.provenance.origin = origin;
        diagnostic.provenance.disposition = FailureDisposition::LanguageError;
        diagnostic.provenance.reason = None;
        diagnostic
    }

    pub fn unsupported(
        origin: DiagnosticOrigin,
        capability: &'static str,
        span: Span,
        message: impl Into<PklString>,
    ) -> Self {
        let mut diagnostic = Self::new(DiagnosticKind::Unsupported, span, message);
        diagnostic.provenance.origin = origin;
        diagnostic.provenance.disposition = FailureDisposition::Unsupported;
        diagnostic.provenance.reason = Some(capability);
        diagnostic
    }

    pub fn limit(
        origin: DiagnosticOrigin,
        resource: &'static str,
        span: Span,
        message: impl Into<PklString>,
    ) -> Self {
        let mut diagnostic = Self::unsupported(origin, resource, span, message);
        diagnostic.provenance.disposition = FailureDisposition::LimitExceeded;
        diagnostic
    }

    pub fn with_hint(mut self, hint: impl Into<PklString>) -> Self {
        self.details = Some(Box::new(DiagnosticDetails { hint: hint.into() }));
        self
    }

    pub fn hint(&self) -> Option<&PklString> {
        self.details.as_ref().map(|details| &details.hint)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message.to_java_utf8())
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
    pub member: &'a PklString,
    pub span: Span,
    pub highlight_length_utf16: Option<usize>,
}

/// Render the ordinary, uncolored Pkl exception layout.
///
/// Includes the pinned recursive frame compression and standard-library URL
/// transformation. Host transformations use the evaluator continuation.
pub fn render_error(message: &str, frames: &[RenderFrame<'_>], hint: Option<&str>) -> String {
    render_error_text(&message.into(), frames, hint.map(PklString::from).as_ref()).to_java_utf8()
}

/// Canonical uncolored diagnostic text before any UTF-8 writer boundary.
pub fn render_error_text(
    message: &PklString,
    frames: &[RenderFrame<'_>],
    hint: Option<&PklString>,
) -> PklString {
    let mut rendering = PreparedDiagnostic::new(message, frames, hint);
    rendering.transform(crate::logging::FrameTransformer::Default);
    rendering.render()
}

/// Detached rendering continuation. Frame compression precedes transformation,
/// and replacements retain all Java string code units and source positions.
#[derive(Clone, Debug)]
pub(crate) struct PreparedDiagnostic {
    message: PklString,
    hint: Option<PklString>,
    frames: frames::DiagnosticFrames,
}
impl PreparedDiagnostic {
    pub(crate) fn new(
        message: &PklString,
        frames: &[RenderFrame<'_>],
        hint: Option<&PklString>,
    ) -> Self {
        Self {
            message: message.clone(),
            hint: hint.cloned(),
            frames: frames::DiagnosticFrames::new(frames),
        }
    }
    pub(crate) fn next_frame(&self) -> Option<&crate::logging::LogFrame> {
        self.frames.next()
    }
    pub(crate) fn complete_frame(&mut self, frame: crate::logging::LogFrame) {
        self.frames.replace(frame);
    }
    pub(crate) fn transform(&mut self, policy: crate::logging::FrameTransformer) {
        while let Some(frame) = self.next_frame().cloned() {
            self.complete_frame(policy.transform(frame));
        }
    }
    pub(crate) fn render(&self) -> PklString {
        let mut output = PklStringBuilder::default();
        output.push_str("–– Pkl Error ––\n");
        output.push_value(&self.message);
        output.push('\n');
        if !self.frames.is_empty() {
            output.push('\n');
            self.frames.render(&mut output, self.hint.as_ref());
        } else if let Some(hint) = &self.hint {
            output.push('\n');
            output.push_value(hint);
        }
        output.finish()
    }
}

/// Validate at the render step immediately following each Host callback. Java
/// fails here before asking the transformer for the next compressed frame.
pub(crate) fn validate_transformed_frame(
    frame: &crate::logging::LogFrame,
) -> Result<(), PklString> {
    frame_line(frame).map(|_| ())
}

struct FrameLine {
    stripped: PklString,
    padding: usize,
    width: usize,
}

fn frame_line(frame: &crate::logging::LogFrame) -> Result<FrameLine, PklString> {
    let original = frame
        .source_lines
        .first()
        .ok_or_else(|| PklString::from("Index 0 out of bounds for length 0"))?;
    let units: Vec<_> = original.encode_utf16().collect();
    let whitespace = |unit: &u16| char::from_u32(u32::from(*unit)).is_some_and(java_whitespace);
    let leading = units.iter().take_while(|unit| whitespace(unit)).count();
    let trailing = units[leading..]
        .iter()
        .rev()
        .take_while(|unit| whitespace(unit))
        .count();
    let stripped = &units[leading..units.len() - trailing];
    let start = frame.start_column as i128 - leading as i128;
    let end = if frame.start_line == frame.end_line {
        frame.end_column as i128 - leading as i128
    } else {
        stripped.len() as i128
    };
    let padding = format!("{} | ", frame.start_line).len() as i128 + start - 1;
    let width = end - start + 1;
    // String.repeat checks its space repetition before its caret repetition.
    for count in [padding, width] {
        if count < 0 {
            return Err(format!("count is negative: {count}").into());
        }
    }
    Ok(FrameLine {
        stripped: PklString::from_utf16(stripped.to_vec()),
        padding: padding as usize,
        width: width as usize,
    })
}

fn render_log_frame(output: &mut PklStringBuilder, frame: &crate::logging::LogFrame, margin: &str) {
    use std::fmt::Write;
    // Invalid authored locations retain the legacy renderer omission. Host
    // replacements are validated by the continuation before reaching this code.
    let Ok(line) = frame_line(frame) else {
        return;
    };
    write!(output, "{margin}{} | ", frame.start_line).unwrap();
    output.push_value(&highlight::uncolored_text(&line.stripped));
    writeln!(
        output,
        "\n{margin}{}{}",
        " ".repeat(line.padding),
        "^".repeat(line.width)
    )
    .unwrap();
    write!(output, "{margin}at ").unwrap();
    if let Some(member) = &frame.member_name {
        output.push_value(member);
    } else {
        output.push_str("<unknown>");
    }
    output.push_str(" (");
    output.push_value(&frame.module_uri);
    output.push_str(")\n");
}

// Java Character.isWhitespace deliberately excludes the non-breaking spaces.
pub(crate) fn java_whitespace(ch: char) -> bool {
    matches!(ch, '\u{0009}'..='\u{000d}' | '\u{001c}'..='\u{0020}' | '\u{1680}'
        | '\u{2000}'..='\u{2006}' | '\u{2008}'..='\u{200a}' | '\u{2028}'
        | '\u{2029}' | '\u{205f}' | '\u{3000}')
}

/// Key candidates in definition order, with Java's stable similarity ranking.
pub(crate) fn key_suggestions(
    name: &PklString,
    candidates: impl IntoIterator<Item = PklString>,
) -> Vec<PklString> {
    let name = name.encode_utf16().collect::<Vec<_>>();
    let mut ranked = candidates
        .into_iter()
        .filter_map(|candidate| {
            let units = candidate.encode_utf16().collect::<Vec<_>>();
            let similarity = string_similarity_units(&units, &name);
            (similarity >= 0.77).then_some((candidate, similarity))
        })
        .collect::<Vec<_>>();
    // Java's Candidate comparison uses only the score. Its stable sort keeps
    // iteration order for ties, including inherited object entry keys.
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    ranked.into_iter().map(|(candidate, _)| candidate).collect()
}

/// Property candidates in lookup order, with stable similarity ranking.
/// Method signatures require additional declaration metadata.
pub(crate) fn property_suggestions(
    name: &str,
    candidates: impl IntoIterator<Item = String>,
    include_base: bool,
) -> Option<PklString> {
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
        .into()
    })
}

/// A candidate in member lookup order, before stable similarity ranking.
pub(crate) enum MemberSuggestion<'a> {
    Property(&'a str),
    Method {
        name: &'a str,
        signature: &'a str,
        arity: usize,
    },
}

/// Explicit receivers search all properties before methods.
pub(crate) fn member_suggestions(
    name: &str,
    arity: isize,
    properties: &[String],
    methods: &[(String, String, usize)],
) -> Option<PklString> {
    ordered_member_suggestions(
        name,
        arity,
        properties
            .iter()
            .map(|name| MemberSuggestion::Property(name))
            .chain(
                methods
                    .iter()
                    .map(|(name, signature, arity)| MemberSuggestion::Method {
                        name,
                        signature,
                        arity: *arity,
                    }),
            ),
    )
}

/// Match MemberLookupSuggestions without losing scope order across namespaces.
pub(crate) fn ordered_member_suggestions<'a>(
    name: &str,
    arity: isize,
    candidates: impl IntoIterator<Item = MemberSuggestion<'a>>,
) -> Option<PklString> {
    let mut seen = std::collections::BTreeSet::new();
    let mut ranked = Vec::new();
    for candidate in candidates {
        let (candidate, signature, count) = match candidate {
            MemberSuggestion::Property(name) => (name, name, -1),
            MemberSuggestion::Method {
                name,
                signature,
                arity,
            } => (name, signature, arity as isize),
        };
        let similarity = string_similarity(candidate, name);
        if similarity < 0.77 || !seen.insert((count >= 0, candidate)) {
            continue;
        }
        let signature = if similarity < 1.0 || arity == 0 {
            signature.to_owned()
        } else if arity >= 0 {
            format!("{signature}.apply(...)")
        } else {
            continue;
        };
        ranked.push((signature, similarity, (count - arity).abs()));
    }
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.2.cmp(&b.2)));
    (!ranked.is_empty()).then(|| {
        format!(
            "Did you mean any of the following?\n{}",
            ranked
                .into_iter()
                .map(|(signature, _, _)| signature)
                .collect::<Vec<_>>()
                .join("\n")
        )
        .into()
    })
}

// Jaro-Winkler using UTF-16 units and the pinned StringSimilarity's integer
// transposition count and uncapped prefix rule.
fn string_similarity(first: &str, second: &str) -> f64 {
    let first = first.encode_utf16().collect::<Vec<_>>();
    let second = second.encode_utf16().collect::<Vec<_>>();
    string_similarity_units(&first, &second)
}

fn string_similarity_units(first: &[u16], second: &[u16]) -> f64 {
    if first == second {
        return 1.0;
    }
    let (long, short) = if first.len() > second.len() {
        (first, second)
    } else {
        (second, first)
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
            .zip(second.iter())
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
            property_suggestions("missing", [], true)
                .as_ref()
                .and_then(|text| text.as_str()),
            Some("Did you mean any of the following?\nListing")
        );
        assert_eq!(
            property_suggestions("answr", ["answer".into(), "answer".into()], false)
                .as_ref()
                .and_then(|text| text.as_str()),
            Some("Did you mean any of the following?\nanswer")
        );
        assert_eq!(property_suggestions("missing", [], false), None);
    }
}
