// Copyright (c) 2018, SnakeYAML. Apache-2.0. See PROVENANCE.md.
use super::super::nodes::push_point;
use super::*;
impl Scanner {
    pub(super) fn flow_scalar(&mut self, style: ScalarStyle) -> Result<Token, YamlError> {
        let start = self.reader.mark();
        let quote = self.reader.peek(0)?;
        self.reader.forward(1)?;
        let mut chunks = Vec::new();
        self.flow_nonspaces(style == ScalarStyle::DoubleQuoted, &start, &mut chunks)?;
        while self.reader.peek(0)? != quote {
            self.flow_spaces(&start, &mut chunks)?;
            self.flow_nonspaces(style == ScalarStyle::DoubleQuoted, &start, &mut chunks)?;
        }
        self.reader.forward(1)?;
        Ok(Token::scalar(chunks, style, start, self.reader.mark()))
    }
    fn flow_nonspaces(
        &mut self,
        double: bool,
        start: &Mark,
        chunks: &mut Vec<u16>,
    ) -> Result<(), YamlError> {
        loop {
            let mut n = 0;
            loop {
                let c = self.reader.peek(n)?;
                if blank(c) || contains("'\"\\", c) {
                    break;
                }
                n += 1;
            }
            if n > 0 {
                chunks.extend(self.reader.prefix_forward(n)?.units());
            }
            let c = self.reader.peek(0)?;
            if !double && c == 39 && self.reader.peek(1)? == 39 {
                chunks.push(39);
                self.reader.forward(2)?;
            } else if (double && c == 39) || (!double && contains("\"\\", c)) {
                push_point(chunks, c);
                self.reader.forward(1)?;
            } else if double && c == 92 {
                self.reader.forward(1)?;
                let c = self.reader.peek(0)?;
                let replacement = match c {
                    48 => Some(0),
                    97 => Some(7),
                    98 => Some(8),
                    116 => Some(9),
                    110 => Some(10),
                    118 => Some(11),
                    102 => Some(12),
                    114 => Some(13),
                    101 => Some(27),
                    32 => Some(32),
                    34 => Some(34),
                    47 => Some(47),
                    92 => Some(92),
                    78 => Some(0x85),
                    95 => Some(0xa0),
                    _ => None,
                };
                if let Some(u) = replacement {
                    chunks.push(u);
                    self.reader.forward(1)?;
                } else if matches!(c, 120 | 117 | 85) {
                    let len = match c {
                        120 => 2,
                        117 => 4,
                        _ => 8,
                    };
                    self.reader.forward(1)?;
                    let hex = self.reader.prefix(len)?;
                    if hex.units().len() != len
                        || !hex
                            .units()
                            .iter()
                            .all(|&u| matches!(u,48..=57|65..=70|97..=102))
                    {
                        return Err(self.error(Some("while scanning a double-quoted scalar"),Some(start.clone()),format!("expected escape sequence of {len} hexadecimal numbers, but found: {}",text_string(&hex))));
                    }
                    let s = text_string(&hex);
                    let decimal = i32::from_str_radix(&s, 16).map_err(|_| {
                        YamlError::foreign(
                            "java.lang.NumberFormatException",
                            format!("For input string: \"{s}\" under radix 16"),
                        )
                    })?;
                    if decimal > 0x10ffff {
                        return Err(self.error(
                            Some("while scanning a double-quoted scalar"),
                            Some(start.clone()),
                            format!("found unknown escape character {s}"),
                        ));
                    }
                    push_point(chunks, decimal as u32);
                    self.reader.forward(len)?;
                } else if c == 9 {
                    chunks.push(9);
                    self.reader.forward(1)?;
                } else if self.line_break()?.is_some() {
                    chunks.extend(self.flow_breaks(start)?);
                } else {
                    return Err(self.error(
                        Some("while scanning a double-quoted scalar"),
                        Some(start.clone()),
                        format!("found unknown escape character {}({c})", point(c)),
                    ));
                }
            } else {
                return Ok(());
            }
        }
    }
    fn flow_spaces(&mut self, start: &Mark, chunks: &mut Vec<u16>) -> Result<(), YamlError> {
        let mut n = 0;
        while matches!(self.reader.peek(n)?, 32 | 9) {
            n += 1;
        }
        let spaces = self.reader.prefix_forward(n)?;
        if self.reader.peek(0)? == 0 {
            return Err(self.error(
                Some("while scanning a quoted scalar"),
                Some(start.clone()),
                "found unexpected end of stream",
            ));
        }
        if self.line_break()?.is_some() {
            let breaks = self.flow_breaks(start)?;
            if breaks.is_empty() {
                chunks.push(32);
            }
            chunks.extend(breaks);
        } else {
            chunks.extend(spaces.units());
        }
        Ok(())
    }
    fn flow_breaks(&mut self, start: &Mark) -> Result<Vec<u16>, YamlError> {
        let mut chunks = Vec::new();
        loop {
            let prefix = self.reader.prefix(3)?;
            if (prefix.is_ascii("---") || prefix.is_ascii("...")) && blank(self.reader.peek(3)?) {
                return Err(self.error(
                    Some("while scanning a quoted scalar"),
                    Some(start.clone()),
                    "found unexpected document separator",
                ));
            }
            while matches!(self.reader.peek(0)?, 32 | 9) {
                self.reader.forward(1)?;
            }
            if let Some(lb) = self.line_break()? {
                chunks.push(lb);
            } else {
                return Ok(chunks);
            }
        }
    }
    pub(super) fn plain(&mut self) -> Result<Token, YamlError> {
        let mut chunks = Vec::new();
        let start = self.reader.mark();
        let mut end = start.clone();
        let plain_indent = self.indent + 1;
        let mut spaces = Vec::new();
        loop {
            if self.reader.peek(0)? == 35 {
                break;
            }
            let mut n = 0;
            loop {
                let c = self.reader.peek(n)?;
                if blank(c) {
                    break;
                }
                if c == 58 {
                    let following = self.reader.peek(n + 1)?;
                    if blank(following) || (self.flow != 0 && contains(",[]{}", following)) {
                        break;
                    }
                }
                if self.flow != 0 && contains(",[]{}", c) {
                    break;
                }
                n += 1;
            }
            if n == 0 {
                break;
            }
            self.allow_key = false;
            chunks.extend(&spaces);
            chunks.extend(self.reader.prefix_forward(n)?.units());
            end = self.reader.mark();
            spaces = self.plain_spaces()?;
            if spaces.is_empty()
                || self.reader.peek(0)? == 35
                || (self.flow == 0 && (self.reader.column as i32) < plain_indent)
            {
                break;
            }
        }
        Ok(Token::scalar(chunks, ScalarStyle::Plain, start, end))
    }
    fn plain_spaces(&mut self) -> Result<Vec<u16>, YamlError> {
        let mut n = 0;
        while matches!(self.reader.peek(n)?, 32 | 9) {
            n += 1;
        }
        let spaces = self.reader.prefix_forward(n)?;
        if self.line_break()?.is_none() {
            return Ok(spaces.units().to_vec());
        }
        self.allow_key = true;
        let prefix = self.reader.prefix(3)?;
        if prefix.is_ascii("---") || (prefix.is_ascii("...") && blank(self.reader.peek(3)?)) {
            return Ok(Vec::new());
        }
        let mut breaks = Vec::new();
        loop {
            if self.reader.peek(0)? == 32 {
                self.reader.forward(1)?;
            } else if let Some(lb) = self.line_break()? {
                breaks.push(lb);
                let prefix = self.reader.prefix(3)?;
                if prefix.is_ascii("---") || (prefix.is_ascii("...") && blank(self.reader.peek(3)?))
                {
                    return Ok(Vec::new());
                }
            } else {
                break;
            }
        }
        if breaks.is_empty() {
            Ok(vec![32])
        } else {
            Ok(breaks)
        }
    }
    pub(super) fn block_scalar(&mut self, style: ScalarStyle) -> Result<Token, YamlError> {
        let start = self.reader.mark();
        self.reader.forward(1)?;
        let (indicator, increment) = self.block_indicators(&start)?;
        self.ignored_line(&start, "while scanning a block scalar")?;
        let min_indent = (self.indent + 1).max(1) as usize;
        let (mut breaks, indent, mut end) = if let Some(incr) = increment {
            let indent = min_indent + incr - 1;
            let (b, e) = self.block_breaks(indent)?;
            (b, indent, e)
        } else {
            let (b, i, e) = self.block_indentation()?;
            (b, min_indent.max(i), e)
        };
        let mut chunks = Vec::new();
        let mut last_break = None;
        while self.reader.column == indent && self.reader.peek(0)? != 0 {
            chunks.extend(&breaks);
            let leading = !matches!(self.reader.peek(0)?, 32 | 9);
            let mut n = 0;
            while !nul_line(self.reader.peek(n)?) {
                n += 1;
            }
            chunks.extend(self.reader.prefix_forward(n)?.units());
            last_break = self.line_break()?;
            (breaks, end) = self.block_breaks(indent)?;
            if self.reader.column == indent && self.reader.peek(0)? != 0 {
                if style == ScalarStyle::Folded
                    && last_break == Some(10)
                    && leading
                    && !matches!(self.reader.peek(0)?, 32 | 9)
                {
                    if breaks.is_empty() {
                        chunks.push(32);
                    }
                } else if let Some(lb) = last_break {
                    chunks.push(lb);
                }
            } else {
                break;
            }
        }
        if indicator != Some(45)
            && let Some(lb) = last_break
        {
            chunks.push(lb);
        }
        if indicator == Some(43) {
            chunks.extend(breaks);
        }
        Ok(Token::scalar(chunks, style, start, end))
    }
    fn block_indicators(
        &mut self,
        start: &Mark,
    ) -> Result<(Option<u32>, Option<usize>), YamlError> {
        let mut indicator = None;
        let mut increment = None;
        let mut c = self.reader.peek(0)?;
        if matches!(c, 45 | 43) {
            indicator = Some(c);
            self.reader.forward(1)?;
            c = self.reader.peek(0)?;
            if let Some(n) = decimal_digit(c)? {
                increment = Some(n as usize);
                if n == 0 {
                    return Err(self.error(
                        Some("while scanning a block scalar"),
                        Some(start.clone()),
                        "expected indentation indicator in the range 1-9, but found 0",
                    ));
                }
                self.reader.forward(1)?;
            }
        } else if let Some(n) = decimal_digit(c)? {
            increment = Some(n as usize);
            if n == 0 {
                return Err(self.error(
                    Some("while scanning a block scalar"),
                    Some(start.clone()),
                    "expected indentation indicator in the range 1-9, but found 0",
                ));
            }
            self.reader.forward(1)?;
            c = self.reader.peek(0)?;
            if matches!(c, 45 | 43) {
                indicator = Some(c);
                self.reader.forward(1)?;
            }
        }
        c = self.reader.peek(0)?;
        if c != 32 && !nul_line(c) {
            return Err(self.error(
                Some("while scanning a block scalar"),
                Some(start.clone()),
                format!(
                    "expected chomping or indentation indicators, but found {}({c})",
                    point(c)
                ),
            ));
        }
        Ok((indicator, increment))
    }
    fn block_indentation(&mut self) -> Result<(Vec<u16>, usize, Mark), YamlError> {
        let mut chunks = Vec::new();
        let mut max_indent = 0;
        let mut end = self.reader.mark();
        while matches!(self.reader.peek(0)?, 32 | 10 | 13) {
            if self.reader.peek(0)? != 32 {
                if let Some(lb) = self.line_break()? {
                    chunks.push(lb);
                }
                end = self.reader.mark();
            } else {
                self.reader.forward(1)?;
                max_indent = max_indent.max(self.reader.column);
            }
        }
        let indent = self.reader.column;
        if indent > 0 && indent < max_indent {
            return Err(self.error(Some("while scanning a block scalar"),Some(end),format!(" the leading empty lines contain more spaces ({max_indent}) than the first non-empty line ({indent}).")));
        }
        Ok((chunks, indent, end))
    }
    fn block_breaks(&mut self, indent: usize) -> Result<(Vec<u16>, Mark), YamlError> {
        let mut chunks = Vec::new();
        let mut end = self.reader.mark();
        while self.reader.column < indent && self.reader.peek(0)? == 32 {
            self.reader.forward(1)?;
        }
        while let Some(lb) = self.line_break()? {
            chunks.push(lb);
            end = self.reader.mark();
            while self.reader.column < indent && self.reader.peek(0)? == 32 {
                self.reader.forward(1)?;
            }
        }
        Ok((chunks, end))
    }
    pub(super) fn ignored_line(&mut self, start: &Mark, context: &str) -> Result<(), YamlError> {
        while self.reader.peek(0)? == 32 {
            self.reader.forward(1)?;
        }
        if self.reader.peek(0)? == 35 {
            let mut n = 0;
            while !nul_line(self.reader.peek(n)?) {
                n += 1;
            }
            self.reader.forward(n)?;
        }
        let c = self.reader.peek(0)?;
        if self.line_break()?.is_none() && c != 0 {
            return Err(self.error(
                Some(context),
                Some(start.clone()),
                format!(
                    "expected a comment or a line break, but found {}({c})",
                    point(c)
                ),
            ));
        }
        Ok(())
    }
}
pub(super) fn decimal_digit(c: u32) -> Result<Option<u32>, YamlError> {
    if (48..=57).contains(&c) {
        return Ok(Some(c - 48));
    }
    if crate::java_regex::character_record(c).is_some_and(|r| r.predicate_bits & (1 << 2) != 0) {
        // Verified across all 680 JDK 21 Unicode 15.0.0 digits: Character.isDigit
        // coincides with General_Category=Nd (9), and every Nd block is a
        // contiguous run of 10 digits 0..=9 (or adjacent 10-digit runs).
        let mut start = c;
        while start > 0
            && crate::java_regex::character_record(start - 1).is_some_and(|r| r.category == 9)
        {
            start -= 1;
        }
        return Ok(Some((c - start) % 10));
    }
    Ok(None)
}
