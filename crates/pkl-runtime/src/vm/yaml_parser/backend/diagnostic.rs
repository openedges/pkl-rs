//! Port of SnakeYAML Mark / MarkedYamlEngineException message composition.
use super::nodes::{Text, push_point};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub label: Text,
    pub index: usize,
    pub line: usize,
    pub column: usize,
    pub buffer: Arc<[u32]>,
    pub pointer: usize,
}
fn line_break(cp: u32) -> bool {
    matches!(cp, 0 | 10 | 13)
}
fn append(out: &mut Vec<u16>, s: &str) {
    out.extend(s.encode_utf16());
}
impl Mark {
    pub fn render(&self) -> Text {
        let half = 75f32 / 2.0 - 1.0;
        let mut start = self.pointer;
        let mut head = "";
        while start > 0 && !line_break(self.buffer[start - 1]) {
            start -= 1;
            if (self.pointer - start) as f32 > half {
                head = " ... ";
                start += 5;
                break;
            }
        }
        let mut end = self.pointer;
        let mut tail = "";
        while end < self.buffer.len() && !line_break(self.buffer[end]) {
            end += 1;
            if (end - self.pointer) as f32 > half {
                tail = " ... ";
                end -= 5;
                break;
            }
        }
        let mut out = Vec::new();
        append(&mut out, " in ");
        out.extend(self.label.units());
        append(
            &mut out,
            &format!(
                ", line {}, column {}:\n    {head}",
                self.line + 1,
                self.column + 1
            ),
        );
        for &cp in &self.buffer[start..end] {
            push_point(&mut out, cp);
        }
        append(&mut out, tail);
        append(&mut out, "\n");
        append(&mut out, &" ".repeat(4 + self.pointer - start + head.len()));
        append(&mut out, "^");
        Text::from_units(out)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Reader,
    Scanner,
    Parser,
    Composer,
    Engine,
    Version,
    Foreign,
    Unsupported,
}
#[derive(Clone, Debug)]
pub struct ReaderFailure {
    pub label: Text,
    pub position: usize,
    pub code_point: u32,
}
#[derive(Clone, Debug)]
pub struct YamlError {
    pub java_class: Option<Text>,
    pub reader: Option<ReaderFailure>,
    pub kind: ErrorKind,
    pub context: Option<Text>,
    pub context_mark: Option<Box<Mark>>,
    pub problem: Text,
    pub problem_mark: Option<Box<Mark>>,
    /// Only marked exceptions add context/mark newline framing.
    pub marked: bool,
}
impl YamlError {
    pub fn marked(
        kind: ErrorKind,
        context: Option<&str>,
        context_mark: Option<Mark>,
        problem: impl Into<Text>,
        problem_mark: Option<Mark>,
    ) -> Self {
        Self {
            java_class: java_class(&kind),
            reader: None,
            kind,
            context: context.map(Text::from),
            context_mark: context_mark.map(Box::new),
            problem: problem.into(),
            problem_mark: problem_mark.map(Box::new),
            marked: true,
        }
    }
    pub fn plain(kind: ErrorKind, problem: impl Into<Text>) -> Self {
        Self {
            java_class: java_class(&kind),
            reader: None,
            kind,
            context: None,
            context_mark: None,
            problem: problem.into(),
            problem_mark: None,
            marked: false,
        }
    }
    pub fn foreign(class: &str, message: impl Into<Text>) -> Self {
        let mut error = Self::plain(ErrorKind::Foreign, message);
        error.java_class = Some(Text::from(class));
        error
    }
    pub(crate) fn reader(label: Text, position: usize, code_point: u32, message: &str) -> Self {
        let mut error = Self::plain(ErrorKind::Reader, message);
        error.reader = Some(ReaderFailure {
            label,
            position,
            code_point,
        });
        error
    }
    pub fn java_to_string(&self) -> Option<Text> {
        if self.marked {
            return Some(self.marked_message());
        }
        if let Some(reader) = &self.reader {
            let mut out = "unacceptable code point '"
                .encode_utf16()
                .collect::<Vec<_>>();
            push_point(&mut out, reader.code_point);
            append(&mut out, &format!("' (0x{:X}) ", reader.code_point));
            out.extend(self.problem.units());
            append(&mut out, "\nin \"");
            out.extend(reader.label.units());
            append(&mut out, &format!("\", position {}", reader.position));
            return Some(Text::from_units(out));
        }
        let class = self.java_class.as_ref()?;
        let mut out = class.units().to_vec();
        append(&mut out, ": ");
        out.extend(self.problem.units());
        Some(Text::from_units(out))
    }
    pub fn message(&self) -> Text {
        if self.marked {
            // MarkedYamlEngineException.getMessage() delegates to toString().
            self.java_to_string()
                .expect("marked YAML errors always have a string representation")
        } else {
            // ReaderException.getMessage() remains the bare problem text.
            self.problem.clone()
        }
    }
    fn marked_message(&self) -> Text {
        let mut out = Vec::new();
        if let Some(c) = &self.context {
            out.extend(c.units());
            out.push(10);
        }
        if let Some(c) = &self.context_mark {
            // Deliberately retain the source condition, including equal names.
            if self
                .problem_mark
                .as_ref()
                .is_none_or(|p| c.label == p.label || c.line != p.line || c.column != p.column)
            {
                out.extend(c.render().units());
                out.push(10);
            }
        }
        out.extend(self.problem.units());
        out.push(10);
        if let Some(p) = &self.problem_mark {
            out.extend(p.render().units());
            out.push(10);
        }
        Text::from_units(out)
    }
}

// Pkl 0.32.0 fat-JAR relocation is part of the oracle exception identity.
fn java_class(kind: &ErrorKind) -> Option<Text> {
    let simple = match kind {
        ErrorKind::Reader => "ReaderException",
        ErrorKind::Scanner => "ScannerException",
        ErrorKind::Parser => "ParserException",
        ErrorKind::Composer => "ComposerException",
        ErrorKind::Engine => "YamlEngineException",
        ErrorKind::Version => "YamlVersionException",
        ErrorKind::Foreign | ErrorKind::Unsupported => return None,
    };
    Some(Text::from(format!(
        "org.pkl.thirdparty.snakeyaml.engine.v2.exceptions.{simple}"
    )))
}
