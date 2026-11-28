//! Path/URI encoding, URI resolution/relativization, identifier quoting, doc-comment
//! summary/overflow splitting, and semver comparison matching Apple Pkl 0.32.0
//! `org.pkl.doc.Util`, `org.pkl.core.util.IoUtils`, and `org.pkl.parser.Lexer`.

use crate::model::DocGeneratorError;
use pkl_runtime::packages::PackageVersion;
use std::cmp::Ordering;
use std::path::{Component, Path, PathBuf};

/// Validates and joins a relative path under `output_dir`, rejecting absolute paths,
/// Windows drive prefixes, NUL bytes, backslashes, `..` traversal, and symlinks resolving
/// outside `output_dir`.
pub fn join_confined_output_path(
    output_dir: &Path,
    rel: &str,
) -> Result<PathBuf, DocGeneratorError> {
    if rel.contains('\0') || rel.contains('\\') {
        return Err(DocGeneratorError::user(format!(
            "Invalid documentation output path `{rel}`: path contains forbidden character."
        )));
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(DocGeneratorError::user(format!(
            "Invalid documentation output path `{rel}`: path must be relative to output directory."
        )));
    }
    for comp in rel_path.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(DocGeneratorError::user(format!(
                    "Invalid documentation output path `{rel}`: path escapes output directory."
                )));
            }
        }
    }
    let candidate = output_dir.join(rel_path);
    ensure_path_under_output_dir(output_dir, &candidate)
}

/// Ensures `candidate` resolves inside `output_dir` (following symlinks including dangling
/// symlinks before `..`) without escaping the documentation output root.
pub fn ensure_path_under_output_dir(
    output_dir: &Path,
    candidate: &Path,
) -> Result<PathBuf, DocGeneratorError> {
    let canon_root = pkl_host_native::canonicalize_root_dir(output_dir)
        .unwrap_or_else(|_| output_dir.to_path_buf());
    ensure_path_under_canon_root(output_dir, &canon_root, candidate, false)
}

/// Fast confinement check against a pre-canonicalized `canon_root`.
///
/// When `candidate` lies lexically beneath `output_dir` or `canon_root` without `..`,
/// walks only the relative tail below `canon_root` via `symlink_metadata`, falling back
/// to full `resolve_canonical_or_ancestor` whenever an existing symlink or `..` is seen.
pub(crate) fn ensure_path_under_canon_root(
    output_dir: &Path,
    canon_root: &Path,
    candidate: &Path,
    allow_equal_root: bool,
) -> Result<PathBuf, DocGeneratorError> {
    if candidate.to_string_lossy().contains('\0') {
        return Err(DocGeneratorError::user(format!(
            "Invalid documentation output path `{}`: contains NUL byte.",
            candidate.display()
        )));
    }
    let rel_fast = candidate
        .strip_prefix(output_dir)
        .or_else(|_| candidate.strip_prefix(canon_root))
        .ok();
    let mut resolved_fast: Option<PathBuf> = None;
    if let Some(rel) = rel_fast {
        let mut cur = canon_root.to_path_buf();
        let mut fallback = false;
        let mut missing_subtree = false;
        for comp in rel.components() {
            match comp {
                Component::CurDir => {}
                Component::Normal(seg) => {
                    cur.push(seg);
                    if !missing_subtree {
                        match std::fs::symlink_metadata(&cur) {
                            Ok(meta) => {
                                if is_symlink_or_reparse_point(&meta) {
                                    fallback = true;
                                    break;
                                }
                            }
                            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                                // Neither `cur` nor any descendant under it exists yet, so
                                // remaining `Normal` components cannot traverse a symlink.
                                missing_subtree = true;
                            }
                            Err(_) => {
                                fallback = true;
                                break;
                            }
                        }
                    }
                }
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    fallback = true;
                    break;
                }
            }
        }
        if !fallback {
            resolved_fast = Some(cur);
        }
    }
    let resolved = match resolved_fast {
        Some(path) => path,
        None => pkl_host_native::try_resolve_canonical_or_ancestor(candidate).map_err(|()| {
            DocGeneratorError::user(format!(
                "Refusing to access path `{}` outside output directory `{}`.",
                candidate.display(),
                output_dir.display()
            ))
        })?,
    };
    if !resolved.starts_with(canon_root) || (!allow_equal_root && resolved == canon_root) {
        return Err(DocGeneratorError::user(format!(
            "Refusing to access path `{}` outside output directory `{}`.",
            candidate.display(),
            output_dir.display()
        )));
    }
    Ok(candidate.to_path_buf())
}

