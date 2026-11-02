//! Lossless Java throwable and Pkl bug-diagnostic rendering.
//!
//! The caller supplies the captured throwable graph and runtime profile. This
//! module does not synthesize JVM frames or turn host failures into exceptions.

use crate::value::PklString;

/// Identity of a throwable in a [`JavaThrowableGraph`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JavaThrowableId(pub usize);

/// Java `StackTraceElement` data, including its display-only format flags.
#[derive(Clone, Debug)]
pub struct JavaStackTraceElement {
    pub class_loader_name: Option<PklString>,
    pub module_name: Option<PklString>,
    pub module_version: Option<PklString>,
    pub declaring_class: PklString,
    pub method_name: PklString,
    pub file_name: Option<PklString>,
    /// Java uses -2 for native methods; other negative values omit the line.
    pub line_number: i32,
    pub omit_class_loader_name: bool,
    pub omit_module_version: bool,
}

impl PartialEq for JavaStackTraceElement {
    fn eq(&self, other: &Self) -> bool {
        self.class_loader_name == other.class_loader_name
            && self.module_name == other.module_name
            && self.module_version == other.module_version
            && self.declaring_class == other.declaring_class
            && self.method_name == other.method_name
            && self.file_name == other.file_name
            && self.line_number == other.line_number
    }
}
impl Eq for JavaStackTraceElement {}

impl JavaStackTraceElement {
    /// Format as JDK `StackTraceElement.toString()`.
    pub fn render(&self) -> PklString {
        let mut out = Vec::new();
        let mut has_prefix = false;
        if let Some(name) = &self.class_loader_name
            && !self.omit_class_loader_name
            && name.encode_utf16().next().is_some()
        {
            push(&mut out, name);
            ascii(&mut out, "/");
            has_prefix = true;
        }
        if let Some(name) = &self.module_name
            && name.encode_utf16().next().is_some()
        {
            push(&mut out, name);
            has_prefix = true;
            if let Some(version) = &self.module_version
                && !self.omit_module_version
                && version.encode_utf16().next().is_some()
            {
                ascii(&mut out, "@");
                push(&mut out, version);
            }
        }
        if has_prefix {
            ascii(&mut out, "/");
        }
        push(&mut out, &self.declaring_class);
        ascii(&mut out, ".");
        push(&mut out, &self.method_name);
        ascii(&mut out, "(");
        if self.line_number == -2 {
            ascii(&mut out, "Native Method");
        } else if let Some(file) = &self.file_name {
            push(&mut out, file);
            if self.line_number >= 0 {
                ascii(&mut out, &format!(":{}", self.line_number));
            }
        } else {
            ascii(&mut out, "Unknown Source");
        }
        ascii(&mut out, ")");
        PklString::from_utf16(out)
    }
}

/// Captured Java throwable fields. `localized_message` is the effective
/// `getLocalizedMessage()` result, independently of `message`/`getMessage()`.
#[derive(Clone, Debug)]
pub struct JavaThrowable {
    pub class_name: PklString,
    pub message: Option<PklString>,
    pub localized_message: Option<PklString>,
    /// Captured effective result when the subclass overrides `toString()`.
    pub to_string_override: Option<PklString>,
    pub stack_trace: Vec<JavaStackTraceElement>,
    pub cause: Option<JavaThrowableId>,
    pub suppressed: Vec<JavaThrowableId>,
}

impl JavaThrowable {
    /// Construct a throwable with Java's default localized-message behavior.
    pub fn new(class_name: impl Into<PklString>, message: Option<PklString>) -> Self {
        Self {
            class_name: class_name.into(),
            localized_message: message.clone(),
            to_string_override: None,
            message,
            stack_trace: Vec::new(),
            cause: None,
            suppressed: Vec::new(),
        }
    }
}

/// An identity-preserving throwable graph; cycles and sharing are supported.
#[derive(Clone, Debug, Default)]
pub struct JavaThrowableGraph {
    pub throwables: Vec<JavaThrowable>,
}

/// An out-of-range graph reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidThrowableId(pub JavaThrowableId);

impl std::fmt::Display for InvalidThrowableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid Java throwable id {}", self.0.0)
    }
}
impl std::error::Error for InvalidThrowableId {}

impl JavaThrowableGraph {
    /// Validate all cause and suppressed references, including unreachable nodes.
    pub fn validate(&self) -> Result<(), InvalidThrowableId> {
        for throwable in &self.throwables {
            for id in throwable.cause.iter().chain(&throwable.suppressed) {
                self.get(*id)?;
            }
        }
        Ok(())
    }

    fn get(&self, id: JavaThrowableId) -> Result<&JavaThrowable, InvalidThrowableId> {
        self.throwables.get(id.0).ok_or(InvalidThrowableId(id))
    }

    /// Format Java `Throwable.toString()` using the effective localized message.
    pub fn to_string(&self, id: JavaThrowableId) -> Result<PklString, InvalidThrowableId> {
        let throwable = self.get(id)?;
        if let Some(value) = &throwable.to_string_override {
            return Ok(value.clone());
        }
        let mut out = Vec::new();
        push(&mut out, &throwable.class_name);
        if let Some(message) = &throwable.localized_message {
            ascii(&mut out, ": ");
            push(&mut out, message);
        }
        Ok(PklString::from_utf16(out))
    }

