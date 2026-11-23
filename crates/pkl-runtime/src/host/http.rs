//! Wasm-pure HTTP policy, rewrite, header matching, proxy/noProxy, redirect loop,
//! and `HttpTransport` abstraction matching Pkl 0.32.0 `org.pkl.core.http.*`.
//!
//! No sockets, TLS, or OS I/O live here; platform adapters (e.g. `pkl-host-native`)
//! supply a single-hop non-redirecting `HttpTransport`.

use crate::host::errors::{IoFailure, ReadFailure};
use crate::java_regex::{Limits, Utf16Regex};
use crate::modules::ModuleHostError;
use crate::value::PklString;
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Default redirect limit matching `RequestRewritingClient.MAX_HTTP_REDIRECTS`.
pub const DEFAULT_MAX_REDIRECTS: usize = 20;

/// Default User-Agent identifying `pkl-rs` (`pkl-rs/<ver> (<os>; <flavor>)`).
pub const DEFAULT_USER_AGENT: &str = if cfg!(target_os = "macos") {
    "pkl-rs/0.32.0 (macOS; native)"
} else if cfg!(target_os = "linux") {
    "pkl-rs/0.32.0 (Linux; native)"
} else if cfg!(target_os = "windows") {
    "pkl-rs/0.32.0 (Windows; native)"
} else if cfg!(target_arch = "wasm32") {
    "pkl-rs/0.32.0 (wasm; wasm)"
} else {
    "pkl-rs/0.32.0 (unknown; native)"
};

const RESERVED_HEADER_NAMES: &[&str] = &[
    "connection",
    "keep-alive",
    "content-length",
    "expect",
    "host",
    "upgrade",
    "te",
    "transfer-encoding",
    "trailer",
];

const RESERVED_HEADER_PREFIXES: &[&str] = &["proxy-", "sec-"];