fn is_symlink_or_reparse_point(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_type().is_symlink() || (meta.file_attributes() & 0x0400 != 0)
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

/// Validates a single relative filename/version path segment (no separators, NUL, `.`, or `..`).
pub fn validate_single_path_segment(label: &str, seg: &str) -> Result<(), DocGeneratorError> {
    if seg.is_empty()
        || matches!(seg, "." | "..")
        || seg.contains(['/', '\\', '\0'])
        || (cfg!(windows) && seg.contains(':'))
    {
        return Err(DocGeneratorError::user(format!(
            "Invalid {label} `{seg}`: contains path separator or traversal segment."
        )));
    }
    Ok(())
}

/// Sanitizes an HTML `href` / `src` URL by neutralizing dangerous executable schemes
/// (`javascript:`, `vbscript:`, `data:`), matching HTML5 leading/embedded ASCII control
/// and whitespace stripping while preserving all safe relative and http(s)/mailto URLs verbatim.
pub fn sanitize_href_url(href: &str) -> &str {
    let trimmed =
        href.trim_start_matches(|c: char| c.is_ascii_control() || c.is_ascii_whitespace());
    let mut scheme = String::new();
    let mut saw_colon = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_control() || ch.is_ascii_whitespace() {
            continue;
        }
        if ch == ':' {
            saw_colon = true;
            break;
        }
        if ch == '/' || ch == '?' || ch == '#' {
            break;
        }
        if scheme.len() < 16 {
            scheme.push(ch.to_ascii_lowercase());
        } else {
            break;
        }
    }
    if saw_colon && matches!(scheme.as_str(), "javascript" | "vbscript" | "data") {
        ""
    } else {
        href
    }
}

/// Serializes `s` as a double-quoted JS/JSON string literal safe to embed inside an inline
/// HTML `<script>` block (escapes `<`, `>`, U+2028, U+2029 in addition to standard JSON escapes).
pub fn push_safe_inline_script_json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{0009}' => out.push_str("\\t"),
            '\u{000A}' => out.push_str("\\n"),
            '\u{000C}' => out.push_str("\\f"),
            '\u{000D}' => out.push_str("\\r"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => {
                let code = c as u32;
                out.push_str("\\u00");
                out.push(hex_lower(((code >> 4) & 0xF) as u8));
                out.push(hex_lower((code & 0xF) as u8));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Mirrors `IoUtils.encodePath(String)`:
/// Encodes Windows-reserved filename characters (`0..=31`, `<>:"\|?*`, excluding `/`)
/// as `(<2-digit-lower-hex>)` and literal `(` as `((`.
pub fn encode_path(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(path.len());
    for ch in path.chars() {
        if is_reserved_windows_filename_char(ch) && ch != '/' {
            let byte = ch as u8;
            out.push('(');
            out.push(hex_lower(byte >> 4));
            out.push(hex_lower(byte & 0x0f));
            out.push(')');
        } else if ch == '(' {
            out.push_str("((");
        } else {
            out.push(ch);
        }
    }
    out
}

fn is_reserved_windows_filename_char(ch: char) -> bool {
    let code = ch as u32;
    code <= 31 || matches!(ch, '<' | '>' | ':' | '"' | '\\' | '/' | '|' | '?' | '*')
}

fn hex_lower(nibble: u8) -> char {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    HEX[nibble as usize] as char
}

fn hex_upper(nibble: u8) -> char {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    HEX[nibble as usize] as char
}

/// Mirrors `Util.kt` `String.uriEncodedComponent`:
/// `URI(null, null, this, null).toString().replace("/", "%2F")`.
pub fn uri_encode_component(s: &str) -> String {
    java_uri_encode_path(s).replace('/', "%2F")
}

/// Mirrors `Util.kt` `String.uriEncoded`:
/// Preserves `;/?:@&=+$,#` and applies `uri_encode_component` to maximal runs between them.
pub fn uri_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut run_start = None;
    for (idx, ch) in s.char_indices() {
        if matches!(
            ch,
            ';' | '/' | '?' | ':' | '@' | '&' | '=' | '+' | '$' | ',' | '#'
        ) {
            if let Some(start) = run_start.take() {
                out.push_str(&uri_encode_component(&s[start..idx]));
            }
            out.push(ch);
        } else if run_start.is_none() {
            run_start = Some(idx);
        }
    }
    if let Some(start) = run_start {
        out.push_str(&uri_encode_component(&s[start..]));
    }
    out
}

/// Encodes a path string matching `new java.net.URI(null, null, path, null).toString()`.
pub fn java_uri_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut buf = [0u8; 4];
    for ch in s.chars() {
        if is_java_uri_path_char(ch) {
            out.push(ch);
        } else {
            let encoded = ch.encode_utf8(&mut buf);
            for &b in encoded.as_bytes() {
                out.push('%');
                out.push(hex_upper(b >> 4));
                out.push(hex_upper(b & 0x0f));
            }
        }
    }
    out
}

