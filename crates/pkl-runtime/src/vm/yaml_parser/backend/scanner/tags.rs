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
        let mut value = 0i32;
        while let Some(d) = decimal_digit(self.reader.peek(n)?)? {
            if n < 3 {
                value = value * 10 + d as i32;
            }
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
        Ok(value)
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
            let code = match parse_java_hex_i32(hex.units()) {
                Some(v) => v,
                None => {
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
        match String::from_utf8(bytes.clone()) {
            Ok(s) => Ok(Text::from(s)),
            Err(_) => {
                let bad_len = java_utf8_malformed_length(&bytes);
                Err(YamlError::marked(
                    ErrorKind::Scanner,
                    Some(&format!("while scanning a {name}")),
                    Some(start.clone()),
                    format!("expected URI in UTF-8: Input length = {bad_len}"),
                    Some(beginning),
                ))
            }
        }
    }
}

/// Exact port of `Integer.parseInt(hex, 16)` on a 2-unit (or 1-unit at EOF) slice.
fn parse_java_hex_i32(units: &[u16]) -> Option<i32> {
    if units.is_empty() {
        return None;
    }
    let mut i = 0;
    let negative = match units[0] {
        0x2d => {
            i = 1;
            true
        }
        0x2b => {
            i = 1;
            false
        }
        _ => false,
    };
    if i >= units.len() {
        return None;
    }
    let mut acc: i32 = 0;
    while i < units.len() {
        let u = units[i];
        let cp = if (0xd800..=0xdbff).contains(&u)
            && i + 1 < units.len()
            && (0xdc00..=0xdfff).contains(&units[i + 1])
        {
            i += 1;
            0x10000 + ((u32::from(u) - 0xd800) << 10) + (u32::from(units[i]) - 0xdc00)
        } else {
            u32::from(u)
        };
        let digit = match cp {
            0x30..=0x39 => cp - 0x30,
            0x41..=0x46 => cp - 0x41 + 10,
            0x61..=0x66 => cp - 0x61 + 10,
            0xff21..=0xff26 => cp - 0xff21 + 10,
            0xff41..=0xff46 => cp - 0xff41 + 10,
            _ => super::scalar::decimal_digit(cp).ok().flatten()?,
        };
        acc = acc.checked_mul(16)?.checked_add(digit as i32)?;
        i += 1;
    }
    Some(if negative { acc.wrapping_neg() } else { acc })
}

/// Exact JDK 21 `sun.nio.cs.UTF_8$Decoder` `MalformedInputException.getInputLength()`.
/// Note that JDK 21 reports length 3 for 3-byte UTF-8 encoded surrogates (`ED A0..BF 80..BF`).
fn java_utf8_malformed_length(bytes: &[u8]) -> usize {
    let mut i = 0;
    let n = bytes.len();
    while i < n {
        let b1 = bytes[i];
        if b1 < 0x80 {
            i += 1;
        } else if (b1 >> 5) == 0b110 {
            if (b1 & 0x1e) == 0 || i + 1 >= n || (bytes[i + 1] & 0xc0) != 0x80 {
                return 1;
            }
            i += 2;
        } else if (b1 >> 4) == 0b1110 {
            if i + 1 >= n {
                return 1;
            }
            let b2 = bytes[i + 1];
            if (b1 == 0xe0 && (b2 & 0xe0) == 0x80) || (b2 & 0xc0) != 0x80 {
                return 1;
            }
            if i + 2 >= n || (bytes[i + 2] & 0xc0) != 0x80 {
                return 2;
            }
            let c = (u32::from(b1 & 0x0f) << 12)
                | (u32::from(b2 & 0x3f) << 6)
                | u32::from(bytes[i + 2] & 0x3f);
            if (0xd800..=0xdfff).contains(&c) {
                return 3;
            }
            i += 3;
        } else if (b1 >> 3) == 0b11110 {
            if i + 1 >= n {
                return 1;
            }
            let b2 = bytes[i + 1];
            if (b2 & 0xc0) != 0x80
                || (b1 == 0xf0 && b2 < 0x90)
                || (b1 == 0xf4 && b2 >= 0x90)
                || b1 > 0xf4
            {
                return 1;
            }
            if i + 2 >= n || (bytes[i + 2] & 0xc0) != 0x80 {
                return 2;
            }
            if i + 3 >= n || (bytes[i + 3] & 0xc0) != 0x80 {
                return 3;
            }
            i += 4;
        } else {
            return 1;
        }
    }
    1
}