/// Parsed `http:` / `https:` URI components used for rewrites, proxy selection, and redirects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedHttpUri {
    pub raw: String,
    pub scheme: String,
    pub user_info: Option<String>,
    /// Host without IPv6 brackets when IPv6, or lowercase domain/IPv4 as authored.
    pub host: String,
    /// Raw host slice as authored in URI (keeps `[...]` for IPv6).
    pub raw_host: String,
    pub port: Option<u16>,
    /// Path starting with `/` (or empty if omitted).
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl ParsedHttpUri {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let Some((scheme_raw, rest)) = raw.split_once("://") else {
            return Err(format!(
                "Expected `http://` or `https://` URI, got `{raw}`."
            ));
        };
        let scheme = scheme_raw.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(format!(
                "Expected `http` or `https` scheme, got `{scheme_raw}` in `{raw}`."
            ));
        }
        let (without_frag, fragment) = match rest.split_once('#') {
            Some((before, frag)) => (before, Some(frag.to_string())),
            None => (rest, None),
        };
        let (without_query, query) = match without_frag.split_once('?') {
            Some((before, q)) => (before, Some(q.to_string())),
            None => (without_frag, None),
        };
        let (authority, path) = match without_query.find('/') {
            Some(idx) => (&without_query[..idx], without_query[idx..].to_string()),
            None => (without_query, String::new()),
        };
        if authority.is_empty() {
            return Err(format!("Missing host in HTTP URI `{raw}`."));
        }
        let (user_info, host_port) = match authority.rsplit_once('@') {
            Some((ui, hp)) => (Some(ui.to_string()), hp),
            None => (None, authority),
        };
        if host_port.is_empty() {
            return Err(format!("Missing host in HTTP URI `{raw}`."));
        }
        let (raw_host, host, port) = if host_port.starts_with('[') {
            let Some(end) = host_port.find(']') else {
                return Err(format!("Unclosed IPv6 host literal in `{raw}`."));
            };
            let bracketed = &host_port[..=end];
            let inner = &host_port[1..end];
            let tail = &host_port[end + 1..];
            let port = if tail.is_empty() {
                None
            } else if let Some(port_str) = tail.strip_prefix(':') {
                if port_str.is_empty() {
                    None
                } else {
                    let parsed = port_str
                        .parse::<u16>()
                        .map_err(|_| format!("Invalid port `{port_str}` in `{raw}`."))?;
                    Some(parsed)
                }
            } else {
                return Err(format!("Invalid host/port syntax in `{raw}`."));
            };
            (bracketed.to_string(), inner.to_string(), port)
        } else if let Some((h, port_str)) = host_port.rsplit_once(':') {
            if h.is_empty() {
                return Err(format!("Missing host in HTTP URI `{raw}`."));
            }
            let port = if port_str.is_empty() {
                None
            } else {
                let parsed = port_str
                    .parse::<u16>()
                    .map_err(|_| format!("Invalid port `{port_str}` in `{raw}`."))?;
                Some(parsed)
            };
            (h.to_string(), h.to_string(), port)
        } else {
            (host_port.to_string(), host_port.to_string(), None)
        };
        if host.is_empty() {
            return Err(format!("Missing host in HTTP URI `{raw}`."));
        }
        Ok(Self {
            raw: raw.to_string(),
            scheme,
            user_info,
            host,
            raw_host,
            port,
            path,
            query,
            fragment,
        })
    }

    pub fn effective_port(&self) -> u16 {
        self.port
            .unwrap_or_else(|| if self.scheme == "https" { 443 } else { 80 })
    }

    /// Formats the `Host` header value (omitting default ports 80/443).
    pub fn host_header(&self) -> String {
        let default_port = if self.scheme == "https" { 443 } else { 80 };
        match self.port {
            Some(p) if p != default_port => format!("{}:{p}", self.raw_host),
            _ => self.raw_host.clone(),
        }
    }

    /// Formats the HTTP/1.1 origin-form request target (`/path?query`).
    pub fn request_target(&self) -> String {
        let base = if self.path.is_empty() {
            "/"
        } else {
            self.path.as_str()
        };
        match &self.query {
            Some(q) => format!("{base}?{q}"),
            None => base.to_string(),
        }
    }

    /// Formats the HTTP/1.1 absolute-form target (for plain HTTP proxy requests, without fragment).
    pub fn absolute_form_target(&self) -> String {
        let mut out = format!("{}://", self.scheme);
        if let Some(ui) = &self.user_info {
            out.push_str(ui);
            out.push('@');
        }
        out.push_str(&self.raw_host);
        if let Some(port) = self.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
        if self.path.is_empty() {
            out.push('/');
        } else {
            out.push_str(&self.path);
        }
        if let Some(q) = &self.query {
            out.push('?');
            out.push_str(q);
        }
        out
    }

    /// Lowercases scheme and host while preserving path/query/fragment case.
    pub fn normalized_scheme_and_host(&self) -> String {
        let mut out = format!("{}://", self.scheme);
        if let Some(ui) = &self.user_info {
            out.push_str(ui);
            out.push('@');
        }
        out.push_str(&self.raw_host.to_ascii_lowercase());
        if let Some(port) = self.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
        out.push_str(&self.path);
        if let Some(q) = &self.query {
            out.push('?');
            out.push_str(q);
        }
        if let Some(f) = &self.fragment {
            out.push('#');
            out.push_str(f);
        }
        out
    }

    /// Replaces port `0` with `test_port` (matching `RequestRewritingClient.replaceTestPort`).
    pub fn with_port(&self, new_port: u16) -> String {
        let mut out = format!("{}://", self.scheme);
        if let Some(ui) = &self.user_info {
            out.push_str(ui);
            out.push('@');
        }
        out.push_str(&self.raw_host);
        out.push(':');
        out.push_str(&new_port.to_string());
        out.push_str(&self.path);
        if let Some(q) = &self.query {
            out.push('?');
            out.push_str(q);
        }
        if let Some(f) = &self.fragment {
            out.push('#');
            out.push_str(f);
        }
        out
    }

    /// Resolves a `Location` redirect target relative to this URI (RFC 3986 / Java `URI.resolve`).
    pub fn resolve_redirect(&self, location: &str) -> Result<String, String> {
        let trimmed = location.trim();
        if trimmed.is_empty() {
            return Err("Empty Location header".into());
        }
        // Reject raw whitespace or control characters matching Java URI.resolve IllegalArgumentException.
        if trimmed.bytes().any(|b| {
            b <= 0x20
                || b == 0x7F
                || matches!(
                    b,
                    b'<' | b'>' | b'"' | b'{' | b'}' | b'|' | b'\\' | b'^' | b'`'
                )
        }) {
            return Err(format!("Illegal character in redirect URI: {trimmed}"));
        }

        // Absolute URI with scheme?
        if let Some((scheme, _)) = trimmed.split_once(':')
            && !scheme.is_empty()
            && scheme.as_bytes()[0].is_ascii_alphabetic()
            && scheme
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        {
            // Validate as HTTP/HTTPS or general URI
            if trimmed.contains("://") {
                return Ok(trimmed.to_string());
            }
            return Err(format!("Opaque redirect URI not supported: {trimmed}"));
        }

        // Scheme-relative (`//host/path`)
        if let Some(rest) = trimmed.strip_prefix("//") {
            return Ok(format!("{}://{rest}", self.scheme));
        }

        let (loc_before_frag, loc_frag) = match trimmed.split_once('#') {
            Some((b, f)) => (b, Some(f)),
            None => (trimmed, None),
        };
        let (loc_path, loc_query) = match loc_before_frag.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (loc_before_frag, None),
        };

        let resolved_path = if loc_path.is_empty() {
            self.path.clone()
        } else if loc_path.starts_with('/') {
            remove_dot_segments(loc_path)
        } else {
            let base_dir = match self.path.rfind('/') {
                Some(idx) => &self.path[..=idx],
                None => "/",
            };
            remove_dot_segments(&format!("{base_dir}{loc_path}"))
        };

        let resolved_query = if !loc_path.is_empty() || loc_query.is_some() {
            loc_query.map(ToString::to_string)
        } else {
            self.query.clone()
        };

        let mut out = format!("{}://", self.scheme);
        if let Some(ui) = &self.user_info {
            out.push_str(ui);
            out.push('@');
        }
        out.push_str(&self.raw_host);
        if let Some(port) = self.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
        out.push_str(&resolved_path);
        if let Some(q) = resolved_query {
            out.push('?');
            out.push_str(&q);
        }
        if let Some(f) = loc_frag {
            out.push('#');
            out.push_str(f);
        }
        Ok(out)
    }
}

fn remove_dot_segments(path: &str) -> String {
    let trailing_slash = path.ends_with('/') || path.ends_with("/.") || path.ends_with("/..");
    let mut segs: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            other => segs.push(other),
        }
    }
    if segs.is_empty() {
        "/".to_string()
    } else if trailing_slash {
        format!("/{}/", segs.join("/"))
    } else {
        format!("/{}", segs.join("/"))
    }
}

/// Validates an HTTP rewrite source or target rule (`IoUtils.validateRewriteRule`).
pub fn validate_rewrite_rule(uri: &str) -> Result<String, String> {
    let parsed = ParsedHttpUri::parse(uri).map_err(|_| {
        format!("Rewrite rule must start with 'http://' or 'https://', but was '{uri}'")
    })?;
    if !uri.ends_with('/') {
        return Err(format!("Rewrite rule must end with '/', but was '{uri}'"));
    }
    Ok(parsed.normalized_scheme_and_host())
}

