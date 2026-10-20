//! M1 Java Pattern candidate with explicit UTF-16 positions and replacement errors.
//! The dependency targets JDK 25; our baseline remains the pinned JDK 21 oracle.
use java_regex::{MatchInfo, Regex};
use std::fmt::Write;

// Java reserves unrecognized ASCII-letter escapes. The candidate treats some
// as literals, so validate the lexical stream before asking it to compile.
// Quoting, character classes, comments, and scoped x/d flags affect that stream.
fn reserved_escape(pattern: &str) -> Option<usize> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut position = 0;
    let mut quoted = false;
    let mut flags = (false, false); // COMMENTS, UNIX_LINES
    let mut scopes = Vec::new();
    let mut classes = Vec::<bool>::new(); // true before the first class member
    while position < chars.len() {
        let c = chars[position];
        if quoted {
            if c == '\\' && chars.get(position + 1) == Some(&'E') {
                quoted = false;
                position += 2;
            } else {
                position += 1;
            }
            continue;
        }
        if flags.0 && c == '#' {
            while position < chars.len() {
                let c = chars[position];
                if c == '\n' || (!flags.1 && matches!(c, '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'))
                {
                    break;
                }
                position += 1;
            }
            continue;
        }
        if c == '\\' {
            if let Some(&escaped) = chars.get(position + 1) {
                if escaped.is_ascii_alphabetic()
                    && !"AbBacDdefGhHkNnpPQrRsStuvVwWxXzZ".contains(escaped)
                {
                    return Some(position + 1);
                }
                quoted = escaped == 'Q';
                if let Some(first) = classes.last_mut() {
                    *first = false;
                }
                position += if escaped == 'c' && position + 2 < chars.len() {
                    3
                } else {
                    2
                };
            } else {
                position += 1;
            }
            continue;
        }
        if c == '[' {
            if let Some(first) = classes.last_mut() {
                *first = false;
            }
            classes.push(true);
        } else if !classes.is_empty() {
            let first = classes.last_mut().expect("class stack");
            if c == ']' && !*first {
                classes.pop();
            } else if c != '^' && !(flags.0 && c.is_ascii_whitespace()) {
                *first = false;
            }
        } else if c == '(' {
            let mut cursor = position + 2;
            let mut changed = flags;
            let mut enable = true;
            if chars.get(position + 1) == Some(&'?') {
                while let Some(&flag) = chars.get(cursor) {
                    match flag {
                        '-' => enable = false,
                        'x' => changed.0 = enable,
                        'd' => changed.1 = enable,
                        'i' | 'm' | 's' | 'u' | 'U' => {}
                        _ => break,
                    }
                    cursor += 1;
                }
                if cursor > position + 2 && chars.get(cursor) == Some(&')') {
                    flags = changed;
                    position = cursor + 1;
                    continue;
                }
                if cursor > position + 2 && chars.get(cursor) == Some(&':') {
                    scopes.push(flags);
                    flags = changed;
                    position = cursor + 1;
                    continue;
                }
            }
            scopes.push(flags);
        } else if c == ')'
            && let Some(saved) = scopes.pop()
        {
            flags = saved;
        }
        position += 1;
    }
    None
}

fn syntax_message(message: &str, index: usize, pattern: &str) -> String {
    let mut result = format!("{message} near index {index}\n{pattern}");
    if index < pattern.encode_utf16().count() {
        result.push('\n');
        result.extend(std::iter::repeat_n(' ', index));
        result.push('^');
    }
    result
}

