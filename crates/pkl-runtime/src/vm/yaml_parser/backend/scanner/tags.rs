// Copyright (c) 2018, SnakeYAML. Apache-2.0. See PROVENANCE.md.
use super::scalar::decimal_digit;
use super::*;
fn alpha(c: u32) -> bool {
    matches!(c,48..=57|65..=90|97..=122|45|95)
}
fn uri_char(c: u32, prefix: bool) -> bool {
    alpha(c) || contains("-;/?:@&=+$_.!~*'()%", c) || (prefix && contains(",[]", c))
}
impl Scanner {
    pub(super) fn anchor(&mut self, is_anchor: bool) -> Result<Token, YamlError> {
        let start = self.reader.mark();
        let name = if is_anchor { "anchor" } else { "alias" };
        let context = format!("while scanning an {name}");
        self.reader.forward(1)?;
        let mut n = 0;
        let mut c = self.reader.peek(n)?;
        while !blank(c) && !contains(",[]{}/.*&", c) {
            n += 1;
            c = self.reader.peek(n)?;
        }
        if n == 0 {
            return Err(self.error(
                Some(&context),
                Some(start),
                format!("unexpected character found {}({c})", point(c)),
            ));
        }
        let value = self.reader.prefix_forward(n)?;
        c = self.reader.peek(0)?;
        if !blank(c) && !contains("?:,]}%@`", c) {
            return Err(self.error(
                Some(&context),
                Some(start),
                format!("unexpected character found {}({c})", point(c)),
            ));
        }
        Ok(Token {
            id: if is_anchor { Id::Anchor } else { Id::Alias },
            start,
            end: self.reader.mark(),
            data: Data::Name(value),
        })
    }
    pub(super) fn directive(&mut self) -> Result<Token, YamlError> {
        let start = self.reader.mark();
        self.reader.forward(1)?;
        let mut n = 0;
        while alpha(self.reader.peek(n)?) {
            n += 1;
        }
        let c = self.reader.peek(n)?;
        if n == 0 {
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!(
                    "expected alphabetic or numeric character, but found {}({c})",
                    point(c)
                ),
            ));
        }
        let name = self.reader.prefix_forward(n)?;
        let c = self.reader.peek(0)?;
        if c != 32 && !nul_line(c) {
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start),
                format!(
                    "expected alphabetic or numeric character, but found {}({c})",
                    point(c)
                ),
            ));
        }
        let data;
        let end;
        if name.is_ascii("YAML") {
            while self.reader.peek(0)? == 32 {
                self.reader.forward(1)?;
            }
            let major = self.version_number(&start)?;
            let c = self.reader.peek(0)?;
            if c != 46 {
                return Err(self.error(
                    Some("while scanning a directive"),
                    Some(start),
                    format!("expected a digit or '.', but found {}({c})", point(c)),
                ));
            }
            self.reader.forward(1)?;
            let minor = self.version_number(&start)?;
            let c = self.reader.peek(0)?;
            if c != 32 && !nul_line(c) {
                return Err(self.error(
                    Some("while scanning a directive"),
                    Some(start),
                    format!("expected a digit or ' ', but found {}({c})", point(c)),
                ));
            }
            data = Some(Directive::Yaml(major, minor));
            end = self.reader.mark();
        } else if name.is_ascii("TAG") {
            while self.reader.peek(0)? == 32 {
                self.reader.forward(1)?;
            }
            let handle = self.tag_handle("directive", &start)?;
            let c = self.reader.peek(0)?;
            if c != 32 {
                return Err(self.error(
                    Some("while scanning a directive"),
                    Some(start),
                    format!("expected ' ', but found {}({c})", point(c)),
                ));
            }
            while self.reader.peek(0)? == 32 {
                self.reader.forward(1)?;
            }
            let prefix = self.tag_uri("directive", true, &start)?;
            let c = self.reader.peek(0)?;
            if c != 32 && !nul_line(c) {
                return Err(self.error(
                    Some("while scanning a directive"),
                    Some(start),
                    format!("expected ' ', but found {}({c})", point(c)),
                ));
            }
            data = Some(Directive::Tag(handle, prefix));
            end = self.reader.mark();
        } else {
            end = self.reader.mark();
            let mut n = 0;
            while !nul_line(self.reader.peek(n)?) {
                n += 1;
            }
            self.reader.forward(n)?;
            data = None;
        }
        self.ignored_line(&start, "while scanning a directive")?;
        Ok(Token {
            id: Id::Directive,
            start,
            end,
            data: Data::Directive(name, data),
        })
    }
    fn version_number(&mut self, start: &Mark) -> Result<i32, YamlError> {
        let c = self.reader.peek(0)?;
        if decimal_digit(c)?.is_none() {
            return Err(self.error(
                Some("while scanning a directive"),
                Some(start.clone()),
                format!("expected a digit, but found {}({c})", point(c)),
            ));
        }
        let mut n = 0;
        while decimal_digit(self.reader.peek(n)?)?.is_some() {
            n += 1;
        }
        let number = self.reader.prefix_forward(n)?;
        if n > 3 {
            return Err(self.error(
                Some("while scanning a YAML directive"),
                Some(start.clone()),
                format!(
                    "found a number which cannot represent a valid version: {}",
                    text_string(&number)
                ),
            ));
        }
        Ok(text_string(&number)
            .parse()
            .expect("ASCII directive digits"))
    }
    pub(super) fn tag(&mut self) -> Result<Token, YamlError> {
        let start = self.reader.mark();
        let c = self.reader.peek(1)?;
        let handle;
        let suffix;
        if c == 60 {
            self.reader.forward(2)?;
            suffix = self.tag_uri("tag", true, &start)?;
            let c = self.reader.peek(0)?;
            if c != 62 {
                return Err(self.error(
                    Some("while scanning a tag"),
                    Some(start),
                    format!("expected '>', but found '{}' ({c})", point(c)),
                ));
            }
            self.reader.forward(1)?;
            handle = None;
        } else if blank(c) {
            suffix = Text::from("!");
            self.reader.forward(1)?;
            handle = None;
        } else {
            let mut n = 1;
            let mut c = c;
            let mut use_handle = false;
            while c != 32 && !nul_line(c) {
                if c == 33 {
                    use_handle = true;
                    break;
                }
                n += 1;
                c = self.reader.peek(n)?;
            }
            handle = Some(if use_handle {
                self.tag_handle("tag", &start)?
            } else {
                self.reader.forward(1)?;
                Text::from("!")
            });
            suffix = self.tag_uri("tag", false, &start)?;
        }
        let c = self.reader.peek(0)?;
        if c != 32 && !nul_line(c) {
            return Err(self.error(
                Some("while scanning a tag"),
                Some(start),
                format!("expected ' ', but found '{}' ({c})", point(c)),
            ));
        }
        Ok(Token {
            id: Id::Tag,
            start,
            end: self.reader.mark(),
            data: Data::Tag(handle, suffix),
        })
    }
    fn tag_handle(&mut self, name: &str, start: &Mark) -> Result<Text, YamlError> {
        let context = format!("while scanning a {name}");
        let c = self.reader.peek(0)?;
        if c != 33 {
            return Err(self.error(
                Some(&context),
                Some(start.clone()),
                format!("expected '!', but found {}({c})", point(c)),
            ));
        }
        let mut n = 1;
        let mut c = self.reader.peek(n)?;
        if c != 32 {
            while alpha(c) {
                n += 1;
                c = self.reader.peek(n)?;
            }
            if c != 33 {
                self.reader.forward(n)?;
                return Err(self.error(
                    Some(&context),
                    Some(start.clone()),
                    format!("expected '!', but found {}({c})", point(c)),
                ));
            }
            n += 1;
        }
        self.reader.prefix_forward(n)
    }
    fn tag_uri(&mut self, name: &str, prefix: bool, start: &Mark) -> Result<Text, YamlError> {
        let mut chunks = Vec::new();
        let mut n = 0;
        let mut c = self.reader.peek(n)?;
        while uri_char(c, prefix) {
            if c == 37 {
                chunks.extend(self.reader.prefix_forward(n)?.units());
                n = 0;
                chunks.extend(self.uri_escapes(name, start)?.units());
            } else {
                n += 1;
            }
            c = self.reader.peek(n)?;
        }
        if n > 0 {
            chunks.extend(self.reader.prefix_forward(n)?.units());
        }
        if chunks.is_empty() {
            return Err(self.error(
                Some(&format!("while scanning a {name}")),
                Some(start.clone()),
                format!("expected URI, but found {}({c})", point(c)),
            ));
        }
        Ok(Text::from_units(chunks))
    }
    fn uri_escapes(&mut self, name: &str, start: &Mark) -> Result<Text, YamlError> {
        let mut length = 1;
        while self.reader.peek(length * 3)? == 37 {
            length += 1;
        }
        let beginning = self.reader.mark();
        let mut bytes = Vec::with_capacity(length);
        while self.reader.peek(0)? == 37 {
            self.reader.forward(1)?;
            let hex = self.reader.prefix(2)?;
            if hex.units().iter().any(|u| *u > 127) {
                return Err(YamlError::plain(
                    ErrorKind::Unsupported,
                    "Pinned Java Unicode digit handling in URI escapes is not ported",
                ));
            }
            let parsed = i32::from_str_radix(&text_string(&hex), 16);
            let code = match parsed {
                Ok(v) => v,
                Err(_) => {
                    let c1 = self.reader.peek(0)?;
                    let c2 = self.reader.peek(1)?;
                    return Err(self.error(Some(&format!("while scanning a {name}")),Some(start.clone()),format!("expected URI escape sequence of 2 hexadecimal numbers, but found {}({c1}) and {}({c2})",point(c1),point(c2))));
                }
            };
            if bytes.len() == length {
                return Err(YamlError::plain(
                    ErrorKind::Unsupported,
                    "Exact Java BufferOverflowException in malformed tag URI escape is not ported",
                ));
            }
            bytes.push(code as u8);
            self.reader.forward(2)?;
        }
        match String::from_utf8(bytes) {
            Ok(s) => Ok(Text::from(s)),
            Err(_) => {
                // Java UTF-8 decoder reports a specific malformed subsequence length.
                // Rust's Utf8Error length is not assumed equivalent.
                Err(YamlError::marked(
                    ErrorKind::Unsupported,
                    Some(&format!("while scanning a {name}")),
                    Some(start.clone()),
                    "Exact Java UTF-8 tag URI decoding error length is not ported",
                    Some(beginning),
                ))
            }
        }
    }
}