/// Validates an HTTP header name (`IoUtils.validateHeaderName`).
pub fn validate_header_name(name: &str) -> Result<(), String> {
    let lower = name.to_ascii_lowercase();
    if RESERVED_HEADER_NAMES.contains(&lower.as_str()) {
        return Err(format!("HTTP header `{name}` is a reserved header."));
    }
    if RESERVED_HEADER_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return Err(format!(
            "HTTP header `{name}` starts with a reserved header prefix."
        ));
    }
    if name.is_empty()
        || !name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
    {
        return Err(format!("HTTP header `{name}` has invalid syntax."));
    }
    Ok(())
}

/// Validates an HTTP header value (`IoUtils.validateHeaderValue`).
pub fn validate_header_value(value: &str) -> Result<(), String> {
    let valid_chars = value.chars().all(|c| {
        c == '\t'
            || ('\u{0020}'..='\u{007E}').contains(&c)
            || ('\u{0080}'..='\u{00FF}').contains(&c)
    });
    if !valid_chars {
        return Err(format!("HTTP header value `{value}` has invalid syntax."));
    }
    let utf16_len: usize = value.encode_utf16().count();
    if utf16_len > 4096 {
        return Err(format!(
            "HTTP Header value is invalid because it is longer than 4096 characters. Value: `{value}`"
        ));
    }
    Ok(())
}

/// Compiles a Pkl HTTP header URL glob pattern (equivalent to `GlobResolver.toRegexPattern`).
#[derive(Clone, Debug)]
pub struct HeaderGlobMatcher {
    pattern: String,
    match_all: bool,
    regex: Option<Arc<Utf16Regex>>,
}

impl HeaderGlobMatcher {
    pub fn compile(pattern: &str) -> Result<Self, String> {
        if pattern == "**" {
            return Ok(Self {
                pattern: pattern.to_string(),
                match_all: true,
                regex: None,
            });
        }
        let regex_str = glob_to_regex_for_url(pattern)?;
        let units: Vec<u16> = regex_str.encode_utf16().collect();
        let regex = crate::regex::compile_utf16(&units, "")
            .map_err(|e| format!("Invalid glob pattern `{pattern}`: {}", e.message))?;
        Ok(Self {
            pattern: pattern.to_string(),
            match_all: false,
            regex: Some(Arc::new(regex)),
        })
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    pub fn matches(&self, uri: &str) -> bool {
        if self.match_all {
            return true;
        }
        let Some(regex) = &self.regex else {
            return false;
        };
        let units: Vec<u16> = uri.encode_utf16().collect();
        regex
            .captures_entire(&units, Limits::default())
            .is_ok_and(|m| m.is_some())
    }
}

fn glob_to_regex_for_url(glob: &str) -> Result<String, String> {
    let chars: Vec<char> = glob.chars().collect();
    let mut out = String::with_capacity(glob.len() * 2);
    let mut i = 0;
    let mut in_brace = false;
    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '\\' => {
                if i + 1 >= chars.len() {
                    return Err(format!(
                        "Invalid glob pattern `{glob}`: trailing backslash."
                    ));
                }
                i += 1;
                push_regex_escaped(&mut out, chars[i]);
            }
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    while i + 1 < chars.len() && chars[i + 1] == '*' {
                        i += 1;
                    }
                    out.push_str(".*");
                } else {
                    out.push_str("[^/]*");
                }
            }
            '?' => out.push_str("[^/]"),
            '[' => {
                i += 1;
                if i >= chars.len() {
                    return Err(format!(
                        "Invalid glob pattern `{glob}`: Missing `]` to terminate character class."
                    ));
                }
                out.push('[');
                if chars[i] == '!' {
                    out.push('^');
                    i += 1;
                }
                let mut closed = false;
                while i < chars.len() {
                    if chars[i] == ']' {
                        out.push(']');
                        closed = true;
                        break;
                    }
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        push_regex_escaped(&mut out, chars[i]);
                    } else {
                        out.push(chars[i]);
                    }
                    i += 1;
                }
                if !closed {
                    return Err(format!(
                        "Invalid glob pattern `{glob}`: Missing `]` to terminate character class."
                    ));
                }
            }
            '{' => {
                if in_brace {
                    return Err(format!(
                        "Invalid glob pattern `{glob}`: Sub-patterns cannot be nested."
                    ));
                }
                in_brace = true;
                out.push_str("(?:");
            }
            '}' => {
                if !in_brace {
                    out.push_str("\\}");
                } else {
                    in_brace = false;
                    out.push(')');
                }
            }
            ',' if in_brace => out.push('|'),
            other => push_regex_escaped(&mut out, other),
        }
        i += 1;
    }
    if in_brace {
        return Err(format!(
            "Invalid glob pattern `{glob}`: Missing `}}` character to terminate sub-pattern."
        ));
    }
    Ok(out)
}

fn push_regex_escaped(out: &mut String, ch: char) {
    if matches!(
        ch,
        '\\' | '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
    ) {
        out.push('\\');
    }
    out.push(ch);
}

/// Validated HTTP proxy address (`ProxySelector.java:28-78`): `http://<host>[:<port>]` only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyAddress {
    pub raw: String,
    pub host: String,
    pub raw_host: String,
    pub port: u16,
}

impl ProxyAddress {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let malformed =
            || format!("Malformed proxy URI (expecting `http://<host>[:<port>]`): `{raw}`.");
        let parsed = ParsedHttpUri::parse(raw).map_err(|_| malformed())?;
        if parsed.scheme != "http"
            || parsed.user_info.is_some()
            || !parsed.path.is_empty()
            || parsed.query.is_some()
            || parsed.fragment.is_some()
        {
            return Err(malformed());
        }
        let port = parsed.port.unwrap_or(80);
        if port == 0 {
            return Err(malformed());
        }
        Ok(Self {
            raw: raw.to_string(),
            host: parsed.host,
            raw_host: parsed.raw_host,
            port,
        })
    }
}

