//! Structured diagnostics before product-specific rendering.
pub mod ansi;
mod frames;
pub(crate) mod highlight;
pub mod mordant;

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
    OracleError,
}

impl FailureDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LanguageError => "language-error",
            Self::Unsupported => "unsupported",
            Self::LimitExceeded => "limit-exceeded",
            Self::InternalFailure => "internal-failure",
            Self::OracleError => "oracle-error",
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
    pub type_source: Option<crate::type_source::TypeSourceSection>,
    pub nontrivial: bool,
    pub diagnostic: Box<TypeDiagnostic>,
}

/// Source-dependent type mismatch details, rendered at the common boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeDiagnostic {
    AtSource {
        section: crate::type_source::TypeSourceSection,
        diagnostic: Box<TypeDiagnostic>,
    },
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
        actual_type: PklString,
        is_null: bool,
        children: Vec<UnionTypeDiagnostic>,
    },
}

impl TypeDiagnostic {
    pub fn span(&self) -> Span {
        match self {
            Self::AtSource { diagnostic, .. } => diagnostic.span(),
            Self::Simple { span, .. }
            | Self::Constraint { span, .. }
            | Self::Union { span, .. } => *span,
        }
    }

    pub(crate) fn source_section(&self) -> Option<&crate::type_source::TypeSourceSection> {
        match self {
            Self::AtSource { section, .. } => Some(section),
            _ => None,
        }
    }

    pub(crate) fn with_source(self, section: crate::type_source::TypeSourceSection) -> Self {
        if self.source_section().is_some() {
            self
        } else {
            Self::AtSource {
                section,
                diagnostic: Box::new(self),
            }
        }
    }

    pub fn render(&self, source: &Source) -> (PklString, Option<PklString>) {
        self.render_indented(source, "", true, false, ansi::AnsiCodeSet::EMPTY)
    }

    pub(crate) fn render_colored(&self, source: &Source) -> (PklString, Option<PklString>) {
        self.render_indented(source, "", true, true, ansi::theme::ERROR_MESSAGE)
    }

    pub(crate) fn render_without_power(&self, source: &Source) -> PklString {
        self.render_indented(source, "", false, false, ansi::AnsiCodeSet::EMPTY)
            .0
    }

    fn render_indented(
        &self,
        source: &Source,
        indent: &str,
        power: bool,
        use_color: bool,
        outer_style: ansi::AnsiCodeSet,
    ) -> (PklString, Option<PklString>) {
        let mut msg = ansi::AnsiStringBuilder::new(use_color);
        let mut hint_out: Option<PklString> = None;
        msg.append_scoped(outer_style, |out| {
            hint_out = self.render_indented_into(source, indent, power, use_color, out);
        });
        (msg.finish_pkl(), hint_out)
    }