/// Encodes a URI fragment matching `new java.net.URI(null, null, null, -1, ..., fragment).toString()`.
fn java_uri_encode_fragment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut buf = [0u8; 4];
    for ch in s.chars() {
        if is_java_uri_uric_char(ch) {
            out.push(ch);
        } else {
            let encoded = ch.encode_utf8(&mut buf);
            for &b in encoded.as_bytes() {
                out.push('%');
                out.push(hex_upper(b >> 4));
                out.push(hex_upper(b & 0x0f));
            }
        }
    }
    out
}

fn is_java_uri_unreserved(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '!' | '.' | '~' | '\'' | '(' | ')' | '*')
}

fn is_java_uri_punct(ch: char) -> bool {
    matches!(ch, ',' | ';' | ':' | '$' | '&' | '+' | '=')
}

fn is_java_uri_other(ch: char) -> bool {
    !ch.is_ascii() && !ch.is_control() && !ch.is_whitespace()
}

/// RFC 2396 `L_PATH` (`pchar | '/' | ';'` + non-ASCII `other` in `java.net.URI`).
fn is_java_uri_path_char(ch: char) -> bool {
    is_java_uri_unreserved(ch)
        || is_java_uri_punct(ch)
        || matches!(ch, '/' | '@')
        || is_java_uri_other(ch)
}

/// RFC 2396 `L_URIC` (`reserved | unreserved` + non-ASCII `other` in `java.net.URI`).
fn is_java_uri_uric_char(ch: char) -> bool {
    is_java_uri_path_char(ch) || matches!(ch, '?' | '[' | ']')
}

/// Decodes `%XX` sequences in a URI component into UTF-8.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Parsed URI components matching `java.net.URI` (hierarchical or opaque).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedUri {
    pub scheme: Option<String>,
    pub opaque: bool,
    pub authority: Option<String>,
    /// Raw (still percent-encoded) path.
    pub raw_path: String,
    pub raw_query: Option<String>,
    pub raw_fragment: Option<String>,
}

impl ParsedUri {
    pub fn parse(raw: &str) -> Self {
        let (before_frag, raw_fragment) = match raw.find('#') {
            Some(i) => (&raw[..i], Some(raw[i + 1..].to_string())),
            None => (raw, None),
        };

        // Parse scheme: alpha *(alpha / digit / '+' / '-' / '.') ':'
        let (scheme, rest) = if let Some(colon) = before_frag.find(':') {
            let cand = &before_frag[..colon];
            let valid = !cand.is_empty()
                && cand.as_bytes()[0].is_ascii_alphabetic()
                && cand
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'));
            if valid {
                (Some(cand.to_string()), &before_frag[colon + 1..])
            } else {
                (None, before_frag)
            }
        } else {
            (None, before_frag)
        };

        if scheme.is_some() && !rest.starts_with('/') {
            return Self {
                scheme,
                opaque: true,
                authority: None,
                raw_path: rest.to_string(),
                raw_query: None,
                raw_fragment,
            };
        }

        let (before_query, raw_query) = match rest.find('?') {
            Some(i) => (&rest[..i], Some(rest[i + 1..].to_string())),
            None => (rest, None),
        };

        let (authority, raw_path) = if let Some(after_slashes) = before_query.strip_prefix("//") {
            match after_slashes.find('/') {
                Some(i) => (
                    Some(after_slashes[..i].to_string()),
                    after_slashes[i..].to_string(),
                ),
                None => (Some(after_slashes.to_string()), String::new()),
            }
        } else {
            (None, before_query.to_string())
        };

        Self {
            scheme,
            opaque: false,
            authority,
            raw_path,
            raw_query,
            raw_fragment,
        }
    }