/// Parsed `http.proxy.noProxy` entry matching `org.pkl.core.http.NoProxyRule`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoProxyRule {
    Wildcard,
    Hostname {
        /// Normalized lowercase hostname without leading dot.
        host: String,
        port: Option<u16>,
    },
    Ipv4Cidr {
        network: u32,
        prefix_len: u8,
        port: Option<u16>,
    },
    Ipv6Cidr {
        network: u128,
        prefix_len: u8,
        port: Option<u16>,
    },
}

impl NoProxyRule {
    pub fn parse(raw: &str) -> Result<Option<Self>, String> {
        let s = raw.trim();
        if s.is_empty() {
            return Ok(None);
        }
        if s == "*" {
            return Ok(Some(Self::Wildcard));
        }

        // Handle IPv4/IPv6 CIDR rules with optional `:port` matching `NoProxyRule.java:40-98`:
        // - `10.0.0.0/8`, `10.0.0.0/8:8080`
        // - `fe80::/10`, `[fe80::/10]:8080`, `[fe80::]/10:8080`
        // Invalid CIDR prefix lengths (>32 / >128) fall back to hostname matching (`NoProxyRule.java:69-72,87-91`).
        if s.contains('/') {
            let (base_with_cidr, trailing_port) = split_cidr_trailing_port(s);
            if let Some((ip_part, prefix_str)) = base_with_cidr.split_once('/') {
                let ip_clean = ip_part
                    .strip_prefix('[')
                    .unwrap_or(ip_part)
                    .strip_suffix(']')
                    .unwrap_or(ip_part.strip_prefix('[').unwrap_or(ip_part));
                let prefix_clean = prefix_str.strip_suffix(']').unwrap_or(prefix_str);
                if let Ok(v4) = ip_clean.parse::<Ipv4Addr>() {
                    if let Ok(prefix_len) = prefix_clean.parse::<u32>() {
                        if prefix_len <= 32 {
                            let prefix_u8 = prefix_len as u8;
                            let mask = ipv4_mask(prefix_u8);
                            return Ok(Some(Self::Ipv4Cidr {
                                network: u32::from(v4) & mask,
                                prefix_len: prefix_u8,
                                port: trailing_port,
                            }));
                        }
                        // Best-effort fallback to hostname rule (`NoProxyRule.java:69-72`).
                        return Ok(Some(Self::Hostname {
                            host: s.to_ascii_lowercase(),
                            port: trailing_port,
                        }));
                    }
                } else if let Ok(v6) = ip_clean.parse::<Ipv6Addr>()
                    && let Ok(prefix_len) = prefix_clean.parse::<u32>()
                {
                    if prefix_len <= 128 {
                        let prefix_u8 = prefix_len as u8;
                        let mask = ipv6_mask(prefix_u8);
                        return Ok(Some(Self::Ipv6Cidr {
                            network: u128::from(v6) & mask,
                            prefix_len: prefix_u8,
                            port: trailing_port,
                        }));
                    }
                    // Best-effort fallback to hostname rule (`NoProxyRule.java:87-91`).
                    return Ok(Some(Self::Hostname {
                        host: s.to_ascii_lowercase(),
                        port: trailing_port,
                    }));
                }
            }
            // Fall through to hostname parsing (best-effort matching `NoProxyRule.java:99-107`).
        }

        // Split optional `:port`, taking care of bracketed `[::1]:8080` and bare IPv6 `::1`.
        let (host_part, port) = if let Some(rest) = s.strip_prefix('[') {
            let Some(end) = rest.find(']') else {
                return Err(format!("Unclosed IPv6 bracket in noProxy rule `{raw}`."));
            };
            let inner = &rest[..end];
            let tail = &rest[end + 1..];
            let port = if tail.is_empty() {
                None
            } else if let Some(p) = tail.strip_prefix(':') {
                Some(
                    p.parse::<u16>()
                        .map_err(|_| format!("Invalid port in noProxy rule `{raw}`."))?,
                )
            } else {
                return Err(format!("Invalid noProxy rule `{raw}`."));
            };
            (inner, port)
        } else if s.parse::<Ipv6Addr>().is_ok() {
            (s, None)
        } else if let Some((h, p)) = s.rsplit_once(':') {
            let parsed_port = p
                .parse::<u16>()
                .map_err(|_| format!("Invalid port in noProxy rule `{raw}`."))?;
            (h, Some(parsed_port))
        } else {
            (s, None)
        };

        if let Ok(v4) = host_part.parse::<Ipv4Addr>() {
            return Ok(Some(Self::Ipv4Cidr {
                network: u32::from(v4),
                prefix_len: 32,
                port,
            }));
        }
        if let Ok(v6) = host_part.parse::<Ipv6Addr>() {
            return Ok(Some(Self::Ipv6Cidr {
                network: u128::from(v6),
                prefix_len: 128,
                port,
            }));
        }

        let normalized = host_part
            .strip_prefix('.')
            .unwrap_or(host_part)
            .to_ascii_lowercase();
        if normalized.is_empty() {
            return Err(format!("Empty host in noProxy rule `{raw}`."));
        }
        Ok(Some(Self::Hostname {
            host: normalized,
            port,
        }))
    }