/// Adapt structured parser categories, retaining the complete pattern and
/// diagnostic bytes. This is deliberately not a claim of all Pattern errors.
fn compile(pattern: &str) -> Result<Regex, String> {
    let invalid_escape = reserved_escape(pattern);
    let compiled = Regex::with_flags(pattern, "u").map_err(|error| {
        let (message, index) = if error.message == "Expected ')'" {
            ("Unclosed group".to_owned(), error.index)
        } else if error.message == "Unclosed character class" {
            (error.message, error.index.saturating_sub(1))
        } else if error.message.starts_with("Invalid range: ") {
            (
                "Illegal character range".to_owned(),
                error.index.saturating_sub(1),
            )
        } else if error
            .message
            .starts_with("Group name must start with a letter")
        {
            (
                "capturing group name does not start with a Latin letter".to_owned(),
                error.index.saturating_sub(1),
            )
        } else {
            (error.message, error.index)
        };
        // OpenJDK compares its parser index with UTF-16 pattern length here,
        // even though the parser can have converted the pattern to code points.
        if let Some(escape) = invalid_escape.filter(|escape| *escape < index) {
            syntax_message("Illegal/unsupported escape sequence", escape, pattern)
        } else {
            syntax_message(&message, index, pattern)
        }
    })?;
    if let Some(index) = invalid_escape {
        Err(syntax_message(
            "Illegal/unsupported escape sequence",
            index,
            pattern,
        ))
    } else {
        Ok(compiled)
    }
}

/// Experimental matcher text model. Positions are UTF-16 code units, while
/// consuming nodes can decode a code point starting at any permitted position.
#[derive(Debug, Clone)]
pub struct Utf16Input {
    units: Vec<u16>,
}

impl Utf16Input {
    pub fn new(input: &str) -> Self {
        Self {
            units: input.encode_utf16().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// A trailing surrogate reached after an empty match remains a single unit.
    /// This is essential: not every Java matcher position is a scalar boundary.
    pub fn code_point_at(&self, position: usize) -> Option<(u32, usize)> {
        let first = u32::from(*self.units.get(position)?);
        if (0xd800..=0xdbff).contains(&first)
            && let Some(second) = self.units.get(position + 1).copied().map(u32::from)
            && (0xdc00..=0xdfff).contains(&second)
        {
            return Some((0x10000 + ((first - 0xd800) << 10) + second - 0xdc00, 2));
        }
        Some((first, 1))
    }

    /// The next Java `find()` search starts at the previous match end; after an
    /// empty match it advances exactly one UTF-16 unit, including within a pair.
    pub fn next_find_position(&self, start: usize, end: usize) -> Option<usize> {
        if start > end || end > self.len() {
            return None;
        }
        let next = if start == end {
            end.checked_add(1)?
        } else {
            end
        };
        (next <= self.len()).then_some(next)
    }
}

// A bounded proof of the proposed text/cursor contract, independent of the
// scalar-only regex backend. Empty and dot are the only supported probe patterns.
fn cursor_probe(pattern: &str, source: &str) -> Result<String, String> {
    if !pattern.is_empty() && pattern != "." {
        return Err("cursor probe supports only empty and dot".into());
    }
    let text = Utf16Input::new(source);
    let mut result = String::new();
    let mut position = 0;
    loop {
        let end = if pattern.is_empty() {
            position
        } else {
            let Some((point, width)) = text.code_point_at(position) else {
                break;
            };
            if matches!(point, 0xa | 0xd | 0x85 | 0x2028 | 0x2029) {
                position += width;
                continue;
            }
            position + width
        };
        let matched = String::from_utf16(&text.units[position..end])
            .map_err(|_| "cursor probe captured an unpaired surrogate")?;
        writeln!(result, "match:{position},{end},{}", hex(&matched)).expect("String write");
        let Some(next) = text.next_find_position(position, end) else {
            break;
        };
        position = next;
    }
    result.push_str("end\n");
    Ok(result)
}

fn hex(value: &str) -> String {
    let mut result = String::new();
    for byte in value.bytes() {
        write!(result, "{byte:02x}").expect("String write");
    }
    result
}
fn unhex(value: &str) -> Result<String, String> {
    if !value.len().is_multiple_of(2) || !value.is_ascii() {
        return Err("invalid UTF-8 hex input".into());
    }
    let bytes = (0..value.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| "invalid hex input".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes).map_err(|_| "invalid UTF-8 input".into())
}
fn utf16_offset(input: &str, scalar: usize) -> usize {
    input.chars().take(scalar).map(char::len_utf16).sum()
}

/// Replacement expansion must be fallible: the dependency silently accepts
/// malformed references. Validate only after a match, as java.util.Matcher does.
fn expand(replacement: &str, matched: &MatchInfo, pattern: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => output.push(chars.next().ok_or("character to be escaped is missing")?),
            '$' => {
                let first = chars
                    .next()
                    .ok_or("Illegal group reference: group index is missing")?;
                if first == '{' {
                    let mut name = String::new();
                    while chars.peek().is_some_and(char::is_ascii_alphanumeric) {
                        name.push(chars.next().expect("peeked character"));
                    }
                    if name.is_empty() {
                        return Err("named capturing group has 0 length name".into());
                    }
                    if chars.next() != Some('}') {
                        return Err("named capturing group is missing trailing '}'".into());
                    }
                    if name.as_bytes()[0].is_ascii_digit() {
                        return Err(format!(
                            "capturing group name {{{name}}} starts with digit character"
                        ));
                    }
                    // The probe supports ordinary named declarations. Exotic
                    // quoted/commented occurrences require a parser-owned name table.
                    if !pattern.contains(&format!("(?<{name}>")) {
                        return Err(format!("No group with name {{{name}}}"));
                    }
                    if let Some(value) = matched.name(&name) {
                        output.push_str(value);
                    }
                } else if first.is_ascii_digit() {
                    let mut group = first.to_digit(10).expect("ASCII digit") as usize;
                    while let Some(next) = chars.peek().and_then(|c| c.to_digit(10)) {
                        let candidate = group * 10 + next as usize;
                        if candidate > matched.group_count() {
                            break;
                        }
                        group = candidate;
                        chars.next();
                    }
                    if group > matched.group_count() {
                        return Err(format!("No group {group}"));
                    }
                    if let Some(value) = matched.group(group) {
                        output.push_str(value);
                    }
                } else {
                    return Err("Illegal group reference".into());
                }
            }
            _ => output.push(c),
        }
    }
    Ok(output)
}