    /// Format `Throwable.printStackTrace`, with the Java println separator supplied
    /// by the runtime profile. Traversal is iterative and uses identity equality.
    pub fn print_stack_trace(
        &self,
        id: JavaThrowableId,
        line_separator: &PklString,
    ) -> Result<PklString, InvalidThrowableId> {
        self.validate()?;
        self.get(id)?;
        let mut out = Vec::new();
        let mut visited = vec![false; self.throwables.len()];
        let mut pending = vec![(id, None, 0usize, "")];
        while let Some((current, parent, indent, caption)) = pending.pop() {
            let item = self.get(current)?;
            tabs(&mut out, indent);
            ascii(&mut out, caption);
            if visited[current.0] {
                ascii(&mut out, "[CIRCULAR REFERENCE: ");
                push(&mut out, &self.to_string(current)?);
                ascii(&mut out, "]");
                push(&mut out, line_separator);
                continue;
            }
            visited[current.0] = true;
            push(&mut out, &self.to_string(current)?);
            push(&mut out, line_separator);
            let common = parent.map_or(0, |parent: JavaThrowableId| {
                item.stack_trace
                    .iter()
                    .rev()
                    .zip(self.throwables[parent.0].stack_trace.iter().rev())
                    .take_while(|(a, b)| a == b)
                    .count()
            });
            for frame in &item.stack_trace[..item.stack_trace.len() - common] {
                tabs(&mut out, indent);
                ascii(&mut out, "\tat ");
                push(&mut out, &frame.render());
                push(&mut out, line_separator);
            }
            if common != 0 {
                tabs(&mut out, indent);
                ascii(&mut out, &format!("\t... {common} more"));
                push(&mut out, line_separator);
            }
            if let Some(cause) = item.cause {
                pending.push((cause, Some(current), indent, "Caused by: "));
            }
            for suppressed in item.suppressed.iter().rev() {
                pending.push((*suppressed, Some(current), indent + 1, "Suppressed: "));
            }
        }
        Ok(PklString::from_utf16(out))
    }

    /// Compose the full Pkl `VmBugException` rendering. The diagnostic must
    /// already contain the ordinary Pkl header, message and trailing newline;
    /// `release_info` is Java's release version string without added newlines.
    pub fn render_pkl_bug(
        &self,
        id: JavaThrowableId,
        ordinary_diagnostic: &PklString,
        release_info: &PklString,
        line_separator: &PklString,
    ) -> Result<PklString, InvalidThrowableId> {
        self.validate()?;
        let item = self.get(id)?;
        let target = item.cause.unwrap_or(id);
        let mut out = Vec::new();
        ascii(
            &mut out,
            "An unexpected error has occurred. Would you mind filing a bug report?\nCmd+Double-click the link below to open an issue.\nPlease copy and paste the entire error output into the issue's description.\n\nhttps://github.com/apple/pkl/issues/new\n\n",
        );
        push(&mut out, &url_encode(&self.to_string(target)?));
        ascii(&mut out, "\n\n");
        push(&mut out, ordinary_diagnostic);
        ascii(&mut out, "\n");
        push(&mut out, release_info);
        ascii(&mut out, "\n\n");
        push(&mut out, &self.print_stack_trace(target, line_separator)?);
        Ok(PklString::from_utf16(out))
    }
}

/// Java UTF-8 form encoding with the Pkl renderer's `+` to `%20` replacement.
pub fn url_encode(input: &PklString) -> PklString {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::new();
    for byte in input.to_java_utf8().bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'*') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[(byte >> 4) as usize]));
            encoded.push(char::from(HEX[(byte & 15) as usize]));
        }
    }
    encoded.into()
}

fn push(out: &mut Vec<u16>, value: &PklString) {
    out.extend(value.encode_utf16());
}
fn ascii(out: &mut Vec<u16>, value: &str) {
    out.extend(value.encode_utf16());
}
fn tabs(out: &mut Vec<u16>, count: usize) {
    out.extend(std::iter::repeat_n(u16::from(b'\t'), count));
}

/// Apply the upstream test module's Java lines/skip/join/strip transformation.
pub fn flatten_caught(rendered: &PklString) -> PklString {
    let units: Vec<_> = rendered.encode_utf16().collect();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < units.len() {
        if matches!(units[index], 10 | 13) {
            lines.push(&units[start..index]);
            if units[index] == 13 && units.get(index + 1) == Some(&10) {
                index += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
    if start < units.len() {
        lines.push(&units[start..]);
    }
    let mut output = Vec::new();
    for (index, line) in lines.into_iter().skip(1).enumerate() {
        if index != 0 {
            output.push(32);
        }
        output.extend_from_slice(line);
    }
    let whitespace = |unit: &u16| {
        char::from_u32(u32::from(*unit)).is_some_and(crate::diagnostic::java_whitespace)
    };
    let start = output.iter().take_while(|unit| whitespace(unit)).count();
    let end = output.len()
        - output[start..]
            .iter()
            .rev()
            .take_while(|unit| whitespace(unit))
            .count();
    PklString::from_utf16(output[start..end].to_vec())
}
