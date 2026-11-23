//! Pinned GlobResolver pattern translation. This is not String.toRegex:
//! Java compiles glob patterns without the language Regex constructor flags.
use super::*;
use crate::java_regex::{Limits, Utf16Regex};

pub(super) struct Pattern(Utf16Regex);

enum TranslationFailure {
    Invalid(PklString),
    Unsupported(PklString),
}
impl From<&str> for TranslationFailure {
    fn from(message: &str) -> Self {
        Self::Invalid(message.into())
    }
}

impl Pattern {
    pub(super) fn compile(part: &PklString, authored: &PklString) -> Result<Self, RuntimeError> {
        let regex = translate(part).map_err(|failure| match failure {
            TranslationFailure::Invalid(hint) => glob::invalid_pattern(authored, hint),
            TranslationFailure::Unsupported(message) => {
                RuntimeError::new(ErrorKind::Unsupported, message)
            }
        })?;
        let units = regex.encode_utf16().collect::<Vec<_>>();
        let compiled = crate::regex::compile_utf16(&units, "").map_err(|failure| {
            // PatternSyntaxException is not caught by upstream GlobResolver.
            // A backend gap or Java internal-error boundary must not become an
            // ordinary invalid-glob diagnostic with fabricated provenance.
            RuntimeError::new(
                ErrorKind::Unsupported,
                PklString::from("Glob regex compilation is not supported: ")
                    .concat(&failure.message),
            )
        })?;
        Ok(Self(compiled))
    }

    pub(super) fn matches(&self, name: &PklString) -> Result<bool, RuntimeError> {
        let units = name.encode_utf16().collect::<Vec<_>>();
        self.0
            .captures_entire(&units, Limits::default())
            .map(|matched| matched.is_some())
            .map_err(|error| {
                RuntimeError::new(
                    ErrorKind::Unsupported,
                    format!("Glob regex execution is not supported: {error}"),
                )
            })
    }
}

pub(super) fn regular(part: &PklString) -> bool {
    !part
        .encode_utf16()
        .any(|unit| matches!(unit, 92 | 123 | 91 | 63 | 42))
}

fn append(output: &mut Vec<u16>, text: &str) {
    output.extend(text.encode_utf16());
}

fn translate(pattern: &PklString) -> Result<PklString, TranslationFailure> {
    let input = pattern.encode_utf16().collect::<Vec<_>>();
    let mut output = vec![u16::from(b'^')];
    let mut group = false;
    let mut index = 0;
    while let Some(&unit) = input.get(index) {
        let next = input.get(index + 1).copied().unwrap_or(0);
        match unit {
            123 => {
                if group {
                    return Err("Sub-patterns cannot be nested. To fix, remove or escape the inner `{` character.".into());
                }
                group = true;
                append(&mut output, "(?:(?:");
            }
            125 if group => {
                group = false;
                append(&mut output, "))");
            }
            44 if group => append(&mut output, ")|(?:"),
            92 => {
                if next == 0 {
                    return Err("The backslash (`\\`) is the escape character, and cannot terminate a glob pattern.".into());
                }
                if !matches!(next, 63 | 42 | 91 | 123 | 92) {
                    return Err(TranslationFailure::Invalid(
                        PklString::from("Invalid escape character `\\")
                            .concat(&PklString::from_utf16(vec![next]))
                            .concat(&"`.".into()),
                    ));
                }
                output.extend([92, next]);
                index += 1;
            }
            91 => {
                index = character_class(&input, index, &mut output)?;
            }
            63 | 42 | 43 | 64 | 33 if next == 40 => {
                return Err("Extended globbing features are not supported.".into());
            }
            63 => append(&mut output, "."),
            42 if next == 42 => {
                append(&mut output, ".*");
                index += 1;
            }
            42 => append(&mut output, "[^/]*"),
            // Preserve the pinned switch's shared '+'/'@' branch, including
            // its literal plus translation for '@'. Do not silently repair it.
            43 | 64 => append(&mut output, "\\+"),
            46 | 40 | 37 | 94 | 36 | 124 => output.extend([92, unit]),
            _ => output.push(unit),
        }
        index += 1;
    }
    if group {
        // The upstream implementation passes this key literally to the
        // exception constructor instead of resolving ErrorMessages.create.
        return Err("invalidGlobUnclosedSubpattern".into());
    }
    output.push(u16::from(b'$'));
    Ok(PklString::from_utf16(output))
}

fn character_class(
    input: &[u16],
    start: usize,
    output: &mut Vec<u16>,
) -> Result<usize, TranslationFailure> {
    let missing =
        || TranslationFailure::Invalid("Missing `]` to terminate character class.".into());
    append(output, "[[^/]&&[");
    let mut index = start;
    match input.get(index + 1).copied().unwrap_or(0) {
        94 => {
            append(output, "\\^");
            index += 1;
        }
        33 => {
            append(output, "^");
            index += 1;
        }
        93 => {
            output.push(93);
            index += 1;
        }
        0 => return Err(missing()),
        _ => {}
    }
    index += 1;
    if index == input.len() {
        // The pinned Java implementation indexes past the String for [!, [^
        // and [] here. Keep that internal-error boundary unsupported instead
        // of manufacturing an ordinary missing-terminator diagnostic.
        return Err(TranslationFailure::Unsupported(
            "Java internal-error handling for this glob character class is not supported.".into(),
        ));
    }
    loop {
        let current = *input.get(index).ok_or_else(missing)?;
        if current == 93 {
            append(output, "]]");
            return Ok(index);
        }
        if current == 91
            && input
                .get(index + 1)
                .is_some_and(|next| matches!(*next, 58 | 61 | 46))
        {
            return Err("Glob patterns do not support named character classes, collating symbols, nor equivalence class expressions.".into());
        }
        if current == 47 {
            return Err("The character `/` is not valid in a character class.".into());
        }
        if current == 92 {
            output.extend([92, 92]);
        } else {
            output.push(current);
        }
        index += 1;
    }
}