    fn render_indented_into(
        &self,
        source: &Source,
        indent: &str,
        power: bool,
        use_color: bool,
        out: &mut ansi::AnsiStringBuilder,
    ) -> Option<PklString> {
        match self {
            Self::AtSource {
                section,
                diagnostic,
            } => section.with_source(|source| {
                diagnostic.render_indented_into(source, indent, power, use_color, out)
            }),
            Self::Simple { message, .. } => {
                let units: Vec<_> = message.encode_utf16().collect();
                for (index, line) in units.split(|unit| *unit == 10).enumerate() {
                    if index > 0 {
                        out.append_char('\n');
                    }
                    out.append_str(indent);
                    out.append_utf16(line);
                }
                None
            }
            Self::Constraint {
                span,
                value,
                body_span,
                observations,
            } => {
                let expression = source
                    .slice(*span)
                    .map(|text| text.to_pkl_string())
                    .unwrap_or_default();
                out.append_str(indent);
                out.append_str("Type constraint `");
                out.append_pkl(&expression);
                out.append_str(&format!("` violated.\n{indent}Value: "));
                out.append_pkl(value);
                if power && let Some(observations) = observations {
                    out.append_str("\n\n");
                    crate::power_assertions::render_to_ansi(
                        out,
                        source,
                        *body_span,
                        observations,
                        &format!("{indent}    "),
                        None,
                    );
                }
                None
            }
            Self::Union {
                span,
                value,
                actual_type,
                is_null,
                children,
            } => {
                let expected = source
                    .slice(*span)
                    .map(|text| text.to_pkl_string())
                    .unwrap_or_default();
                let nontrivial: Vec<_> = children.iter().filter(|child| child.nontrivial).collect();
                let actual = if !nontrivial.is_empty() {
                    PklString::from("a different `")
                        .concat(actual_type)
                        .concat(&"`".into())
                } else if *is_null {
                    PklString::from("`null`")
                } else {
                    PklString::from("type `")
                        .concat(actual_type)
                        .concat(&"`".into())
                };
                out.append_str(&format!("{indent}Expected value of type `"));
                out.append_pkl(&expected);
                out.append_str("`, but got ");
                out.append_pkl(&actual);
                out.append_str(&format!(".\n{indent}Value: "));
                out.append_pkl(value);
                if nontrivial.is_empty() {
                    return None;
                }
                let mut hint_builder = ansi::AnsiStringBuilder::new_with_active_style(
                    use_color,
                    ansi::theme::ERROR_MESSAGE_HINT,
                );
                for (idx, child) in nontrivial.into_iter().enumerate() {
                    if idx > 0 {
                        hint_builder.append_str("\n\n");
                    }
                    let expected = match &child.type_source {
                        Some(section) => section
                            .text()
                            .slice(section.span())
                            .map(|text| text.to_pkl_string())
                            .unwrap_or_default(),
                        None => source
                            .slice(child.type_span)
                            .map(|text| text.to_pkl_string())
                            .unwrap_or_default(),
                    };
                    hint_builder.append_str(&format!("{indent}* Value is not of type `"));
                    hint_builder.append_pkl(&expected);
                    hint_builder.append_str("` because:\n");
                    let child_hint = child.diagnostic.render_indented_into(
                        source,
                        &format!("{indent}  "),
                        power,
                        use_color,
                        &mut hint_builder,
                    );
                    if let Some(sub_hint) = child_hint {
                        hint_builder.append_str("\n\n");
                        hint_builder.append_precolored_pkl(&sub_hint);
                    }
                }
                Some(hint_builder.finish_raw_pkl())
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
    pub foreign_exception:
        Option<std::sync::Arc<crate::java_exception::provenance::ForeignExceptionProvenance>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub provenance: Box<DiagnosticProvenance>,
    pub span: Span,
    pub message: PklString,
    /// Authored member path for delayed AST validation (e.g. `result` or `C.x`).
    /// The evaluation boundary inserts `#` between the module and this path,
    /// unless it begins with an explicit `.` for a module annotation scope.
    /// Syntax errors retain a module frame and leave this unset.
    pub member: Option<Box<PklString>>,
    /// Explicit module identity for AST validation after its header was parsed.
    pub module_name: Option<Box<PklString>>,
    pub frames: Box<[LogicalFrame]>,
    details: Option<Box<DiagnosticDetails>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Utf16ColumnOffset(usize);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DiagnosticDetails {
    hint: Option<PklString>,
    /// Offset inside the anchored source scalar, not a byte or absolute index.
    highlight_offset_utf16: Utf16ColumnOffset,
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
                foreign_exception: None,
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
        self.details.get_or_insert_with(Default::default).hint = Some(hint.into());
        self
    }

    pub fn hint(&self) -> Option<&PklString> {
        self.details
            .as_ref()
            .and_then(|details| details.hint.as_ref())
    }

    /// Java's section start can fall between a valid scalar's surrogate units.
    /// `span` remains a valid UTF-8 anchor; this is the UTF-16 adjustment from it.
    pub fn highlight_offset_utf16(&self) -> usize {
        self.details
            .as_ref()
            .map_or(0, |details| details.highlight_offset_utf16.0)
    }

    pub(crate) fn with_highlight_offset_utf16(mut self, offset: usize) -> Self {
        if offset != 0 {
            self.details
                .get_or_insert_with(Default::default)
                .highlight_offset_utf16 = Utf16ColumnOffset(offset);
        }
        self
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
    pub uri: PklString,
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
    render_error_text_colored(message, frames, hint, false)
}

/// Canonical diagnostic text with optional ANSI color codes matching Pkl 0.32.0 `AnsiTheme`.
pub fn render_error_text_colored(
    message: &PklString,
    frames: &[RenderFrame<'_>],
    hint: Option<&PklString>,
    use_color: bool,
) -> PklString {
    let mut rendering = PreparedDiagnostic::new(message, frames, hint);
    rendering.transform(crate::logging::FrameTransformer::Default);
    rendering.render_colored(use_color)
}

/// Canonical diagnostic text with an uncolored `ProgramValue`/cause trailer beneath the
/// styled headline, matching `VmExceptionRenderer.java:112-134`.
pub fn render_error_text_with_trailer_colored(
    headline: &PklString,
    trailer: &PklString,
    frames: &[RenderFrame<'_>],
    hint: Option<&PklString>,
    use_color: bool,
) -> PklString {
    let full = headline.concat(&"\n".into()).concat(trailer);
    let mut rendering = PreparedDiagnostic::new(&full, frames, hint);
    rendering.set_message_split(headline.clone(), trailer.clone());
    rendering.transform(crate::logging::FrameTransformer::Default);
    rendering.render_colored(use_color)
}

pub(crate) fn render_frontend_error_text(
    error: &Diagnostic,
    frames: &[RenderFrame<'_>],
) -> PklString {
    render_frontend_error_text_colored(error, frames, false)
}

pub(crate) fn render_frontend_error_text_colored(
    error: &Diagnostic,
    frames: &[RenderFrame<'_>],
    use_color: bool,
) -> PklString {
    let mut rendering = PreparedDiagnostic::new_frontend(
        &error.message,
        frames,
        error.hint(),
        error.highlight_offset_utf16(),
    );
    rendering.transform(crate::logging::FrameTransformer::Default);
    rendering.render_colored(use_color)
}

/// Renders a frontend diagnostic without the `–– Pkl Error ––` header, matching
/// `VmExceptionRenderer.renderException(wrapped, sb, false)` for `VmWrappedEvalException`.
pub(crate) fn render_frontend_error_body_colored(
    error: &Diagnostic,
    frames: &[RenderFrame<'_>],
    use_color: bool,
) -> PklString {
    let mut rendering = PreparedDiagnostic::new_frontend(
        &error.message,
        frames,
        error.hint(),
        error.highlight_offset_utf16(),
    );
    rendering.transform(crate::logging::FrameTransformer::Default);
    rendering.render_colored_inner(use_color, false)
}

/// Detached rendering continuation. Frame compression precedes transformation,
/// and replacements retain all Java string code units and source positions.
#[derive(Clone, Debug)]
pub(crate) struct PreparedDiagnostic {
    message: PklString,
    message_split: Option<(PklString, PklString)>,
    hint: Option<PklString>,
    colored_message: Option<PklString>,
    colored_hint: Option<PklString>,
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
            message_split: None,
            hint: hint.cloned(),
            colored_message: None,
            colored_hint: None,
            frames: frames::DiagnosticFrames::new(frames),
        }
    }
    pub(crate) fn new_frontend(
        message: &PklString,
        frames: &[RenderFrame<'_>],
        hint: Option<&PklString>,
        first_offset_utf16: usize,
    ) -> Self {
        if first_offset_utf16 == 0 {
            return Self::new(message, frames, hint);
        }
        Self {
            message: message.clone(),
            message_split: None,
            hint: hint.cloned(),
            colored_message: None,
            colored_hint: None,
            frames: frames::DiagnosticFrames::new_with_first_offset(frames, first_offset_utf16),
        }
    }
    pub(crate) fn set_colored_override(&mut self, message: PklString, hint: Option<PklString>) {
        self.colored_message = Some(message);
        self.colored_hint = hint;
    }
    pub(crate) fn set_message_split(&mut self, headline: PklString, trailer: PklString) {
        self.message_split = Some((headline, trailer));
    }
    pub(crate) fn set_colored_hint(&mut self, hint: PklString) {
        self.colored_hint = Some(hint);
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
        self.render_colored(false)
    }

    pub(crate) fn render_colored(&self, use_color: bool) -> PklString {
        self.render_colored_inner(use_color, true)
    }

    fn render_colored_inner(&self, use_color: bool, with_header: bool) -> PklString {
        let mut out = ansi::AnsiStringBuilder::new(use_color);
        if with_header {
            out.append_styled(ansi::theme::ERROR_HEADER, "–– Pkl Error ––")
                .append_char('\n');
        }
        if use_color && let Some(precolored) = &self.colored_message {
            out.append_pkl(precolored);
        } else if let Some((headline, trailer)) = &self.message_split {
            out.append_styled_pkl(ansi::theme::ERROR_MESSAGE, headline)
                .append_char('\n')
                .append_pkl(trailer);
        } else {
            out.append_styled_pkl(ansi::theme::ERROR_MESSAGE, &self.message);
        }
        out.append_char('\n');
        if !self.frames.is_empty() {
            out.append_char('\n');
            if use_color && let Some(precolored_hint) = &self.colored_hint {
                self.frames
                    .render_ansi_precolored_hint(&mut out, precolored_hint);
            } else {
                self.frames.render_ansi(&mut out, self.hint.as_ref());
            }
        } else if let Some(hint) = &self.hint {
            out.append_char('\n');
            if use_color && let Some(precolored_hint) = &self.colored_hint {
                out.append_scoped(ansi::theme::ERROR_MESSAGE_HINT, |out| {
                    out.append_precolored_pkl(precolored_hint);
                });
            } else {
                out.append_styled_pkl(ansi::theme::ERROR_MESSAGE_HINT, hint);
            }
        }
        out.finish_pkl()
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
    let (stripped_pkl, leading, stripped_len) = if let Some(s) = original.as_str()
        && s.is_ascii()
    {
        let bytes = s.as_bytes();
        let is_ws = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c | 0x1c..=0x1f);
        let leading = bytes.iter().take_while(|&&b| is_ws(b)).count();
        let trailing = bytes[leading..]
            .iter()
            .rev()
            .take_while(|&&b| is_ws(b))
            .count();
        let trimmed = &s[leading..s.len() - trailing];
        let pkl = if leading == 0 && trailing == 0 {
            original.clone()
        } else {
            PklString::from(trimmed)
        };
        (pkl, leading, trimmed.len())
    } else {
        let units: Vec<_> = original.encode_utf16().collect();
        let whitespace = |unit: &u16| char::from_u32(u32::from(*unit)).is_some_and(java_whitespace);
        let leading = units.iter().take_while(|unit| whitespace(unit)).count();
        let trailing = units[leading..]
            .iter()
            .rev()
            .take_while(|unit| whitespace(unit))
            .count();
        let stripped = &units[leading..units.len() - trailing];
        (
            PklString::from_utf16(stripped.to_vec()),
            leading,
            stripped.len(),
        )
    };
    let start = frame.start_column as i128 - leading as i128;
    let end = if frame.start_line == frame.end_line {
        frame.end_column as i128 - leading as i128
    } else {
        stripped_len as i128
    };
    let mut line_digits = 1usize;
    let mut n = frame.start_line;
    while n >= 10 {
        n /= 10;
        line_digits += 1;
    }
    let padding = (line_digits + 3) as i128 + start - 1;
    let width = end - start + 1;
    // String.repeat checks its space repetition before its caret repetition.
    for count in [padding, width] {
        if count < 0 {
            return Err(format!("count is negative: {count}").into());
        }
    }
    Ok(FrameLine {
        stripped: stripped_pkl,
        padding: padding as usize,
        width: width as usize,
    })
}

fn render_log_frame_ansi(
    out: &mut ansi::AnsiStringBuilder,
    frame: &crate::logging::LogFrame,
    margin: &str,
) {
    // Invalid authored locations retain the legacy renderer omission. Host
    // replacements are validated by the continuation before reaching this code.
    let Ok(line) = frame_line(frame) else {
        return;
    };
    let prefix = format!("{} | ", frame.start_line);
    out.append_styled(ansi::theme::STACK_TRACE_MARGIN, margin)
        .append_styled(ansi::theme::STACK_TRACE_LINE_NUMBER, &prefix);
    highlight::write_to(out, &line.stripped);
    out.append_char('\n')
        .append_styled(ansi::theme::STACK_TRACE_MARGIN, margin)
        .append_str(&" ".repeat(line.padding))
        .append_styled(ansi::theme::STACK_TRACE_CARET, &"^".repeat(line.width))
        .append_char('\n');
    out.append_styled(ansi::theme::STACK_TRACE_MARGIN, margin)
        .append_scoped(ansi::theme::STACK_FRAME, |out| {
            out.append_str("at ");
            if let Some(member) = &frame.member_name {
                out.append_pkl(member);
            } else {
                out.append_str("<unknown>");
            }
            out.append_str(" (")
                .append_untrusted_pkl(&frame.module_uri)
                .append_str(")\n");
        });
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
    name: impl Into<PklString>,
    candidates: impl IntoIterator<Item = PklString>,
    include_base: bool,
) -> Option<PklString> {
    let name = name.into();
    let mut seen = std::collections::BTreeSet::new();
    let base = crate::diagnostic_names::BASE_PROPERTIES
        .iter()
        .filter(move |_| include_base)
        .map(|name| PklString::from(*name));
    let mut ranked = candidates
        .into_iter()
        .chain(base)
        .filter_map(|candidate| {
            let similarity = string_similarity_units(
                &candidate.encode_utf16().collect::<Vec<_>>(),
                &name.encode_utf16().collect::<Vec<_>>(),
            );
            (candidate != name && similarity >= 0.77 && seen.insert(candidate.clone()))
                .then_some((candidate, similarity))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    (!ranked.is_empty()).then(|| {
        PklString::from("Did you mean any of the following?\n").concat(&PklString::join(
            &ranked.into_iter().map(|(name, _)| name).collect::<Vec<_>>(),
            "\n",
        ))
    })
}

/// A candidate in member lookup order, before stable similarity ranking.
pub(crate) enum MemberSuggestion {
    Property(PklString),
    Method {
        name: PklString,
        signature: PklString,
        arity: usize,
    },
}

/// Explicit receivers search all properties before methods.
pub(crate) fn member_suggestions<T: Clone + Into<PklString>>(
    name: impl Into<PklString>,
    arity: isize,
    properties: &[T],
    methods: &[(T, T, usize)],
) -> Option<PklString> {
    ordered_member_suggestions(
        name,
        arity,
        properties
            .iter()
            .map(|name| MemberSuggestion::Property(name.clone().into()))
            .chain(
                methods
                    .iter()
                    .map(|(name, signature, arity)| MemberSuggestion::Method {
                        name: name.clone().into(),
                        signature: signature.clone().into(),
                        arity: *arity,
                    }),
            ),
    )
}

/// Match MemberLookupSuggestions without losing scope order across namespaces.
pub(crate) fn ordered_member_suggestions(
    name: impl Into<PklString>,
    arity: isize,
    candidates: impl IntoIterator<Item = MemberSuggestion>,
) -> Option<PklString> {
    let name = name.into();
    let mut seen = std::collections::BTreeSet::new();
    let mut ranked = Vec::new();
    for candidate in candidates {
        let (candidate, signature, count) = match candidate {
            MemberSuggestion::Property(name) => (name.clone(), name, -1),
            MemberSuggestion::Method {
                name,
                signature,
                arity,
            } => (name, signature, arity as isize),
        };
        let similarity = string_similarity_units(
            &candidate.encode_utf16().collect::<Vec<_>>(),
            &name.encode_utf16().collect::<Vec<_>>(),
        );
        if similarity < 0.77 || !seen.insert((count >= 0, candidate)) {
            continue;
        }
        let signature = if similarity < 1.0 || arity == 0 {
            signature.to_owned()
        } else if arity >= 0 {
            signature.concat(&".apply(...)".into())
        } else {
            continue;
        };
        ranked.push((signature, similarity, (count - arity).abs()));
    }
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.2.cmp(&b.2)));
    (!ranked.is_empty()).then(|| {
        PklString::from("Did you mean any of the following?\n").concat(&PklString::join(
            &ranked
                .into_iter()
                .map(|(signature, _, _)| signature)
                .collect::<Vec<_>>(),
            "\n",
        ))
    })
}

// Jaro-Winkler using UTF-16 units and the pinned StringSimilarity's integer
// transposition count and uncapped prefix rule.
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