    pub fn matches(&self, target_host: &str, target_port: u16) -> bool {
        let host_clean = target_host
            .strip_prefix('[')
            .and_then(|x| x.strip_suffix(']'))
            .unwrap_or(target_host);
        match self {
            Self::Wildcard => true,
            Self::Hostname { host, port } => {
                if let Some(expected_port) = port
                    && *expected_port != target_port
                {
                    return false;
                }
                let lower = host_clean.to_ascii_lowercase();
                if lower == *host {
                    return true;
                }
                lower
                    .strip_suffix(host.as_str())
                    .is_some_and(|prefix| prefix.ends_with('.'))
            }
            Self::Ipv4Cidr {
                network,
                prefix_len,
                port,
            } => {
                if let Some(expected_port) = port
                    && *expected_port != target_port
                {
                    return false;
                }
                let Ok(IpAddr::V4(v4)) = host_clean.parse::<IpAddr>() else {
                    return false;
                };
                let mask = ipv4_mask(*prefix_len);
                (u32::from(v4) & mask) == *network
            }
            Self::Ipv6Cidr {
                network,
                prefix_len,
                port,
            } => {
                if let Some(expected_port) = port
                    && *expected_port != target_port
                {
                    return false;
                }
                let Ok(IpAddr::V6(v6)) = host_clean.parse::<IpAddr>() else {
                    return false;
                };
                let mask = ipv6_mask(*prefix_len);
                (u128::from(v6) & mask) == *network
            }
        }
    }
}

fn split_cidr_trailing_port(s: &str) -> (&str, Option<u16>) {
    let Some(slash_idx) = s.find('/') else {
        return (s, None);
    };
    let after_slash = &s[slash_idx + 1..];
    if let Some(colon_rel) = after_slash.rfind(':') {
        let port_str = &after_slash[colon_rel + 1..];
        if !port_str.is_empty()
            && port_str.bytes().all(|b| b.is_ascii_digit())
            && let Ok(port) = port_str.parse::<u16>()
        {
            return (&s[..slash_idx + 1 + colon_rel], Some(port));
        }
    }
    (s, None)
}

fn ipv4_mask(prefix_len: u8) -> u32 {
    if prefix_len == 0 {
        0
    } else {
        u32::MAX << (32 - prefix_len)
    }
}

fn ipv6_mask(prefix_len: u8) -> u128 {
    if prefix_len == 0 {
        0
    } else {
        u128::MAX << (128 - prefix_len)
    }
}

/// Proxy + noProxy selector matching `org.pkl.core.http.ProxySelector`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyConfig {
    pub address: Option<ProxyAddress>,
    pub no_proxy: Vec<NoProxyRule>,
}

impl ProxyConfig {
    pub fn new(address: Option<&str>, no_proxy: &[String]) -> Result<Self, String> {
        let address = address.map(ProxyAddress::parse).transpose()?;
        let mut rules = Vec::with_capacity(no_proxy.len());
        for raw in no_proxy {
            if let Some(rule) = NoProxyRule::parse(raw)? {
                rules.push(rule);
            }
        }
        Ok(Self {
            address,
            no_proxy: rules,
        })
    }

    pub fn select(&self, uri: &ParsedHttpUri) -> Option<&ProxyAddress> {
        let proxy = self.address.as_ref()?;
        let port = uri.effective_port();
        if self
            .no_proxy
            .iter()
            .any(|rule| rule.matches(&uri.host, port))
        {
            None
        } else {
            Some(proxy)
        }
    }
}

/// Ordered URL rewrite entry (sorted longest source prefix first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteRule {
    pub source_normalized: String,
    pub target_normalized: String,
}

/// Ordered custom HTTP header rule bound to a URL glob pattern.
#[derive(Clone, Debug)]
pub struct HeaderRule {
    pub matcher: HeaderGlobMatcher,
    pub headers: Vec<(String, String)>,
}

/// Full pure-core HTTP client configuration.
#[derive(Clone, Debug)]
pub struct HttpClientConfig {
    pub user_agent: String,
    pub max_redirects: usize,
    pub rewrites: Vec<RewriteRule>,
    pub headers: Vec<HeaderRule>,
    pub proxy: ProxyConfig,
    pub test_port: Option<u16>,
    pub connect_timeout_ms: u64,
    pub request_timeout_ms: u64,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            user_agent: DEFAULT_USER_AGENT.to_string(),
            max_redirects: DEFAULT_MAX_REDIRECTS,
            rewrites: Vec::new(),
            headers: Vec::new(),
            proxy: ProxyConfig::default(),
            test_port: None,
            connect_timeout_ms: 60_000,
            request_timeout_ms: 60_000,
        }
    }
}