    pub fn decoded_path(&self) -> String {
        percent_decode(&self.raw_path)
    }

    pub fn decoded_query(&self) -> Option<String> {
        self.raw_query.as_deref().map(percent_decode)
    }

    pub fn decoded_fragment(&self) -> Option<String> {
        self.raw_fragment.as_deref().map(percent_decode)
    }

    pub fn normalize_path(path: &str) -> String {
        if path.is_empty() {
            return String::new();
        }
        let leading_slash = path.starts_with('/');
        let trailing_slash = path.ends_with('/');
        let mut stack: Vec<&str> = Vec::new();
        for seg in path.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    if let Some(&last) = stack.last()
                        && last != ".."
                    {
                        stack.pop();
                    } else if !leading_slash {
                        stack.push("..");
                    }
                }
                other => stack.push(other),
            }
        }
        let mut res = String::new();
        if leading_slash {
            res.push('/');
        }
        res.push_str(&stack.join("/"));
        if trailing_slash && !res.ends_with('/') {
            res.push('/');
        }
        res
    }

    pub fn to_uri_string(&self) -> String {
        let mut out = String::new();
        if let Some(ref s) = self.scheme {
            out.push_str(s);
            out.push(':');
        }
        if self.opaque {
            out.push_str(&self.raw_path);
        } else {
            if let Some(ref a) = self.authority {
                out.push_str("//");
                out.push_str(a);
            }
            out.push_str(&self.raw_path);
            if let Some(ref q) = self.raw_query {
                out.push('?');
                out.push_str(q);
            }
        }
        if let Some(ref f) = self.raw_fragment {
            out.push('#');
            out.push_str(f);
        }
        out
    }
}

/// Mirrors `java.net.URI.resolve(String)` (RFC 2396).
pub fn resolve_uri(base: &str, rel: &str) -> String {
    if rel.is_empty() {
        return base.to_string();
    }
    let b = ParsedUri::parse(base);
    let r = ParsedUri::parse(rel);
    if b.opaque || r.scheme.is_some() {
        return rel.to_string();
    }
    // Fragment-only relative URI
    if r.authority.is_none() && r.raw_path.is_empty() && r.raw_query.is_none() {
        let resolved = ParsedUri {
            scheme: b.scheme,
            opaque: false,
            authority: b.authority,
            raw_path: b.raw_path,
            raw_query: b.raw_query,
            raw_fragment: r.raw_fragment,
        };
        return resolved.to_uri_string();
    }
    let (authority, raw_path, raw_query) = if r.authority.is_some() {
        (r.authority, r.raw_path, r.raw_query)
    } else if r.raw_path.starts_with('/') {
        (
            b.authority,
            ParsedUri::normalize_path(&r.raw_path),
            r.raw_query,
        )
    } else if r.raw_path.is_empty() {
        (b.authority, b.raw_path, r.raw_query.or(b.raw_query))
    } else {
        let dir = match b.raw_path.rfind('/') {
            Some(i) => &b.raw_path[..=i],
            None => "",
        };
        let joined = format!("{dir}{}", r.raw_path);
        (b.authority, ParsedUri::normalize_path(&joined), r.raw_query)
    };
    ParsedUri {
        scheme: b.scheme,
        opaque: false,
        authority,
        raw_path,
        raw_query,
        raw_fragment: r.raw_fragment,
    }
    .to_uri_string()
}

