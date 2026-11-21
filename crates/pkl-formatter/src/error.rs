//! Error message templates mirroring `org/pkl/parser/errorMessages.properties`.

pub(crate) fn unexpected_character(got: &str, did_you_mean: &str) -> String {
    format!("Unexpected character `{got}`. Did you mean `{did_you_mean}`?")
}

pub(crate) fn unexpected_character_3(got: &str, opt1: &str, opt2: &str, opt3: &str) -> String {
    format!("Unexpected character `{got}`. Did you mean `{opt1}`, `{opt2}` or `{opt3}`?")
}

pub(crate) fn unexpected_token(got: &str, expected: &str) -> String {
    format!("Unexpected token `{got}`. Expected `{expected}`.")
}

pub(crate) fn unexpected_token_2(got: &str, expected1: &str, expected2: &str) -> String {
    format!("Unexpected token `{got}`. Expected `{expected1}` or `{expected2}`.")
}

pub(crate) fn unexpected_token_for_expression(got: &str) -> String {
    format!("Unexpected token `{got}`.")
}

pub(crate) fn unexpected_token_for_type(got: &str) -> String {
    format!("Unexpected token `{got}`. Expected a type.")
}

pub(crate) fn unexpected_token_for_type_2(got: &str, expected: &str) -> String {
    format!("Unexpected token `{got}`. Expected a type or `{expected}`.")
}

pub(crate) fn unexpected_curly_probably_amends_expression(parent: &str) -> String {
    format!(
        "Unexpected token: `{{`.\n\nIf you meant to write an amends expression, wrap the parent in parentheses. Try: `({parent}) {{ ... }}`"
    )
}

pub(crate) fn unexpected_end_of_file() -> String {
    "Unexpected end of file.".to_string()
}

pub(crate) fn invalid_character(ch: &str) -> String {
    format!("Invalid identifier `{ch}`.")
}

pub(crate) fn invalid_top_level_token() -> String {
    "Invalid token at position. Expected a class, typealias, method, or property.".to_string()
}

pub(crate) fn invalid_separator_position() -> String {
    "Unexpected separator character.\n\nThe separator character (`_`) cannot follow `0x`, `0b`, `.`, `e`, or 'E' in a number literal.".to_string()
}

pub(crate) fn invalid_character_escape_sequence(seq: &str, backslash: &str) -> String {
    format!(
        "Invalid character escape sequence `{seq}`.\n\nValid character escape sequences are:  {backslash}n  {backslash}r  {backslash}t  {backslash}\"  {backslash}\\ {backslash}<newline>"
    )
}

pub(crate) fn invalid_line_continuation_escape_sequence() -> String {
    "Invalid line continuation escape sequence.\n\nLine continuations are only allowed in multi-line strings.".to_string()
}

pub(crate) fn invalid_line_continuation_escape_sequence_whitespace() -> String {
    "Invalid line continuation escape sequence.\n\nWhitespace between the continuation escape and following newline is not allowed.".to_string()
}

pub(crate) fn missing_delimiter(delim: &str) -> String {
    format!("Missing `{delim}` delimiter.")
}

pub(crate) fn unterminated_unicode_escape_sequence(seq: &str) -> String {
    format!(
        "Unterminated Unicode escape sequence `{seq}`.\n\nUnicode escape sequences must end with `}}`."
    )
}

pub(crate) fn keyword_not_allowed_here(keyword: &str) -> String {
    format!(
        "Keyword `{keyword}` is not allowed here.\n\nIf you must use this name as identifier, enclose it in backticks."
    )
}

pub(crate) fn type_annotation_in_amends() -> String {
    "Properties with type annotations cannot have object bodies.\n\nTo define both a type annotation and an object body, try using assignment instead.\nFor example: `obj: Bird = new { ... }`.".to_string()
}

pub(crate) fn string_content_must_begin_on_new_line() -> String {
    "The content of a multi-line string must begin on a new line.".to_string()
}

pub(crate) fn string_indentation_must_match_last_line() -> String {
    "Line must match or exceed indentation of the String's last line.".to_string()
}

pub(crate) fn closing_string_delimiter_must_begin_on_new_line() -> String {
    "The closing delimiter of a multi-line string must begin on a new line.".to_string()
}

pub(crate) fn interpolation_in_constant() -> String {
    "String constant cannot have interpolated values.".to_string()
}

pub(crate) fn multiple_union_defaults() -> String {
    "A type union cannot have more than one default type.".to_string()
}

pub(crate) fn not_a_union() -> String {
    "Only type unions can have a default marker (*).".to_string()
}

pub(crate) fn wrong_headers(target: &str) -> String {
    format!("{target} cannot have doc comments, annotations or modifiers.")
}

pub(crate) fn dangling_doc_comment() -> String {
    "Dangling documentation comment.\n\nDocumentation comments must be attached to modules, classes, typealiases, methods, or properties.".to_string()
}