impl HttpClientConfig {
    pub fn with_rewrites(
        mut self,
        rewrites: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, String> {
        let mut rules = Vec::new();
        for (src, dst) in rewrites {
            let source_normalized = validate_rewrite_rule(&src)?;
            let target_normalized = validate_rewrite_rule(&dst)?;
            rules.push(RewriteRule {
                source_normalized,
                target_normalized,
            });
        }
        // Longest source prefix wins (`RequestRewritingClient.java:102`).
        rules.sort_by_key(|a| std::cmp::Reverse(a.source_normalized.len()));
        self.rewrites = rules;
        Ok(self)
    }

    pub fn add_headers(
        &mut self,
        url_pattern: &str,
        headers: impl IntoIterator<Item = (String, String)>,
    ) -> Result<(), String> {
        let matcher = HeaderGlobMatcher::compile(url_pattern)?;
        let mut validated = Vec::new();
        for (name, value) in headers {
            validate_header_name(&name)?;
            validate_header_value(&value)?;
            validated.push((name, value));
        }
        if let Some(existing) = self
            .headers
            .iter_mut()
            .find(|rule| rule.matcher.pattern() == url_pattern)
        {
            existing.headers.extend(validated);
        } else {
            self.headers.push(HeaderRule {
                matcher,
                headers: validated,
            });
        }
        Ok(())
    }

    pub fn with_proxy(mut self, proxy: ProxyConfig) -> Self {
        self.proxy = proxy;
        self
    }

    pub fn with_test_port(mut self, test_port: Option<u16>) -> Self {
        self.test_port = test_port;
        self
    }

    /// Rewrites a URI once (no single-hop chaining), applying longest prefix + `--test-port`.
    /// Returns `(rewritten_uri, was_rewritten)`.
    pub fn rewrite_uri(&self, uri: &str) -> Result<(String, bool), String> {
        let parsed = ParsedHttpUri::parse(uri)?;
        let normalized = parsed.normalized_scheme_and_host();
        let mut out = uri.to_string();
        let mut rewritten = false;

        for rule in &self.rewrites {
            if let Some(suffix) = normalized.strip_prefix(&rule.source_normalized) {
                out = format!("{}{suffix}", rule.target_normalized);
                rewritten = true;
                break;
            }
        }

        if let Some(test_port) = self.test_port {
            let after = ParsedHttpUri::parse(&out)?;
            if after.port == Some(0) {
                out = after.with_port(test_port);
                rewritten = true;
            }
        }

        Ok((out, rewritten))
    }

    /// Computes ordered HTTP request headers for `target_uri` (per-hop, never leaking prior hops).
    /// Adds default `User-Agent` iff no matched rule sets `User-Agent` (case-insensitively).
    pub fn headers_for_uri(&self, target_uri: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut has_user_agent = false;
        for rule in &self.headers {
            if rule.matcher.matches(target_uri) {
                for (k, v) in &rule.headers {
                    if k.eq_ignore_ascii_case("user-agent") {
                        has_user_agent = true;
                    }
                    out.push((k.clone(), v.clone()));
                }
            }
        }
        if !has_user_agent {
            out.push(("User-Agent".to_string(), self.user_agent.clone()));
        }
        out
    }
}

/// Single-hop HTTP request handed to a platform `HttpTransport`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub uri: String,
    pub parsed: ParsedHttpUri,
    pub headers: Vec<(String, String)>,
    pub proxy: Option<ProxyAddress>,
    pub connect_timeout_ms: u64,
    pub request_timeout_ms: u64,
}

/// Single-hop HTTP response returned by a platform `HttpTransport`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn first_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn header_map_lowercase(&self) -> BTreeMap<String, Vec<String>> {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (k, v) in &self.headers {
            map.entry(k.to_ascii_lowercase())
                .or_default()
                .push(v.clone());
        }
        map
    }
}

/// Typed HTTP execution errors preserving Pkl 0.32.0 `HttpClientException` vs `IOException`
/// vs `SecurityManagerException` boundaries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpError {
    /// User-presentable `org.pkl.core.http.HttpClientException` (no `IOException:` prefix).
    HttpClientException(PklString),
    /// `org.pkl.core.SecurityManagerException` from `httpRequestChecker`.
    SecurityManagerException(PklString),
    /// `java.io.IOException` (rendered under `I/O error ...` + `IOException: <msg>`).
    Io(PklString),
    /// Client was already closed (`IllegalStateException` equivalent).
    Closed(PklString),
}

impl HttpError {
    fn io_failure(msg: PklString) -> IoFailure {
        if let Some(s) = msg.as_str() {
            if s.starts_with("Error during SSL handshake with host ") {
                return IoFailure::with_class("SSLHandshakeException", msg);
            }
            if s.starts_with("Error connecting to host ") {
                return IoFailure::with_class("ConnectException", msg);
            }
        }
        IoFailure::from(msg)
    }

    pub fn into_module_error(self) -> ModuleHostError {
        match self {
            Self::HttpClientException(msg) => {
                ModuleHostError::HttpClientException { message: Some(msg) }
            }
            Self::SecurityManagerException(msg) => {
                ModuleHostError::SecurityManagerException { message: Some(msg) }
            }
            Self::Io(msg) | Self::Closed(msg) => ModuleHostError::Io(Self::io_failure(msg)),
        }
    }

    pub fn into_read_error(self) -> ReadFailure {
        match self {
            Self::HttpClientException(msg) => {
                ReadFailure::HttpClientException { message: Some(msg) }
            }
            Self::SecurityManagerException(msg) => {
                ReadFailure::SecurityManagerException { message: Some(msg) }
            }
            Self::Io(msg) | Self::Closed(msg) => ReadFailure::Io(Self::io_failure(msg)),
        }
    }
}