/// Mirrors `IoUtils.relativize(URI uri, URI base)` (`IoUtils.java:488-537`) byte-for-byte.
pub fn relativize_uri(uri_str: &str, base_str: &str) -> String {
    let uri = ParsedUri::parse(uri_str);
    let base = ParsedUri::parse(base_str);
    if uri.opaque || base.opaque || uri.scheme != base.scheme || uri.authority != base.authority {
        return uri_str.to_string();
    }
    let uri_norm_raw = ParsedUri::normalize_path(&uri.raw_path);
    let base_norm_raw = ParsedUri::normalize_path(&base.raw_path);
    let uri_path = percent_decode(&uri_norm_raw);
    let base_path = percent_decode(&base_norm_raw);
    if base_path.is_empty() {
        return uri_str.to_string();
    }
    let uri_parts = java_split_slash(&uri_path);
    let mut base_parts = java_split_slash(&base_path);
    if !base_path.ends_with('/') && !base_parts.is_empty() {
        base_parts.pop();
    }
    let query_part = uri.decoded_query();
    let frag_part = uri.decoded_fragment();
    let format_rel = |path: &str| -> String {
        let mut out = java_uri_encode_path(path);
        if let Some(ref q) = query_part {
            out.push('?');
            out.push_str(&java_uri_encode_fragment(q));
        }
        if let Some(ref f) = frag_part {
            out.push('#');
            out.push_str(&java_uri_encode_fragment(f));
        }
        out
    };

    if uri_parts == base_parts {
        return format_rel("");
    }
    let mut start = 0;
    while start < uri_parts.len().min(base_parts.len()) {
        if uri_parts[start] != base_parts[start] {
            break;
        }
        start += 1;
    }
    let uri_rem = &uri_parts[start..];
    let base_rem = &base_parts[start..];
    if base_rem.is_empty() {
        return format_rel(&uri_rem.join("/"));
    }
    let resulting_path = format!("{}{}", "../".repeat(base_rem.len()), uri_rem.join("/"));
    format_rel(&resulting_path)
}

/// Mirrors Java `String.split("/")` (which drops trailing empty strings).
fn java_split_slash(s: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = s.split('/').collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Mirrors `IoUtils.ensurePathEndsWithSlash(Path.toUri())` for an output directory.
pub fn dir_to_file_uri_with_slash(dir: &Path) -> String {
    let raw = pkl_host_native::path_to_file_uri(dir).to_java_utf8();
    if raw.ends_with('/') {
        raw
    } else {
        format!("{raw}/")
    }
}

/// Converts a `file://` URI string back to a `PathBuf` (mirroring `URI.toPath()`).
pub fn file_uri_to_path_buf(uri: &str) -> Option<std::path::PathBuf> {
    let pkl_uri = pkl_runtime::value::PklString::from(uri);
    pkl_host_native::file_uri_to_path(&pkl_uri).ok()
}

/// Mirrors `Util.kt` `getModulePath(moduleName, packagePrefix)`.
pub fn get_module_path(module_name: &str, package_prefix: &str) -> String {
    let suffix = module_name
        .get(package_prefix.len()..)
        .unwrap_or(module_name);
    suffix.replace('.', "/")
}

/// Mirrors `Util.kt` `String.replaceSourceCodePlaceholders`.
pub fn replace_source_code_placeholders(
    scheme: &str,
    path: &str,
    start_line: u32,
    end_line: u32,
) -> String {
    scheme
        .replace("%{path}", path)
        .replace("%{line}", &start_line.to_string())
        .replace("%{endLine}", &end_line.to_string())
}

/// Mirrors `Lexer.maybeQuoteIdentifier(String)` (`Lexer.java:781-835`).
pub fn maybe_quote_identifier(ident: &str) -> String {
    if is_regular_identifier(ident) {
        ident.to_string()
    } else {
        format!("`{ident}`")
    }
}

/// Mirrors `Util.kt` `String.asModuleName`.
pub fn as_module_name(name: &str) -> String {
    name.split('.')
        .map(maybe_quote_identifier)
        .collect::<Vec<_>>()
        .join(".")
}

pub fn is_regular_identifier(ident: &str) -> bool {
    if ident.is_empty() || is_pkl_keyword(ident) {
        return false;
    }
    let mut chars = ident.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '$' || first == '_' || is_unicode_identifier_start(first)) {
        return false;
    }
    chars.all(|c| c == '$' || is_unicode_identifier_part(c))
}

