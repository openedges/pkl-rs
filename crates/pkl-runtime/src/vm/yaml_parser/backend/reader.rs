//! UTF-16 StringReader path of Engine 3.0.1 StreamReader.
//! Reader marks snapshot the rolling code-point window, including zero padding.
use super::{Mark, Text, YamlError};
use std::cell::RefCell;
use std::sync::Arc;
#[derive(Clone)]
pub(crate) struct Reader {
    input: Text,
    label: Text,
    input_pos: usize,
    window: Vec<u32>,
    mark_snapshot: RefCell<Option<Arc<[u32]>>>,
    data_length: usize,
    pointer: usize,
    eof: bool,
    pub index: usize,
    pub document_index: usize,
    pub line: usize,
    pub column: usize,
}
impl Reader {
    pub fn new(input: Text, label: Text) -> Self {
        Self {
            input,
            label,
            input_pos: 0,
            window: Vec::new(),
            mark_snapshot: RefCell::new(None),
            data_length: 0,
            pointer: 0,
            eof: false,
            index: 0,
            document_index: 0,
            line: 0,
            column: 0,
        }
    }
    pub fn mark(&self) -> Mark {
        let mut cached = self.mark_snapshot.borrow_mut();
        let buffer = cached
            .get_or_insert_with(|| Arc::from(self.window.as_slice()))
            .clone();
        Mark {
            label: self.label.clone(),
            index: self.index,
            line: self.line,
            column: self.column,
            buffer,
            pointer: self.pointer,
        }
    }
    fn update(&mut self) -> Result<(), YamlError> {
        let mut read = (self.input.units().len() - self.input_pos).min(1025);
        if read == 0 {
            self.eof = true;
            return Ok(());
        }
        let remaining = self.data_length - self.pointer;
        if (0xd800..=0xdbff).contains(&self.input.units()[self.input_pos + read - 1]) {
            // StringReader checks the destination range before EOF. Engine's
            // bufferSize+1 read can fill all 1025 slots, making this extension
            // invalid even when the high surrogate is the final input unit.
            // Actual pinned-JDK graph controls cover both EOF and a next unit.
            if read == 1025 {
                return Err(YamlError::foreign(
                    "java.lang.IndexOutOfBoundsException",
                    "Range [1025, 1025 + 1) out of bounds for length 1025",
                ));
            }
            if self.input_pos + read == self.input.units().len() {
                return Err(YamlError::reader(
                    self.label.clone(),
                    self.index + read,
                    u32::from(self.input.units()[self.input_pos + read - 1]),
                    "The last char is HighSurrogate (no LowSurrogate detected).",
                ));
            }
            read += 1;
        }
        if self.pointer > 0 {
            self.window.copy_within(self.pointer..self.data_length, 0);
        }
        self.window.truncate(remaining);
        self.window.resize(remaining + read, 0);
        let mut i = 0;
        let mut cp_index = remaining;
        while i < read {
            let a = self.input.units()[self.input_pos + i];
            let cp = if (0xd800..=0xdbff).contains(&a)
                && i + 1 < read
                && (0xdc00..=0xdfff).contains(&self.input.units()[self.input_pos + i + 1])
            {
                let b = self.input.units()[self.input_pos + i + 1];
                i += 1;
                0x10000 + ((u32::from(a) - 0xd800) << 10) + u32::from(b) - 0xdc00
            } else {
                u32::from(a)
            };
            if !printable(cp) {
                return Err(YamlError::reader(
                    self.label.clone(),
                    self.index + cp_index,
                    cp,
                    "special characters are not allowed",
                ));
            }
            if cp_index == self.window.len() {
                self.window.push(cp);
            } else {
                self.window[cp_index] = cp;
            }
            cp_index += 1;
            i += 1;
        }
        self.input_pos += read;
        *self.mark_snapshot.borrow_mut() = None;
        self.data_length = cp_index;
        self.pointer = 0;
        Ok(())
    }
    fn enough(&mut self, ahead: usize) -> Result<bool, YamlError> {
        if !self.eof && self.pointer + ahead >= self.data_length {
            self.update()?;
        }
        Ok(self.pointer + ahead < self.data_length)
    }
    pub fn peek(&mut self, ahead: usize) -> Result<u32, YamlError> {
        Ok(if self.enough(ahead)? {
            self.window[self.pointer + ahead]
        } else {
            0
        })
    }
    pub fn forward(&mut self, count: usize) -> Result<(), YamlError> {
        for _ in 0..count {
            if !self.enough(0)? {
                break;
            }
            let cp = self.window[self.pointer];
            self.pointer += 1;
            self.index += 1;
            self.document_index += 1;
            if cp == 10 || (cp == 13 && self.enough(0)? && self.window[self.pointer] != 10) {
                self.line += 1;
                self.column = 0;
            } else if cp != 0xfeff {
                self.column += 1;
            }
        }
        Ok(())
    }
    pub fn prefix(&mut self, len: usize) -> Result<Text, YamlError> {
        if len == 0 {
            return Ok(Text::default());
        }
        self.enough(len)?;
        Ok(Text::from_points(
            &self.window[self.pointer..self.pointer + len.min(self.data_length - self.pointer)],
        ))
    }
    pub fn prefix_forward(&mut self, len: usize) -> Result<Text, YamlError> {
        let result = self.prefix(len)?;
        self.pointer += len;
        self.index += len;
        self.document_index += len;
        self.column += len;
        Ok(result)
    }
}
fn printable(c: u32) -> bool {
    (0x20..=0x7e).contains(&c)
        || matches!(c, 9 | 10 | 13 | 0x85)
        || (0xa0..=0xd7ff).contains(&c)
        || (0xe000..=0xfffd).contains(&c)
        || (0x10000..=0x10ffff).contains(&c)
}