/// Single-hop non-redirecting platform transport (`JdkHttpClient` equivalent).
pub trait HttpTransport: Send + Sync {
    /// Sends a single HTTP GET request WITHOUT following redirects.
    fn send_single_hop(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError>;

    /// Releases underlying transport resources (connections, sockets).
    fn close(&self) {}
}

pub fn is_redirect_status(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// Validates HTTP status code 200 (`HttpUtils.checkHasStatusCode200`).
pub fn check_status_200(status: u16, request_uri: &str) -> Result<(), HttpError> {
    if status == 200 {
        Ok(())
    } else {
        Err(HttpError::Io(
            format!("Received unexpected status code `{status}` when making request `GET {request_uri}`.")
                .into(),
        ))
    }
}

/// Wasm-pure `RequestRewritingClient` implementing exact Pkl 0.32.0 redirect, rewrite,
/// per-hop header isolation, per-hop security callback, downgrade block, and `close()`.
pub struct RewritingHttpClient<T: HttpTransport> {
    config: HttpClientConfig,
    transport: T,
    closed: AtomicBool,
}

impl<T: HttpTransport> RewritingHttpClient<T> {
    pub fn new(config: HttpClientConfig, transport: T) -> Self {
        Self {
            config,
            transport,
            closed: AtomicBool::new(false),
        }
    }

    pub fn config(&self) -> &HttpClientConfig {
        &self.config
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub fn close(&self) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.transport.close();
        }
    }

    /// Sends a GET request through rewrites + manual redirect loop + per-hop `checker`.
    pub fn get<F>(&self, original_uri: &str, checker: F) -> Result<HttpResponse, HttpError>
    where
        F: FnMut(&str) -> Result<(), PklString>,
    {
        self.get_with_uri(original_uri, checker)
            .map(|(resp, _)| resp)
    }

    /// Sends a GET request and returns `(response, final_uri)` where `final_uri` is the
    /// final rewritten/redirected request URI (`response.uri()` in `HttpUtils.java:60-61`).
    pub fn get_with_uri<F>(
        &self,
        original_uri: &str,
        mut checker: F,
    ) -> Result<(HttpResponse, String), HttpError>
    where
        F: FnMut(&str) -> Result<(), PklString>,
    {
        if self.is_closed() {
            return Err(HttpError::Closed(
                format!(
                    "Cannot send request GET {original_uri} because this client has already been closed."
                )
                .into(),
            ));
        }

        let (initial_rewritten, initial_was_rewritten) = self
            .config
            .rewrite_uri(original_uri)
            .map_err(|msg| HttpError::HttpClientException(msg.into()))?;

        let result = self.do_get(&initial_rewritten, &mut checker);
        match result {
            Ok(pair) => Ok(pair),
            Err(HttpError::Io(msg))
                if initial_was_rewritten && initial_rewritten != original_uri =>
            {
                Err(HttpError::Io(PklString::from(format!(
                    "{} (request was rewritten: {original_uri} -> {initial_rewritten})",
                    msg.to_java_utf8()
                ))))
            }
            Err(other) => Err(other),
        }
    }

    fn do_get<F>(
        &self,
        start_uri: &str,
        checker: &mut F,
    ) -> Result<(HttpResponse, String), HttpError>
    where
        F: FnMut(&str) -> Result<(), PklString>,
    {
        let mut redirect_count = 0usize;
        let mut current_uri = start_uri.to_string();

        loop {
            let parsed = ParsedHttpUri::parse(&current_uri)
                .map_err(|msg| HttpError::HttpClientException(msg.into()))?;

            // Per-hop SecurityManager check BEFORE sending (`RequestRewritingClient.java:116`).
            checker(&current_uri).map_err(HttpError::SecurityManagerException)?;

            let headers = self.config.headers_for_uri(&current_uri);
            let proxy = self.config.proxy.select(&parsed).cloned();
            let request = HttpRequest {
                uri: current_uri.clone(),
                parsed: parsed.clone(),
                headers,
                proxy,
                connect_timeout_ms: self.config.connect_timeout_ms,
                request_timeout_ms: self.config.request_timeout_ms,
            };

            let response = self.transport.send_single_hop(&request)?;
            if !is_redirect_status(response.status) {
                return Ok((response, current_uri));
            }

            if redirect_count >= self.config.max_redirects {
                return Err(HttpError::HttpClientException(
                    format!(
                        "Too many redirects, exceeded the redirect threshold ({}).",
                        self.config.max_redirects
                    )
                    .into(),
                ));
            }

            let Some(location) = response.first_header("Location") else {
                return Err(HttpError::HttpClientException(
                    format!(
                        "Cannot follow HTTP redirect because no 'Location' header was found.\n\nHTTP Request: `GET {current_uri}`"
                    )
                    .into(),
                ));
            };

            let redirect_uri = parsed.resolve_redirect(location).map_err(|_| {
                HttpError::HttpClientException(
                    format!(
                        "Cannot follow HTTP redirect because the response 'Location' header has a malformed URI.\n\nHTTP Request: `GET {current_uri}`\nLocation header: `{location}`"
                    )
                    .into(),
                )
            })?;

            let redirect_parsed = ParsedHttpUri::parse(&redirect_uri).map_err(|_| {
                HttpError::HttpClientException(
                    format!(
                        "Cannot follow HTTP redirect because the response 'Location' header has a malformed URI.\n\nHTTP Request: `GET {current_uri}`\nLocation header: `{location}`"
                    )
                    .into(),
                )
            })?;

            // Block https -> http downgrade BEFORE rewriting the redirect target (`RequestRewritingClient.java:137-141`).
            if parsed.scheme == "https" && redirect_parsed.scheme == "http" {
                return Err(HttpError::HttpClientException(
                    format!(
                        "Cannot follow redirect from 'https:' URL to 'http:' URL.\nHTTP Request: `GET {current_uri}`\nRedirected to: `{redirect_uri}`"
                    )
                    .into(),
                ));
            }

            let (rewritten_next, _) = self
                .config
                .rewrite_uri(&redirect_uri)
                .map_err(|msg| HttpError::HttpClientException(msg.into()))?;
            current_uri = rewritten_next;
            redirect_count += 1;
        }
    }

    /// Module fetch semantics (`ModuleKeys.Http.resolve`): ANY non-200 (including 404) is `IOException`.
    pub fn fetch_module<F>(
        &self,
        original_uri: &str,
        checker: F,
    ) -> Result<Vec<u8>, ModuleHostError>
    where
        F: FnMut(&str) -> Result<(), PklString>,
    {
        let (resp, final_uri) = self
            .get_with_uri(original_uri, checker)
            .map_err(HttpError::into_module_error)?;
        check_status_200(resp.status, &final_uri).map_err(HttpError::into_module_error)?;
        Ok(resp.body)
    }

    /// Resource fetch semantics (`ResourceReaders.UrlResource.read`): 404 -> `Ok(None)` (Missing),
    /// 200 -> `Ok(Some(bytes))`, other -> `IOException`.
    pub fn fetch_resource<F>(
        &self,
        original_uri: &str,
        checker: F,
    ) -> Result<Option<Vec<u8>>, ReadFailure>
    where
        F: FnMut(&str) -> Result<(), PklString>,
    {
        let (resp, final_uri) = self
            .get_with_uri(original_uri, checker)
            .map_err(HttpError::into_read_error)?;
        if resp.status == 404 {
            return Ok(None);
        }
        check_status_200(resp.status, &final_uri).map_err(HttpError::into_read_error)?;
        Ok(Some(resp.body))
    }
}

/// Lazy HTTP transport decorator matching `org.pkl.core.http.LazyHttpClient`:
/// defers initializing the underlying transport (e.g., loading CA cert files) until the first
/// request, caches initialization errors so initialization is attempted only once, and closes
/// cleanly if initialized (never holding the mutex across blocking network I/O).
pub struct LazyHttpTransport<T, F>
where
    T: HttpTransport,
    F: Fn() -> Result<T, HttpError> + Send + Sync,
{
    factory: F,
    state: Mutex<LazyState<T>>,
}

enum LazyState<T> {
    Uninitialized,
    Ready(Arc<T>),
    Failed(HttpError),
    Closed,
}

impl<T, F> LazyHttpTransport<T, F>
where
    T: HttpTransport,
    F: Fn() -> Result<T, HttpError> + Send + Sync,
{
    pub fn new(factory: F) -> Self {
        Self {
            factory,
            state: Mutex::new(LazyState::Uninitialized),
        }
    }

    pub fn is_initialized(&self) -> bool {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        matches!(*guard, LazyState::Ready(_))
    }
}

impl<T, F> HttpTransport for LazyHttpTransport<T, F>
where
    T: HttpTransport,
    F: Fn() -> Result<T, HttpError> + Send + Sync,
{
    fn send_single_hop(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        let client = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            match &*guard {
                LazyState::Uninitialized => match (self.factory)() {
                    Ok(client) => {
                        let arc = Arc::new(client);
                        *guard = LazyState::Ready(Arc::clone(&arc));
                        arc
                    }
                    Err(err) => {
                        *guard = LazyState::Failed(err.clone());
                        return Err(err);
                    }
                },
                LazyState::Ready(client) => Arc::clone(client),
                LazyState::Failed(err) => return Err(err.clone()),
                LazyState::Closed => {
                    return Err(HttpError::Closed(
                        format!(
                            "Cannot send request GET {} because this client has already been closed.",
                            request.uri
                        )
                        .into(),
                    ));
                }
            }
        };
        // Mutex guard is dropped before blocking network I/O so `close()` and concurrent
        // requests never deadlock or serialize behind the init lock.
        client.send_single_hop(request)
    }