fn is_unicode_identifier_start(ch: char) -> bool {
    // Matches Java `Character.isUnicodeIdentifierStart` for practical Pkl identifiers.
    ch.is_alphabetic()
}

fn is_unicode_identifier_part(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '\u{200C}' || ch == '\u{200D}'
}

fn is_pkl_keyword(s: &str) -> bool {
    matches!(
        s,
        "_" | "abstract"
            | "amends"
            | "as"
            | "case"
            | "class"
            | "const"
            | "delete"
            | "else"
            | "extends"
            | "external"
            | "false"
            | "fixed"
            | "for"
            | "function"
            | "hidden"
            | "if"
            | "import"
            | "import*"
            | "in"
            | "is"
            | "let"
            | "local"
            | "module"
            | "new"
            | "nothing"
            | "null"
            | "open"
            | "out"
            | "outer"
            | "override"
            | "protected"
            | "read"
            | "read*"
            | "read?"
            | "record"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "trace"
            | "true"
            | "typealias"
            | "unknown"
            | "vararg"
            | "when"
    )
}

/// Finds the first paragraph separator matching `Util.kt:70`:
/// `Regex("(?m:^\\s*(`{3,}\\w*)?\\s*\n)")`.
/// Returns `(match_start, match_end, has_code_fence_group)` where `match_end` is exclusive.
fn find_paragraph_separator(doc: &str) -> Option<(usize, usize, bool)> {
    let bytes = doc.as_bytes();
    let mut line_start = 0usize;
    while line_start < bytes.len() {
        let line_end = line_start + doc[line_start..].find('\n')?;
        let line = &doc[line_start..line_end];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Some((line_start, line_end + 1, false));
        }
        // Check ```` ```\w* ```` (3 or more backticks followed by 0+ word chars `[A-Za-z0-9_]`)
        let backticks = trimmed.bytes().take_while(|&b| b == b'`').count();
        if backticks >= 3
            && trimmed[backticks..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Some((line_start, line_end + 1, true));
        }
        line_start = line_end + 1;
    }
    None
}

/// Mirrors `Util.kt:72-79` `getDocCommentSummary`.
pub fn get_doc_comment_summary(doc_comment: Option<&str>) -> Option<String> {
    let doc = doc_comment?;
    let slice = match find_paragraph_separator(doc) {
        None => doc.trim(),
        Some((start, _, _)) => doc[..start].trim(),
    };
    if slice.is_empty() {
        None
    } else {
        Some(slice.to_string())
    }
}

/// Mirrors `Util.kt:81-91` `getDocCommentOverflow`.
pub fn get_doc_comment_overflow(doc_comment: Option<&str>) -> Option<String> {
    let doc = doc_comment?;
    let (start, end, has_fence) = find_paragraph_separator(doc)?;
    let idx = if has_fence { start } else { end };
    let slice = doc[idx..].trim();
    if slice.is_empty() {
        None
    } else {
        Some(slice.to_string())
    }
}

/// Parses a Pkl semver version string (`Version.parseOrNull`).
pub fn parse_semver_or_none(v: &str) -> Option<PackageVersion> {
    PackageVersion::parse(v).ok()
}

/// Compares two Pkl semver strings in ascending order (`Version.parse(v1).compareTo(Version.parse(v2))`).
pub fn compare_versions_asc(v1: &str, v2: &str) -> Ordering {
    match (PackageVersion::parse(v1), PackageVersion::parse(v2)) {
        (Ok(p1), Ok(p2)) => p1.cmp(&p2),
        _ => v1.cmp(v2),
    }
}

/// Compares two Pkl semver strings in descending order (for `descendingVersionComparator`).
pub fn compare_versions_desc(v1: &str, v2: &str) -> Ordering {
    compare_versions_asc(v1, v2).reverse()
}

