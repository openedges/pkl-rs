//! Resumable UTF-16 JSON event reader following Pkl 0.32 JsonParser.
//! Values are completed before the following separator is inspected, so VM
//! converters can run (and fail) at the same point as Java's JsonHandler.
use crate::value::PklString;

pub(super) enum Scalar {
    Null,
    Boolean(bool),
    String(PklString),
    Number(String),
}
pub(super) enum Event {
    StartArray,
    EndArray,
    StartObject,
    EndObject,
    Key(PklString),
    Value(Scalar),
    EndDocument,
}
#[derive(Clone, Copy)]
enum Task {
    Value,
    EndDocument,
    ArrayFirst,
    ArrayValue,
    ArrayAfter,
    ObjectFirst,
    ObjectName,
    ObjectAfter,
}
pub(super) struct Reader {
    units: Vec<u16>,
    cursor: usize,
    line: usize,
    line_start: usize,
    depth: usize,
    tasks: Vec<Task>,
}
impl Reader {
    pub(super) fn new(text: &PklString) -> Self {
        Self {
            units: text.encode_utf16().collect(),
            cursor: 0,
            line: 1,
            line_start: 0,
            depth: 0,
            tasks: vec![Task::EndDocument, Task::Value],
        }
    }
    fn current(&self) -> Option<u16> {
        self.units.get(self.cursor).copied()
    }
    fn advance(&mut self) {
        let previous = self.current();
        if previous.is_some() {
            self.cursor += 1;
            // Java read() returns on EOF before updating a consumed LF's
            // line counter. Preserve that observable final-newline location.
            if self.cursor < self.units.len() && previous == Some(10) {
                self.line += 1;
                self.line_start = self.cursor;
            }
        }
    }
    fn take(&mut self, unit: u16) -> bool {
        if self.current() == Some(unit) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn whitespace(&mut self) {
        while matches!(self.current(), Some(9 | 10 | 13 | 32)) {
            self.advance();
        }
    }
    pub(super) fn error(&self, message: &str) -> PklString {
        format!(
            "{message} at {}:{}",
            self.line,
            self.cursor - self.line_start + 1
        )
        .into()
    }
    fn expected(&self, expected: &str) -> PklString {
        self.error(&if self.current().is_none() {
            "Unexpected end of input".to_owned()
        } else {
            format!("Expected {expected}")
        })
    }
    fn digit(&mut self) -> bool {
        if matches!(self.current(), Some(48..=57)) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Result<PklString, PklString> {
        self.advance(); // opening quote was established by the caller
        let mut result = Vec::new();
        loop {
            match self.current() {
                Some(34) => {
                    self.advance();
                    return Ok(PklString::from_utf16(result));
                }
                Some(92) => {
                    self.advance();
                    let unit = match self.current() {
                        Some(unit @ (34 | 47 | 92)) => unit,
                        Some(98) => 8,
                        Some(102) => 12,
                        Some(110) => 10,
                        Some(114) => 13,
                        Some(116) => 9,
                        Some(117) => {
                            let mut value = 0;
                            for _ in 0..4 {
                                self.advance();
                                let digit = match self.current() {
                                    Some(unit @ 48..=57) => unit - 48,
                                    Some(unit @ 65..=70) => unit - 65 + 10,
                                    Some(unit @ 97..=102) => unit - 97 + 10,
                                    _ => return Err(self.expected("hexadecimal digit")),
                                };
                                value = value * 16 + digit;
                            }
                            value
                        }
                        _ => return Err(self.expected("valid escape sequence")),
                    };
                    result.push(unit);
                    self.advance();
                }
                Some(unit) if unit >= 0x20 => {
                    result.push(unit);
                    self.advance();
                }
                _ => return Err(self.expected("valid string character")),
            }
        }
    }
    fn number(&mut self) -> Result<String, PklString> {
        let start = self.cursor;
        self.take(45);
        let first = self.current();
        if !self.digit() {
            return Err(self.expected("digit"));
        }
        if first != Some(48) {
            while self.digit() {}
        }
        if self.take(46) {
            if !self.digit() {
                return Err(self.expected("digit"));
            }
            while self.digit() {}
        }
        if self.take(101) || self.take(69) {
            if !self.take(43) {
                self.take(45);
            }
            if !self.digit() {
                return Err(self.expected("digit"));
            }
            while self.digit() {}
        }
        // Every accepted unit in this span is an ASCII number token.
        Ok(self.units[start..self.cursor]
            .iter()
            .map(|unit| char::from(*unit as u8))
            .collect())
    }
    fn literal(&mut self, text: &str) -> Result<(), PklString> {
        for unit in text.bytes() {
            if !self.take(u16::from(unit)) {
                return Err(self.expected(&format!("'{}'", char::from(unit))));
            }
        }
        Ok(())
    }
    pub(super) fn next(&mut self) -> Result<Event, PklString> {
        loop {
            match self.tasks.pop().unwrap_or(Task::EndDocument) {
                Task::Value => {
                    self.whitespace();
                    let scalar = match self.current() {
                        Some(110) => {
                            self.literal("null")?;
                            Scalar::Null
                        }
                        Some(116) => {
                            self.literal("true")?;
                            Scalar::Boolean(true)
                        }
                        Some(102) => {
                            self.literal("false")?;
                            Scalar::Boolean(false)
                        }
                        Some(34) => Scalar::String(self.string()?),
                        Some(45 | 48..=57) => Scalar::Number(self.number()?),
                        Some(unit @ (91 | 123)) => {
                            self.advance();
                            self.depth += 1;
                            if self.depth > 1000 {
                                return Err(self.error("Nesting too deep"));
                            }
                            self.tasks.push(if unit == 91 {
                                Task::ArrayFirst
                            } else {
                                Task::ObjectFirst
                            });
                            return Ok(if unit == 91 {
                                Event::StartArray
                            } else {
                                Event::StartObject
                            });
                        }
                        _ => return Err(self.expected("value")),
                    };
                    return Ok(Event::Value(scalar));
                }
                Task::ArrayFirst => {
                    self.whitespace();
                    if self.take(93) {
                        self.depth -= 1;
                        return Ok(Event::EndArray);
                    }
                    self.tasks.push(Task::ArrayAfter);
                    self.tasks.push(Task::Value);
                }
                Task::ArrayValue => {
                    self.tasks.push(Task::ArrayAfter);
                    self.tasks.push(Task::Value);
                }
                Task::ArrayAfter => {
                    self.whitespace();
                    if self.take(44) {
                        self.tasks.push(Task::ArrayValue);
                    } else if self.take(93) {
                        self.depth -= 1;
                        return Ok(Event::EndArray);
                    } else {
                        return Err(self.expected("',' or ']'"));
                    }
                }
                Task::ObjectFirst => {
                    self.whitespace();
                    if self.take(125) {
                        self.depth -= 1;
                        return Ok(Event::EndObject);
                    }
                    self.tasks.push(Task::ObjectName);
                }
                Task::ObjectName => {
                    self.whitespace();
                    if self.current() != Some(34) {
                        return Err(self.expected("name"));
                    }
                    let name = self.string()?;
                    self.whitespace();
                    if !self.take(58) {
                        return Err(self.expected("':'"));
                    }
                    self.whitespace();
                    self.tasks.push(Task::ObjectAfter);
                    self.tasks.push(Task::Value);
                    return Ok(Event::Key(name));
                }
                Task::ObjectAfter => {
                    self.whitespace();
                    if self.take(44) {
                        self.tasks.push(Task::ObjectName);
                    } else if self.take(125) {
                        self.depth -= 1;
                        return Ok(Event::EndObject);
                    } else {
                        return Err(self.expected("',' or '}'"));
                    }
                }
                Task::EndDocument => {
                    self.whitespace();
                    if self.current().is_some() {
                        return Err(self.error("Unexpected character"));
                    }
                    return Ok(Event::EndDocument);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_limit_is_iterative_and_matches_the_source_boundary() {
        for depth in [1000, 1001] {
            let text = PklString::from(format!("{}0{}", "[".repeat(depth), "]".repeat(depth)));
            let mut reader = Reader::new(&text);
            let mut opens = 0;
            loop {
                match reader.next() {
                    Ok(Event::StartArray) => opens += 1,
                    Ok(Event::EndDocument) => {
                        assert_eq!(depth, 1000);
                        assert_eq!(opens, depth);
                        break;
                    }
                    Ok(_) => {}
                    Err(hint) => {
                        assert_eq!(depth, 1001);
                        assert_eq!(opens, 1000);
                        assert_eq!(hint, "Nesting too deep at 1:1002");
                        break;
                    }
                }
            }
        }
    }
}
