//! Shared checked UTF-16 regex adapter. Language Regex and the M1 transport
//! use the same matcher and retain their separate Uu/u compile flags.
use crate::java_regex::{CompileErrorKind, Limits, MatchRanges, Utf16Regex};
use crate::pkl_string::PklString;
use std::fmt::Write;

#[derive(Debug)]
pub(crate) struct CompileFailure {
    pub kind: CompileErrorKind,
    pub message: PklString,
}
fn syntax_message(message: &str, index: usize, pattern: &[u16]) -> PklString {
    let mut parts = vec![
        format!("{message} near index {index}\n").into(),
        PklString::from_utf16(pattern.to_vec()),
    ];
    if index < pattern.len() {
        parts.push(format!("\n{}^", " ".repeat(index)).into());
    }
    PklString::join(&parts, "")
}
/// The error index follows the backend parser cursor, while the source retains
/// the original UTF-16 units. Unsupported and budget failures keep their kind.
pub(crate) fn compile_utf16(pattern: &[u16], flags: &str) -> Result<Utf16Regex, CompileFailure> {
    Utf16Regex::with_flags(pattern, flags).map_err(|error| {
        let kind = error.kind;
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
        CompileFailure {
            kind,
            message: syntax_message(&message, index, pattern),
        }
    })
}
fn compile(pattern: &str) -> Result<Utf16Regex, String> {
    compile_utf16(&pattern.encode_utf16().collect::<Vec<_>>(), "u")
        .map_err(|e| e.message.to_java_utf8())
}
fn execution_error(error: crate::java_regex::MatchError) -> String {
    format!("Regex backend execution failed: {error}")
}
fn hex(value: &str) -> String {
    let mut result = String::new();
    for byte in value.bytes() {
        write!(result, "{byte:02x}").expect("String write");
    }
    result
}
fn raw_hex(units: &[u16]) -> String {
    hex(&PklString::from_utf16(units.to_vec()).to_java_utf8())
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
const MAX_REGEX_REPLACEMENT_UTF16_UNITS: usize = 128 * 1024 * 1024;

fn checked_extend_u16(out: &mut Vec<u16>, slice: &[u16]) -> Result<(), String> {
    let next = out
        .len()
        .checked_add(slice.len())
        .ok_or_else(|| "Regex replacement result exceeds maximum string length".to_string())?;
    if next > MAX_REGEX_REPLACEMENT_UTF16_UNITS {
        return Err("Regex replacement result exceeds maximum string length".to_string());
    }
    out.try_reserve(slice.len())
        .map_err(|_| "Regex replacement result exceeds available memory".to_string())?;
    out.extend_from_slice(slice);
    Ok(())
}

fn checked_push_u16(out: &mut Vec<u16>, unit: u16) -> Result<(), String> {
    if out.len() >= MAX_REGEX_REPLACEMENT_UTF16_UNITS {
        return Err("Regex replacement result exceeds maximum string length".to_string());
    }
    out.try_reserve(1)
        .map_err(|_| "Regex replacement result exceeds available memory".to_string())?;
    out.push(unit);
    Ok(())
}

fn append_group(
    output: &mut Vec<u16>,
    matched: &MatchRanges,
    index: usize,
    input: &[u16],
) -> Result<(), String> {
    let range = matched
        .groups
        .get(index)
        .ok_or_else(|| format!("No group {index}"))?;
    if let Some((a, b)) = range {
        let slice = input.get(*a..*b).ok_or("invalid backend capture range")?;
        checked_extend_u16(output, slice)?;
    }
    Ok(())
}
/// Expand after a successful match, preserving Java's replacement error order.
/// Named references use the parser-owned name map, not a textual pattern scan.
pub(crate) fn expand_replacement(
    replacement: &[u16],
    matched: &MatchRanges,
    regex: &Utf16Regex,
    input: &[u16],
) -> Result<Vec<u16>, String> {
    let mut output = Vec::new();
    let mut position = 0;
    while let Some(&unit) = replacement.get(position) {
        position += 1;
        match unit {
            0x5c => {
                let next = *replacement
                    .get(position)
                    .ok_or("character to be escaped is missing")?;
                checked_push_u16(&mut output, next)?;
                position += 1;
            }
            0x24 => {
                let first = *replacement
                    .get(position)
                    .ok_or("Illegal group reference: group index is missing")?;
                position += 1;
                if first == 0x7b {
                    let start = position;
                    while replacement
                        .get(position)
                        .is_some_and(|u| matches!(*u, 0x30..=0x39|0x41..=0x5a|0x61..=0x7a))
                    {
                        position += 1;
                    }
                    if start == position {
                        return Err("named capturing group has 0 length name".into());
                    }
                    if replacement.get(position) != Some(&0x7d) {
                        return Err("named capturing group is missing trailing '}'".into());
                    }
                    position += 1;
                    let name = replacement[start..position - 1]
                        .iter()
                        .map(|u| char::from(*u as u8))
                        .collect::<String>();
                    if name.as_bytes()[0].is_ascii_digit() {
                        return Err(format!(
                            "capturing group name {{{name}}} starts with digit character"
                        ));
                    }
                    let index = regex
                        .named_groups()
                        .get(&name)
                        .ok_or_else(|| format!("No group with name {{{name}}}"))?;
                    append_group(&mut output, matched, *index, input)?;
                } else if (0x30..=0x39).contains(&first) {
                    let mut group = usize::from(first - 0x30);
                    while let Some(&next) = replacement
                        .get(position)
                        .filter(|u| (0x30..=0x39).contains(*u))
                    {
                        let candidate = group
                            .checked_mul(10)
                            .and_then(|n| n.checked_add(usize::from(next - 0x30)));
                        let Some(candidate) = candidate.filter(|n| *n <= regex.group_count())
                        else {
                            break;
                        };
                        group = candidate;
                        position += 1;
                    }
                    append_group(&mut output, matched, group, input)?;
                } else {
                    return Err("Illegal group reference".into());
                }
            }
            _ => checked_push_u16(&mut output, unit)?,
        }
    }
    Ok(output)
}
fn replace_checked(
    regex: &Utf16Regex,
    input: &[u16],
    replacement: &str,
) -> Result<Vec<u16>, String> {
    let mut matcher = regex.matcher(input, Limits::default());
    let mut result = Vec::new();
    let replacement_units: Vec<u16> = replacement.encode_utf16().collect();
    let mut end = 0;
    while let Some(m) = matcher.find().map_err(execution_error)? {
        let prefix = input
            .get(end..m.start)
            .ok_or("invalid backend match range")?;
        checked_extend_u16(&mut result, prefix)?;
        let expanded = expand_replacement(&replacement_units, &m, regex, input)?;
        checked_extend_u16(&mut result, &expanded)?;
        end = m.end;
    }
    let suffix = input.get(end..).ok_or("invalid backend match end")?;
    checked_extend_u16(&mut result, suffix)?;
    Ok(result)
}
/// UTF-8 public adapter; Java UTF-8 replacement of unmatched surrogates occurs
/// only at this output boundary, never in the parser or matcher.
pub fn replace_all(pattern: &str, input: &str, replacement: &str) -> Result<String, String> {
    let regex = compile(pattern)?;
    let units = input.encode_utf16().collect::<Vec<_>>();
    replace_checked(&regex, &units, replacement).map(|u| PklString::from_utf16(u).to_java_utf8())
}
fn find_text(regex: &Utf16Regex, input: &[u16], groups: bool) -> Result<String, String> {
    let mut matcher = regex.matcher(input, Limits::default());
    let mut result = String::new();
    while let Some(m) = matcher.find().map_err(execution_error)? {
        let value = input
            .get(m.start..m.end)
            .ok_or("invalid backend match range")?;
        writeln!(result, "match:{},{},{}", m.start, m.end, raw_hex(value)).expect("String write");
        if groups {
            for group in m.groups.iter().skip(1) {
                match group {
                    Some((a, b)) => writeln!(
                        result,
                        "group:{a},{b},{}",
                        raw_hex(input.get(*a..*b).ok_or("invalid backend capture range")?)
                    )
                    .expect("String write"),
                    None => result.push_str("group:null\n"),
                }
            }
        }
    }
    result.push_str("end\n");
    Ok(result)
}
/// M1's unchanged UTF-8 hex protocol. Actual find/matches/replace/cursor all use
/// the shared checked matcher; an engine failure is an error, never no-match.
pub fn probe(input: &str) -> Result<String, String> {
    let fields = input.split('\t').collect::<Vec<_>>();
    if !(3..=4).contains(&fields.len()) {
        return Err("invalid regex probe input".into());
    }
    let pattern = unhex(fields[1])?;
    let source = unhex(fields[2])?;
    if fields[0] == "cursor" && !pattern.is_empty() && pattern != "." {
        return Err("cursor probe supports only empty and dot".into());
    }
    let regex = match compile(&pattern) {
        Ok(r) => r,
        Err(e) => return Ok(format!("error:{}\n", hex(&e))),
    };
    let units = source.encode_utf16().collect::<Vec<_>>();
    let result = match fields[0] {
        "matches" => regex
            .captures_entire(&units, Limits::default())
            .map(|m| format!("{}\n", m.is_some()))
            .map_err(execution_error),
        "replace" if fields.len() == 4 => {
            let replacement = unhex(fields[3])?;
            replace_checked(&regex, &units, &replacement).map(|u| format!("text:{}\n", raw_hex(&u)))
        }
        "find" => find_text(&regex, &units, true),
        "cursor" => find_text(&regex, &units, false),
        _ => return Err("invalid regex probe operation".into()),
    };
    Ok(match result {
        Ok(text) => text,
        Err(error) => format!("error:{}\n", hex(&error)),
    })
}