/// Mirrors Kotlin `String.trimIndent()` used in `PageGenerator.kt:825`.
pub fn kotlin_trim_indent(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
        .min()
        .unwrap_or(0);
    let mut start = 0;
    let mut end = lines.len();
    if lines[0].trim().is_empty() {
        start = 1;
    }
    if end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    lines[start..end]
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else {
                let drop_bytes = line
                    .char_indices()
                    .nth(min_indent)
                    .map_or(line.len(), |(i, _)| i);
                &line[drop_bytes..]
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_path_matches_java() {
        assert_eq!(encode_path("localhost:0/birds"), "localhost(3a)0/birds");
        assert_eq!(encode_path("a(b)c"), "a((b)c");
        assert_eq!(encode_path("Class Two {}"), "Class Two {}");
    }

    #[test]
    fn uri_encoding_matches_java() {
        assert_eq!(
            uri_encode_component("localhost:0/birds"),
            "localhost:0%2Fbirds"
        );
        assert_eq!(
            uri_encode_component("property {} <> four"),
            "property%20%7B%7D%20%3C%3E%20four"
        );
        assert_eq!(
            uri_encode_component("ifNonNull().Result"),
            "ifNonNull().Result"
        );
        assert_eq!(
            uri_encode("Module Containing Spaces/index.html"),
            "Module%20Containing%20Spaces/index.html"
        );
    }

    #[test]
    fn relativize_uri_matches_java() {
        assert_eq!(
            relativize_uri(
                "file:///out/data/com.package1/_/index.json",
                "file:///out/com.package1/1.2.3/index.html"
            ),
            "../../data/com.package1/_/index.json"
        );
        assert_eq!(
            relativize_uri(
                "file:///out/com.package1/1.2.3/docLinks/index.html#age",
                "file:///out/com.package1/1.2.3/docLinks/index.html"
            ),
            "index.html#age"
        );
        assert_eq!(
            relativize_uri(
                "https://example.com/docs/a.html",
                "file:///out/com.package1/1.2.3/index.html"
            ),
            "https://example.com/docs/a.html"
        );
        assert_eq!(
            relativize_uri(
                "com.package1/1.2.3/moduleTypes2/Foo.html",
                "com.package1/1.2.3/moduleTypes1/index.html"
            ),
            "../moduleTypes2/Foo.html"
        );
    }

    #[test]
    fn paragraph_split_matches_util_kt() {
        let doc = "First para.\n```pkl\ncode\n```";
        assert_eq!(
            get_doc_comment_summary(Some(doc)).as_deref(),
            Some("First para.")
        );
        assert_eq!(
            get_doc_comment_overflow(Some(doc)).as_deref(),
            Some("```pkl\ncode\n```")
        );
        let doc2 = "Summary line 1.\nSummary line 2.\n\nOverflow.";
        assert_eq!(
            get_doc_comment_summary(Some(doc2)).as_deref(),
            Some("Summary line 1.\nSummary line 2.")
        );
        assert_eq!(
            get_doc_comment_overflow(Some(doc2)).as_deref(),
            Some("Overflow.")
        );
    }

    #[test]
    fn security_helpers_block_traversal_xss_and_unsafe_hrefs() {
        let base = std::env::temp_dir().join(format!("pkl-doc-sec-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        assert!(join_confined_output_path(&base, "pkg/1.0.0/index.html").is_ok());
        assert!(join_confined_output_path(&base, "../escape.html").is_err());
        assert!(join_confined_output_path(&base, "/etc/passwd").is_err());
        assert!(join_confined_output_path(&base, "a\\b.html").is_err());
        assert!(join_confined_output_path(&base, "a\0b.html").is_err());

        let mut script_str = String::new();
        push_safe_inline_script_json_string(&mut script_str, "</script><img src=x>\u{2028}");
        assert!(!script_str.contains("</script>"));
        assert!(script_str.contains("\\u003c/script\\u003e"));
        assert!(script_str.contains("\\u2028"));

        assert_eq!(
            sanitize_href_url("https://pkl-lang.org"),
            "https://pkl-lang.org"
        );
        assert_eq!(sanitize_href_url("../Foo.html#bar"), "../Foo.html#bar");
        assert_eq!(sanitize_href_url("javascript:alert(1)"), "");
        assert_eq!(sanitize_href_url(" \tJaVa\tScRiPt:alert(1)"), "");
        assert_eq!(sanitize_href_url("vbscript:msgbox(1)"), "");
        assert_eq!(
            sanitize_href_url("data:text/html,<script>alert(1)</script>"),
            ""
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