pub fn replace_all(pattern: &str, input: &str, replacement: &str) -> Result<String, String> {
    let regex = compile(pattern)?;
    let chars: Vec<_> = input.chars().collect();
    let mut result = String::new();
    let mut end = 0;
    for matched in regex.find_iter(input) {
        result.extend(chars[end..matched.start].iter());
        result.push_str(&expand(replacement, &matched, pattern)?);
        end = matched.end;
    }
    result.extend(chars[end..].iter());
    Ok(result)
}

/// Protocol: `find|matches|replace<TAB>pattern_utf8_hex<TAB>input_utf8_hex`
/// and a replacement hex field for `replace`. Results retain complete error text
/// in hex and all captures with Java UTF-16 offsets; no diagnostic trimming.
pub fn probe(input: &str) -> Result<String, String> {
    let fields: Vec<_> = input.split('\t').collect();
    if !(3..=4).contains(&fields.len()) {
        return Err("invalid regex probe input".into());
    }
    let pattern = unhex(fields[1])?;
    let source = unhex(fields[2])?;
    if fields[0] == "cursor" {
        return cursor_probe(&pattern, &source);
    }
    let regex = match compile(&pattern) {
        Ok(regex) => regex,
        Err(error) => return Ok(format!("error:{}\n", hex(&error.to_string()))),
    };
    match fields[0] {
        "matches" => Ok(format!("{}\n", regex.matches(&source))),
        "replace" if fields.len() == 4 => {
            let replacement = unhex(fields[3])?;
            Ok(match replace_all(&pattern, &source, &replacement) {
                Ok(output) => format!("text:{}\n", hex(&output)),
                Err(error) => format!("error:{}\n", hex(&error)),
            })
        }
        "find" => {
            let mut result = String::new();
            for matched in regex.find_iter(&source) {
                writeln!(
                    result,
                    "match:{},{},{}",
                    utf16_offset(&source, matched.start),
                    utf16_offset(&source, matched.end),
                    hex(&matched.matched_text)
                )
                .expect("String write");
                for (value, position) in matched.groups.iter().zip(&matched.group_positions) {
                    if let (Some(value), Some((start, end))) = (value, position) {
                        writeln!(
                            result,
                            "group:{},{},{}",
                            utf16_offset(&source, *start),
                            utf16_offset(&source, *end),
                            hex(value)
                        )
                        .expect("String write");
                    } else {
                        result.push_str("group:null\n");
                    }
                }
            }
            result.push_str("end\n");
            Ok(result)
        }
        _ => Err("invalid regex probe operation".into()),
    }
}