    fn close(&self) {
        let client = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            match std::mem::replace(&mut *guard, LazyState::Closed) {
                LazyState::Ready(client) => Some(client),
                _ => None,
            }
        };
        if let Some(client) = client {
            client.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_proxy_rules_match_domains_ports_and_cidrs() {
        let cfg = ProxyConfig::new(
            Some("http://proxy.internal:8080"),
            &[
                ".example.com".into(),
                "api.test:8443".into(),
                "10.0.0.0/8".into(),
                "fe80::/10".into(),
                "[::1]:9000".into(),
            ],
        )
        .unwrap();

        let bypass_sub = ParsedHttpUri::parse("https://sub.example.com/a").unwrap();
        let bypass_root = ParsedHttpUri::parse("https://example.com/a").unwrap();
        let proxied_other = ParsedHttpUri::parse("https://notexample.com/a").unwrap();
        let bypass_port = ParsedHttpUri::parse("https://api.test:8443/a").unwrap();
        let proxied_port = ParsedHttpUri::parse("https://api.test:443/a").unwrap();
        let bypass_v4 = ParsedHttpUri::parse("http://10.42.1.2/x").unwrap();
        let proxied_v4 = ParsedHttpUri::parse("http://192.168.1.2/x").unwrap();
        let bypass_v6 = ParsedHttpUri::parse("http://[fe80::1]/x").unwrap();
        let bypass_v6_port = ParsedHttpUri::parse("http://[::1]:9000/x").unwrap();
        let proxied_v6_port = ParsedHttpUri::parse("http://[::1]:9001/x").unwrap();

        assert!(cfg.select(&bypass_sub).is_none());
        assert!(cfg.select(&bypass_root).is_none());
        assert!(cfg.select(&proxied_other).is_some());
        assert!(cfg.select(&bypass_port).is_none());
        assert!(cfg.select(&proxied_port).is_some());
        assert!(cfg.select(&bypass_v4).is_none());
        assert!(cfg.select(&proxied_v4).is_some());
        assert!(cfg.select(&bypass_v6).is_none());
        assert!(cfg.select(&bypass_v6_port).is_none());
        assert!(cfg.select(&proxied_v6_port).is_some());
    }

    #[test]
    fn header_validation_rejects_reserved_and_invalid_values() {
        assert!(validate_header_name("Host").is_err());
        assert!(validate_header_name("Connection").is_err());
        assert!(validate_header_name("Proxy-Authorization").is_err());
        assert!(validate_header_name("Sec-Fetch-Mode").is_err());
        assert!(validate_header_name("Bad Header").is_err());
        assert!(validate_header_name("Authorization").is_ok());
        assert!(validate_header_name("X-Custom-Token").is_ok());

        assert!(validate_header_value("Bearer abc123").is_ok());
        assert!(validate_header_value("bad\r\nheader").is_err());
        assert!(validate_header_value(&"a".repeat(4097)).is_err());
        assert!(validate_header_value(&"a".repeat(4096)).is_ok());
    }

    #[test]
    fn proxy_validation_enforces_http_host_port_only() {
        assert!(ProxyAddress::parse("http://proxy.local").unwrap().port == 80);
        assert!(ProxyAddress::parse("http://proxy.local:3128").unwrap().port == 3128);
        assert!(ProxyAddress::parse("https://proxy.local:3128").is_err());
        assert!(ProxyAddress::parse("http://user:pass@proxy.local:3128").is_err());
        assert!(ProxyAddress::parse("http://proxy.local:3128/path").is_err());
    }
}
