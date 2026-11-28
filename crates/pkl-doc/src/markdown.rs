//! Pure-Rust CommonMark 0.31.2 + GFM Tables AST parser and `org.commonmark` 0.29.0
//! HTML renderer with `MarkdownParserContext` / `MarkdownNodeRenderer` (`Markdown.kt`)
//! doc-link resolution semantics.

use crate::html::escape_html_into;
use crate::util::{kotlin_trim_indent, sanitize_href_url};

const MAX_MARKDOWN_DEPTH: usize = 128;

/// Resolution returned by `MarkdownParserContext.getLinkReferenceDefinition(label)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefLinkResolution {
    pub destination: String,
    pub label: String,
    /// `true` => command `'1'` (short ref renders as `<code>label</code>`),
    /// `false` => command `'2'` (short ref renders as `<a href="dest"><code>label</code></a>`).
    pub code_only_if_short: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableAlignment {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableCell {
    pub alignment: TableAlignment,
    pub inlines: Vec<Inline>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    CodeBlock {
        info: Option<String>,
        literal: String,
    },
    ThematicBreak,
    BlockQuote(Vec<Block>),
    List {
        ordered: bool,
        start: u32,
        tight: bool,
        items: Vec<Vec<Block>>,
    },
    Table {
        headers: Vec<TableCell>,
        rows: Vec<Vec<TableCell>>,
    },
    HtmlBlock(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Code(String),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Link {
        destination: String,
        title: Option<String>,
        pkldoc: Option<PkldocLinkMeta>,
        children: Vec<Inline>,
    },
    Image {
        destination: String,
        title: Option<String>,
        alt: Vec<Inline>,
    },
    Autolink {
        destination: String,
        text: String,
    },
    HardBreak,
    SoftBreak,
    HtmlInline(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkldocLinkMeta {
    pub label: String,
    pub code_only_if_short: bool,
}

/// Renders a Markdown block string using `org.commonmark` + `TablesExtension` + `MarkdownNodeRenderer`,
/// trimmed matching `PageGenerator.kt:706,717` (`markdownRenderer.render(markdownParser.parse(it)).trim()`).
pub fn render_markdown_trimmed<F>(input: &str, mut resolve_ref: F) -> Option<String>
where
    F: FnMut(&str) -> RefLinkResolution,
{
    let blocks = parse_blocks(input, &mut resolve_ref);
    let mut out = String::new();
    render_blocks(&blocks, false, &mut out);
    let trimmed = out.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Mirrors `PageGenerator.kt:824-833` `renderInlineMarkdownText`:
/// trims indent + trims, parses AST, unwraps single-child wrapper chain (`Document -> Paragraph -> single inline`),
/// and renders.
pub fn render_inline_markdown_text<F>(input: &str, mut resolve_ref: F) -> String
where
    F: FnMut(&str) -> RefLinkResolution,
{
    let cleaned = kotlin_trim_indent(input);
    let trimmed = cleaned.trim();
    let blocks = parse_blocks(trimmed, &mut resolve_ref);
    if blocks.len() == 1 {
        let mut out = String::new();
        match &blocks[0] {
            Block::Paragraph(inlines) if inlines.len() == 1 => {
                // Unwrap Document -> Paragraph -> single inline (and recursively if single child)
                render_unwrapped_inline(&inlines[0], &mut out);
                return out;
            }
            block => {
                render_block(block, false, &mut out);
                return out;
            }
        }
    }
    let mut out = String::new();
    render_blocks(&blocks, false, &mut out);
    out
}

fn render_unwrapped_inline(inline: &Inline, out: &mut String) {
    match inline {
        Inline::Emphasis(ch) if ch.len() == 1 => render_unwrapped_inline(&ch[0], out),
        Inline::Strong(ch) if ch.len() == 1 => render_unwrapped_inline(&ch[0], out),
        other => render_inline(other, out),
    }
}

// ---------------------------------------------------------------------------
// Block Parser (CommonMark + GFM Tables)
// ---------------------------------------------------------------------------

fn normalize_newlines(s: &str) -> String {
    if !s.contains('\r') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' {
            out.push('\n');
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i += 2;
            } else {
                i += 1;
            }
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn split_lines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = s.split('\n').collect();
    if s.ends_with('\n') && lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

fn parse_blocks<F>(input: &str, resolve_ref: &mut F) -> Vec<Block>
where
    F: FnMut(&str) -> RefLinkResolution,
{
    let norm = normalize_newlines(input);
    let lines = split_lines(&norm);
    parse_block_lines(&lines, resolve_ref, 0)
}

fn count_leading_spaces(line: &str) -> (usize, usize) {
    let mut cols = 0;
    let mut byte_idx = 0;
    for (i, ch) in line.char_indices() {
        match ch {
            ' ' => {
                cols += 1;
                byte_idx = i + 1;
            }
            '\t' => {
                let advance = 4 - (cols % 4);
                cols += advance;
                byte_idx = i + 1;
            }
            _ => break,
        }
    }
    (cols, byte_idx)
}

fn strip_columns(line: &str, max_cols: usize) -> String {
    let mut cols = 0;
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if cols >= max_cols {
            break;
        }
        match ch {
            ' ' => {
                cols += 1;
                chars.next();
            }
            '\t' => {
                let advance = 4 - (cols % 4);
                chars.next();
                if cols + advance <= max_cols {
                    cols += advance;
                } else {
                    let rem = (cols + advance) - max_cols;
                    for _ in 0..rem {
                        out.push(' ');
                    }
                    cols = max_cols;
                }
            }
            _ => break,
        }
    }
    for ch in chars {
        out.push(ch);
    }
    out
}

fn is_blank(line: &str) -> bool {
    line.bytes().all(|b| matches!(b, b' ' | b'\t'))
}

#[derive(Clone, Debug)]
struct FenceOpen {
    indent: usize,
    fence_char: u8,
    fence_len: usize,
    info: Option<String>,
}

fn parse_fence_open(line: &str) -> Option<FenceOpen> {
    let (indent, start) = count_leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[start..];
    let first = *rest.as_bytes().first()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let fence_len = rest.bytes().take_while(|&b| b == first).count();
    if fence_len < 3 {
        return None;
    }
    let after = &rest[fence_len..];
    if first == b'`' && after.contains('`') {
        return None;
    }
    let info_trimmed = after.trim();
    let info = if info_trimmed.is_empty() {
        None
    } else {
        Some(unescape_string(info_trimmed))
    };
    Some(FenceOpen {
        indent,
        fence_char: first,
        fence_len,
        info,
    })
}

fn is_fence_close(line: &str, open: &FenceOpen) -> bool {
    let (indent, start) = count_leading_spaces(line);
    if indent > 3 {
        return false;
    }
    let rest = &line[start..];
    let count = rest.bytes().take_while(|&b| b == open.fence_char).count();
    if count < open.fence_len {
        return false;
    }
    rest[count..].bytes().all(|b| matches!(b, b' ' | b'\t'))
}

fn parse_atx_heading(line: &str) -> Option<(u8, String)> {
    let (indent, start) = count_leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[start..];
    let hashes = rest.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let after = &rest[hashes..];
    if !after.is_empty() && !matches!(after.as_bytes()[0], b' ' | b'\t') {
        return None;
    }
    let mut content = after.trim();
    // Strip optional closing sequence of `#` preceded by space/tab (or whole string of `#`)
    let without_hashes = content.trim_end_matches('#');
    if without_hashes.is_empty() {
        content = "";
    } else if without_hashes.len() < content.len()
        && matches!(without_hashes.as_bytes().last(), Some(b' ' | b'\t'))
    {
        content = without_hashes.trim_end_matches([' ', '\t']);
    }
    Some((hashes as u8, content.to_string()))
}

fn is_thematic_break(line: &str) -> bool {
    let (indent, start) = count_leading_spaces(line);
    if indent > 3 {
        return false;
    }
    let rest = &line[start..];
    let mut marker: Option<u8> = None;
    let mut count = 0usize;
    for b in rest.bytes() {
        match b {
            b' ' | b'\t' => {}
            b'-' | b'_' | b'*' => {
                if let Some(m) = marker {
                    if m != b {
                        return false;
                    }
                } else {
                    marker = Some(b);
                }
                count += 1;
            }
            _ => return false,
        }
    }
    count >= 3
}

fn parse_setext_underline(line: &str) -> Option<u8> {
    let (indent, start) = count_leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = line[start..].trim_end_matches([' ', '\t']);
    if rest.is_empty() {
        return None;
    }
    let first = rest.as_bytes()[0];
    if first != b'=' && first != b'-' {
        return None;
    }
    if rest.bytes().all(|b| b == first) {
        Some(if first == b'=' { 1 } else { 2 })
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ListMarker {
    ordered: bool,
    delimiter: u8,
    start: u32,
    /// Column where list item content starts (indent + marker + spaces after marker).
    content_col: usize,
    /// Byte index right after the marker + required spaces (for first line).
    content_byte: usize,
    empty_first_line: bool,
}

fn parse_list_marker(line: &str, in_paragraph: bool) -> Option<ListMarker> {
    let (indent, start) = count_leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[start..];
    let bytes = rest.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let (ordered, delimiter, start_num, marker_len) = if matches!(bytes[0], b'-' | b'+' | b'*') {
        (false, bytes[0], 1u32, 1usize)
    } else if bytes[0].is_ascii_digit() {
        let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 || digits > 9 || digits >= bytes.len() {
            return None;
        }
        let delim = bytes[digits];
        if delim != b'.' && delim != b')' {
            return None;
        }
        let num: u32 = rest[..digits].parse().ok()?;
        if in_paragraph && num != 1 {
            return None;
        }
        (true, delim, num, digits + 1)
    } else {
        return None;
    };

    let after_marker = &rest[marker_len..];
    if !after_marker.is_empty() && !matches!(after_marker.as_bytes()[0], b' ' | b'\t') {
        return None;
    }

    let marker_end_col = indent + marker_len;
    if is_blank(after_marker) {
        if in_paragraph {
            return None;
        }
        return Some(ListMarker {
            ordered,
            delimiter,
            start: start_num,
            content_col: marker_end_col + 1,
            content_byte: line.len(),
            empty_first_line: true,
        });
    }

    // Measure spaces/tabs after marker; if >= 5 cols (indented code), content_col = marker_end_col + 1
    let mut sp_cols = 0usize;
    let mut sp_bytes = 0usize;
    let mut col = marker_end_col;
    for (i, ch) in after_marker.char_indices() {
        match ch {
            ' ' => {
                sp_cols += 1;
                col += 1;
                sp_bytes = i + 1;
            }
            '\t' => {
                let adv = 4 - (col % 4);
                sp_cols += adv;
                col += adv;
                sp_bytes = i + 1;
            }
            _ => break,
        }
    }
    let (content_col, content_byte) = if sp_cols >= 5 {
        // Only consume 1 space; remainder belongs to indented code block
        (marker_end_col + 1, start + marker_len + 1)
    } else {
        (marker_end_col + sp_cols, start + marker_len + sp_bytes)
    };

    Some(ListMarker {
        ordered,
        delimiter,
        start: start_num,
        content_col,
        content_byte,
        empty_first_line: false,
    })
}

fn is_blockquote_line(line: &str) -> Option<&str> {
    let (indent, start) = count_leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[start..];
    let after = rest.strip_prefix('>')?;
    if let Some(s) = after.strip_prefix(' ') {
        Some(s)
    } else {
        Some(after)
    }
}

fn parse_table_delimiter_row(line: &str) -> Option<Vec<TableAlignment>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.contains('-') {
        return None;
    }
    // Check only allowed chars: | - : space tab
    if !trimmed
        .bytes()
        .all(|b| matches!(b, b'|' | b'-' | b':' | b' ' | b'\t'))
    {
        return None;
    }
    let cells = split_table_cells(trimmed);
    if cells.is_empty() {
        return None;
    }
    let mut alignments = Vec::with_capacity(cells.len());
    for cell in cells {
        let c = cell.trim();
        if c.is_empty() {
            return None;
        }
        let left = c.starts_with(':');
        let right = c.ends_with(':');
        let inner = c.trim_start_matches(':').trim_end_matches(':');
        if inner.is_empty() || !inner.bytes().all(|b| b == b'-') {
            return None;
        }
        let align = match (left, right) {
            (true, true) => TableAlignment::Center,
            (true, false) => TableAlignment::Left,
            (false, true) => TableAlignment::Right,
            (false, false) => TableAlignment::None,
        };
        alignments.push(align);
    }
    Some(alignments)
}

fn split_table_cells(line: &str) -> Vec<String> {
    let mut s = line.trim();
    if let Some(rest) = s.strip_prefix('|') {
        s = rest;
    }
    // Strip unescaped trailing pipe
    if s.ends_with('|') {
        let backslashes = s[..s.len() - 1]
            .bytes()
            .rev()
            .take_while(|&b| b == b'\\')
            .count();
        if backslashes % 2 == 0 {
            s = &s[..s.len() - 1];
        }
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'|') {
            chars.next();
            cur.push('|');
        } else if ch == '|' {
            cells.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    cells.push(cur);
    cells
}

fn can_start_table(
    header_line: &str,
    delim_line: &str,
) -> Option<(Vec<String>, Vec<TableAlignment>)> {
    if !header_line.contains('|') {
        return None;
    }
    let alignments = parse_table_delimiter_row(delim_line)?;
    let headers = split_table_cells(header_line);
    if headers.len() != alignments.len() {
        return None;
    }
    Some((headers, alignments))
}

fn starts_html_block(line: &str) -> Option<u8> {
    let (indent, start) = count_leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[start..];
    if !rest.starts_with('<') {
        return None;
    }
    if rest.starts_with("<!--") {
        return Some(2);
    }
    if rest.starts_with("<?") {
        return Some(3);
    }
    if rest.len() >= 3 && rest.starts_with("<!") && rest.as_bytes()[2].is_ascii_uppercase() {
        return Some(4);
    }
    if rest.starts_with("<![CDATA[") {
        return Some(5);
    }
    // Check block-level tag names (CommonMark HTML block type 6)
    let after = rest.strip_prefix("</").unwrap_or(&rest[1..]);
    let tag_len = after
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric())
        .count();
    if tag_len == 0 {
        return None;
    }
    let tag = after[..tag_len].to_ascii_lowercase();
    let next_ch = after.as_bytes().get(tag_len).copied();
    let valid_term = next_ch.is_none()
        || matches!(next_ch, Some(b' ' | b'\t' | b'>'))
        || after[tag_len..].starts_with("/>");
    if !valid_term {
        return None;
    }
    if matches!(tag.as_str(), "pre" | "script" | "style" | "textarea") && !rest.starts_with("</") {
        return Some(1);
    }
    if matches!(
        tag.as_str(),
        "address"
            | "article"
            | "aside"
            | "base"
            | "basefont"
            | "blockquote"
            | "body"
            | "caption"
            | "center"
            | "col"
            | "colgroup"
            | "dd"
            | "details"
            | "dialog"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hr"
            | "html"
            | "iframe"
            | "legend"
            | "li"
            | "link"
            | "main"
            | "menu"
            | "menuitem"
            | "nav"
            | "noframes"
            | "ol"
            | "optgroup"
            | "option"
            | "p"
            | "param"
            | "search"
            | "section"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "title"
            | "tr"
            | "track"
            | "ul"
    ) {
        return Some(6);
    }
    None
}

fn ends_html_block(line: &str, kind: u8) -> bool {
    match kind {
        1 => {
            let lower = line.to_ascii_lowercase();
            lower.contains("</pre>")
                || lower.contains("</script>")
                || lower.contains("</style>")
                || lower.contains("</textarea>")
        }
        2 => line.contains("-->"),
        3 => line.contains("?>"),
        4 => line.contains('>'),
        5 => line.contains("]]>"),
        _ => false,
    }
}

fn parse_block_lines<F>(lines: &[&str], resolve_ref: &mut F, depth: usize) -> Vec<Block>
where
    F: FnMut(&str) -> RefLinkResolution,
{
    if depth >= MAX_MARKDOWN_DEPTH {
        let text = lines.join("\n");
        let trimmed = text.trim();
        return if trimmed.is_empty() {
            Vec::new()
        } else {
            vec![Block::Paragraph(vec![Inline::Text(trimmed.to_string())])]
        };
    }
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if is_blank(line) {
            i += 1;
            continue;
        }

        // 1. Fenced code block
        if let Some(open) = parse_fence_open(line) {
            i += 1;
            let mut body = String::new();
            while i < lines.len() {
                if is_fence_close(lines[i], &open) {
                    i += 1;
                    break;
                }
                let stripped = strip_columns(lines[i], open.indent);
                body.push_str(&stripped);
                body.push('\n');
                i += 1;
            }
            blocks.push(Block::CodeBlock {
                info: open.info,
                literal: body,
            });
            continue;
        }

        // 2. Indented code block (>= 4 columns)
        let (indent, _) = count_leading_spaces(line);
        if indent >= 4 {
            let mut code_lines: Vec<String> = Vec::new();
            while i < lines.len() {
                if is_blank(lines[i]) || count_leading_spaces(lines[i]).0 >= 4 {
                    code_lines.push(strip_columns(lines[i], 4));
                    i += 1;
                } else {
                    break;
                }
            }
            while code_lines.last().is_some_and(|l| is_blank(l)) {
                code_lines.pop();
            }
            let mut literal = code_lines.join("\n");
            literal.push('\n');
            blocks.push(Block::CodeBlock {
                info: None,
                literal,
            });
            continue;
        }

        // 3. ATX Heading
        if let Some((level, content)) = parse_atx_heading(line) {
            let inlines = parse_inlines(&content, resolve_ref);
            blocks.push(Block::Heading { level, inlines });
            i += 1;
            continue;
        }

        // 4. Block quote
        if is_blockquote_line(line).is_some() {
            let mut q_lines: Vec<&str> = Vec::new();
            while i < lines.len() {
                if let Some(inner) = is_blockquote_line(lines[i]) {
                    q_lines.push(inner);
                    i += 1;
                } else {
                    break;
                }
            }
            let inner_blocks = parse_block_lines(&q_lines, resolve_ref, depth + 1);
            blocks.push(Block::BlockQuote(inner_blocks));
            continue;
        }

        // 5. Thematic break (must precede list marker since `- - -` / `* * *` match both)
        if is_thematic_break(line) {
            blocks.push(Block::ThematicBreak);
            i += 1;
            continue;
        }

        // 6. List (ordered or unordered)
        if let Some(first_marker) = parse_list_marker(line, false) {
            let (list_block, next_i) = parse_list(lines, i, first_marker, resolve_ref, depth + 1);
            blocks.push(list_block);
            i = next_i;
            continue;
        }

        // 7. HTML block
        if let Some(kind) = starts_html_block(line) {
            let mut html_lines = Vec::new();
            while i < lines.len() {
                if kind == 6 && is_blank(lines[i]) {
                    break;
                }
                html_lines.push(lines[i]);
                let done = ends_html_block(lines[i], kind);
                i += 1;
                if done {
                    break;
                }
            }
            blocks.push(Block::HtmlBlock(html_lines.join("\n")));
            continue;
        }

        // 8. GFM Table
        if i + 1 < lines.len()
            && let Some((header_cells, alignments)) = can_start_table(line, lines[i + 1])
        {
            i += 2;
            let headers = header_cells
                .into_iter()
                .zip(alignments.iter().copied())
                .map(|(text, alignment)| TableCell {
                    alignment,
                    inlines: parse_inlines(text.trim(), resolve_ref),
                })
                .collect();
            let mut rows = Vec::new();
            while i < lines.len() {
                let row_line = lines[i];
                if is_blank(row_line)
                    || parse_fence_open(row_line).is_some()
                    || parse_atx_heading(row_line).is_some()
                    || is_blockquote_line(row_line).is_some()
                    || is_thematic_break(row_line)
                    || parse_list_marker(row_line, true).is_some()
                {
                    break;
                }
                let raw_cells = split_table_cells(row_line);
                let mut row = Vec::with_capacity(alignments.len());
                for (col_idx, &alignment) in alignments.iter().enumerate() {
                    let cell_text = raw_cells.get(col_idx).map(|s| s.trim()).unwrap_or("");
                    row.push(TableCell {
                        alignment,
                        inlines: parse_inlines(cell_text, resolve_ref),
                    });
                }
                rows.push(row);
                i += 1;
            }
            blocks.push(Block::Table { headers, rows });
            continue;
        }

        // 9. Paragraph (or Setext heading)
        let mut para_lines: Vec<&str> = vec![line.trim_start_matches([' ', '\t'])];
        i += 1;
        let mut setext_level: Option<u8> = None;
        while i < lines.len() {
            let next = lines[i];
            if is_blank(next) {
                break;
            }
            if let Some(level) = parse_setext_underline(next) {
                setext_level = Some(level);
                i += 1;
                break;
            }
            if parse_fence_open(next).is_some()
                || parse_atx_heading(next).is_some()
                || is_blockquote_line(next).is_some()
                || is_thematic_break(next)
                || parse_list_marker(next, true).is_some()
                || starts_html_block(next).is_some()
            {
                break;
            }
            // Check if next line starts a table with the following line
            if i + 1 < lines.len() && can_start_table(next, lines[i + 1]).is_some() {
                break;
            }
            para_lines.push(next.trim_start_matches([' ', '\t']));
            i += 1;
        }
        let text = para_lines
            .join("\n")
            .trim_end_matches([' ', '\t'])
            .to_string();
        let inlines = parse_inlines(&text, resolve_ref);
        if let Some(level) = setext_level {
            blocks.push(Block::Heading { level, inlines });
        } else {
            blocks.push(Block::Paragraph(inlines));
        }
    }

    blocks
}

fn parse_list<F>(
    lines: &[&str],
    start_idx: usize,
    first_marker: ListMarker,
    resolve_ref: &mut F,
    depth: usize,
) -> (Block, usize)
where
    F: FnMut(&str) -> RefLinkResolution,
{
    let ordered = first_marker.ordered;
    let delimiter = first_marker.delimiter;
    let start_num = first_marker.start;

    let mut raw_items: Vec<(Vec<String>, bool)> = Vec::new(); // (lines, had_internal_blank_between_blocks)
    let mut blank_between_items = false;
    let mut i = start_idx;

    while i < lines.len() {
        // Consume blank lines between items (only if followed by matching list marker)
        let mut saw_blank = false;
        let mut peek = i;
        while peek < lines.len() && is_blank(lines[peek]) {
            saw_blank = true;
            peek += 1;
        }
        if peek >= lines.len() {
            break;
        }
        let line = lines[peek];
        if is_thematic_break(line) {
            break;
        }
        let Some(marker) = parse_list_marker(line, false) else {
            break;
        };
        if marker.ordered != ordered || marker.delimiter != delimiter {
            break;
        }
        if saw_blank && !raw_items.is_empty() {
            blank_between_items = true;
        }
        i = peek;

        let mut item_lines: Vec<String> = Vec::new();
        if marker.empty_first_line {
            item_lines.push(String::new());
        } else {
            item_lines.push(line[marker.content_byte..].to_string());
        }
        i += 1;

        let mut in_fence: Option<FenceOpen> = if !marker.empty_first_line {
            parse_fence_open(&item_lines[0])
        } else {
            None
        };

        while i < lines.len() {
            let cur = lines[i];
            if let Some(ref f) = in_fence {
                let stripped = strip_columns(cur, marker.content_col);
                if is_fence_close(&stripped, f) {
                    in_fence = None;
                }
                item_lines.push(stripped);
                i += 1;
                continue;
            }
            if is_blank(cur) {
                // Look ahead to see if continuation of this item follows
                let mut la = i + 1;
                while la < lines.len() && is_blank(lines[la]) {
                    la += 1;
                }
                if la >= lines.len() {
                    break;
                }
                let next_indent = count_leading_spaces(lines[la]).0;
                if next_indent >= marker.content_col {
                    while i < la {
                        item_lines.push(String::new());
                        i += 1;
                    }
                    continue;
                }
                // Blank line followed by next list item or outside list
                break;
            }
            let cur_indent = count_leading_spaces(cur).0;
            if cur_indent >= marker.content_col {
                let stripped = strip_columns(cur, marker.content_col);
                if in_fence.is_none() {
                    in_fence = parse_fence_open(&stripped);
                }
                item_lines.push(stripped);
                i += 1;
                continue;
            }
            // Lazy continuation line: only if not starting a new block/list item
            if is_thematic_break(cur)
                || parse_list_marker(cur, false).is_some()
                || parse_atx_heading(cur).is_some()
                || parse_fence_open(cur).is_some()
                || is_blockquote_line(cur).is_some()
            {
                break;
            }
            // Lazy continuation of a paragraph inside this item
            if item_lines.last().is_some_and(|l| !is_blank(l)) {
                item_lines.push(cur.trim_start_matches([' ', '\t']).to_string());
                i += 1;
            } else {
                break;
            }
        }

        // Check whether this item has a blank line directly between two blocks
        let trimmed_end_blanks = item_lines
            .iter()
            .rposition(|l| !is_blank(l))
            .map_or(0, |p| p + 1);
        let item_slice = &item_lines[..trimmed_end_blanks];
        let had_internal_blank = has_blank_outside_fences(item_slice);
        raw_items.push((item_slice.to_vec(), had_internal_blank));
    }

    let mut loose = blank_between_items;
    let mut items = Vec::with_capacity(raw_items.len());
    for (item_lines, had_internal_blank) in raw_items {
        let refs: Vec<&str> = item_lines.iter().map(String::as_str).collect();
        let parsed = parse_block_lines(&refs, resolve_ref, depth);
        if had_internal_blank && parsed.len() >= 2 {
            loose = true;
        }
        items.push(parsed);
    }

    (
        Block::List {
            ordered,
            start: start_num,
            tight: !loose,
            items,
        },
        i,
    )
}

fn has_blank_outside_fences(lines: &[String]) -> bool {
    let mut fence: Option<FenceOpen> = None;
    let mut seen_non_blank = false;
    for l in lines {
        if let Some(ref f) = fence {
            if is_fence_close(l, f) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = parse_fence_open(l) {
            seen_non_blank = true;
            fence = Some(open);
            continue;
        }
        if is_blank(l) {
            if seen_non_blank {
                return true;
            }
        } else {
            seen_non_blank = true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Inline Parser (CommonMark + MarkdownParserContext link refs)
// ---------------------------------------------------------------------------

fn parse_inlines<F>(input: &str, resolve_ref: &mut F) -> Vec<Inline>
where
    F: FnMut(&str) -> RefLinkResolution,
{
    parse_inlines_depth(input, resolve_ref, 0)
}

fn parse_inlines_depth<F>(input: &str, resolve_ref: &mut F, depth: usize) -> Vec<Inline>
where
    F: FnMut(&str) -> RefLinkResolution,
{
    if depth >= MAX_MARKDOWN_DEPTH {
        return if input.is_empty() {
            Vec::new()
        } else {
            vec![Inline::Text(input.to_string())]
        };
    }
    let mut parser = InlineParser {
        src: input,
        bytes: input.as_bytes(),
        pos: 0,
        depth,
        resolve_ref,
    };
    parser.parse_until_eof()
}

struct InlineParser<'a, F> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
    resolve_ref: &'a mut F,
}

impl<'a, F> InlineParser<'a, F>
where
    F: FnMut(&str) -> RefLinkResolution,
{
    fn parse_until_eof(&mut self) -> Vec<Inline> {
        let mut tokens: Vec<InlineToken> = Vec::new();
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            match b {
                b'\\' => self.parse_backslash(&mut tokens),
                b'`' => self.parse_code_span(&mut tokens),
                b'<' => self.parse_angle(&mut tokens),
                b'\n' => self.parse_newline(&mut tokens),
                b'*' | b'_' => self.parse_delim_run(b, &mut tokens),
                b'!' if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'[' => {
                    self.parse_link_or_image(true, &mut tokens);
                }
                b'[' => {
                    self.parse_link_or_image(false, &mut tokens);
                }
                b'&' => self.parse_entity(&mut tokens),
                _ => {
                    let start = self.pos;
                    let ch = self.src[self.pos..].chars().next().unwrap();
                    self.pos += ch.len_utf8();
                    while self.pos < self.bytes.len() {
                        let nb = self.bytes[self.pos];
                        if matches!(
                            nb,
                            b'\\' | b'`' | b'<' | b'\n' | b'*' | b'_' | b'[' | b'!' | b'&'
                        ) {
                            break;
                        }
                        let c = self.src[self.pos..].chars().next().unwrap();
                        self.pos += c.len_utf8();
                    }
                    push_text_token(&mut tokens, &self.src[start..self.pos]);
                }
            }
        }
        resolve_emphasis_tokens(tokens)
    }

    fn parse_backslash(&mut self, tokens: &mut Vec<InlineToken>) {
        if self.pos + 1 >= self.bytes.len() {
            self.pos += 1;
            push_text_token(tokens, "\\");
            return;
        }
        let next = self.bytes[self.pos + 1];
        if next == b'\n' {
            self.pos += 2;
            // Skip leading spaces on next line
            while self.pos < self.bytes.len() && self.bytes[self.pos] == b' ' {
                self.pos += 1;
            }
            tokens.push(InlineToken::Node(Inline::HardBreak));
        } else if is_ascii_punctuation(next) {
            self.pos += 2;
            push_text_token(tokens, std::str::from_utf8(&[next]).unwrap());
        } else {
            self.pos += 1;
            push_text_token(tokens, "\\");
        }
    }

    fn parse_newline(&mut self, tokens: &mut Vec<InlineToken>) {
        self.pos += 1;
        // Skip leading spaces on next line
        while self.pos < self.bytes.len() && self.bytes[self.pos] == b' ' {
            self.pos += 1;
        }
        // Check trailing spaces in previous text token for hard break (>= 2 spaces)
        if let Some(InlineToken::Node(Inline::Text(prev))) = tokens.last_mut() {
            let trailing_spaces = prev.bytes().rev().take_while(|&b| b == b' ').count();
            if trailing_spaces >= 2 {
                prev.truncate(prev.len() - trailing_spaces);
                if prev.is_empty() {
                    tokens.pop();
                }
                tokens.push(InlineToken::Node(Inline::HardBreak));
                return;
            } else if trailing_spaces == 1 {
                prev.pop();
                if prev.is_empty() {
                    tokens.pop();
                }
            }
        }
        tokens.push(InlineToken::Node(Inline::SoftBreak));
    }

    fn parse_code_span(&mut self, tokens: &mut Vec<InlineToken>) {
        let start = self.pos;
        let tick_len = self.bytes[start..]
            .iter()
            .take_while(|&&b| b == b'`')
            .count();
        let content_start = start + tick_len;
        let mut scan = content_start;
        while scan < self.bytes.len() {
            if self.bytes[scan] == b'`' {
                let run = self.bytes[scan..]
                    .iter()
                    .take_while(|&&b| b == b'`')
                    .count();
                if run == tick_len {
                    let raw = &self.src[content_start..scan];
                    let mut normalized = raw.replace('\n', " ");
                    if normalized.len() >= 2
                        && normalized.starts_with(' ')
                        && normalized.ends_with(' ')
                        && !normalized.bytes().all(|b| b == b' ')
                    {
                        normalized = normalized[1..normalized.len() - 1].to_string();
                    }
                    self.pos = scan + tick_len;
                    tokens.push(InlineToken::Node(Inline::Code(normalized)));
                    return;
                }
                scan += run;
            } else {
                scan += 1;
            }
        }
        // Unmatched backticks are literal text
        self.pos = content_start;
        push_text_token(tokens, &self.src[start..content_start]);
    }

    fn parse_angle(&mut self, tokens: &mut Vec<InlineToken>) {
        let rest = &self.src[self.pos..];
        // 1. URI autolink: <scheme:...>
        if let Some(close) = rest[1..].find('>') {
            let inner = &rest[1..1 + close];
            if is_uri_autolink(inner) {
                self.pos += close + 2;
                tokens.push(InlineToken::Node(Inline::Autolink {
                    destination: inner.to_string(),
                    text: inner.to_string(),
                }));
                return;
            }
            if is_email_autolink(inner) {
                self.pos += close + 2;
                tokens.push(InlineToken::Node(Inline::Autolink {
                    destination: format!("mailto:{inner}"),
                    text: inner.to_string(),
                }));
                return;
            }
        }
        // 2. Raw HTML inline tag / comment
        if let Some(len) = match_raw_html_inline(rest) {
            self.pos += len;
            tokens.push(InlineToken::Node(Inline::HtmlInline(
                rest[..len].to_string(),
            )));
            return;
        }
        self.pos += 1;
        push_text_token(tokens, "<");
    }

    fn parse_entity(&mut self, tokens: &mut Vec<InlineToken>) {
        if let Some((decoded, consumed)) = decode_html_entity(&self.src[self.pos..]) {
            self.pos += consumed;
            push_text_token(tokens, &decoded);
        } else {
            self.pos += 1;
            push_text_token(tokens, "&");
        }
    }

    fn parse_delim_run(&mut self, delim: u8, tokens: &mut Vec<InlineToken>) {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos] == delim {
            self.pos += 1;
        }
        let count = self.pos - start;

        let before = self.src[..start].chars().next_back().unwrap_or(' ');
        let after = self.src[self.pos..].chars().next().unwrap_or(' ');

        let before_ws = is_cm_whitespace(before);
        let after_ws = is_cm_whitespace(after);
        let before_punct = is_cm_punctuation(before);
        let after_punct = is_cm_punctuation(after);

        let left_flanking = !after_ws && (!after_punct || before_ws || before_punct);
        let right_flanking = !before_ws && (!before_punct || after_ws || after_punct);

        let (can_open, can_close) = if delim == b'_' {
            (
                left_flanking && (!right_flanking || before_punct),
                right_flanking && (!left_flanking || after_punct),
            )
        } else {
            (left_flanking, right_flanking)
        };

        tokens.push(InlineToken::Delim {
            ch: delim,
            count,
            orig_count: count,
            can_open,
            can_close,
        });
    }

    fn parse_link_or_image(&mut self, is_image: bool, tokens: &mut Vec<InlineToken>) {
        let start = self.pos;
        let open_bracket = if is_image { start + 1 } else { start };
        let Some(close_bracket) = find_matching_bracket(self.src, open_bracket) else {
            self.pos = open_bracket + 1;
            push_text_token(tokens, &self.src[start..self.pos]);
            return;
        };
        let label_raw = &self.src[open_bracket + 1..close_bracket];
        let after_close = close_bracket + 1;

        // 1. Inline link/image: [text](dest "title")
        if after_close < self.bytes.len()
            && self.bytes[after_close] == b'('
            && let Some((dest, title, end_pos)) = parse_inline_link_dest(self.src, after_close)
        {
            let children = parse_inlines_depth(label_raw, self.resolve_ref, self.depth + 1);
            self.pos = end_pos;
            if is_image {
                tokens.push(InlineToken::Node(Inline::Image {
                    destination: dest,
                    title,
                    alt: children,
                }));
            } else {
                tokens.push(InlineToken::Node(Inline::Link {
                    destination: dest,
                    title,
                    pkldoc: None,
                    children,
                }));
            }
            return;
        }

        // 2. Full / collapsed reference link: [text][ref] or [ref][]
        if after_close < self.bytes.len()
            && self.bytes[after_close] == b'['
            && let Some(ref_close) = find_simple_ref_label_end(self.src, after_close)
        {
            let ref_label_raw = &self.src[after_close + 1..ref_close];
            let effective_label = if ref_label_raw.is_empty() {
                label_raw
            } else {
                ref_label_raw
            };
            if is_valid_ref_label(effective_label) {
                // Parse children first (matches commonmark-java link child processing),
                // then resolve the reference label.
                let children = parse_inlines_depth(label_raw, self.resolve_ref, self.depth + 1);
                let res = (self.resolve_ref)(effective_label);
                self.pos = ref_close + 1;
                if is_image {
                    tokens.push(InlineToken::Node(Inline::Image {
                        destination: res.destination,
                        title: None,
                        alt: children,
                    }));
                } else {
                    tokens.push(InlineToken::Node(Inline::Link {
                        destination: res.destination,
                        title: None,
                        pkldoc: Some(PkldocLinkMeta {
                            label: res.label,
                            code_only_if_short: res.code_only_if_short,
                        }),
                        children,
                    }));
                }
                return;
            }
        }

        // 3. Shortcut reference link: [label] (only if NOT followed by `[` and valid ref label)
        if !is_image
            && is_valid_ref_label(label_raw)
            && !(after_close < self.bytes.len() && self.bytes[after_close] == b'[')
        {
            let children = parse_inlines_depth(label_raw, self.resolve_ref, self.depth + 1);
            let res = (self.resolve_ref)(label_raw);
            self.pos = after_close;
            tokens.push(InlineToken::Node(Inline::Link {
                destination: res.destination,
                title: None,
                pkldoc: Some(PkldocLinkMeta {
                    label: res.label,
                    code_only_if_short: res.code_only_if_short,
                }),
                children,
            }));
            return;
        }

        // Fallback: emit literal `[` (or `![`) and continue
        self.pos = open_bracket + 1;
        push_text_token(tokens, &self.src[start..self.pos]);
    }
}

fn find_matching_bracket(src: &str, open_pos: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut i = open_pos + 1;
    let mut depth = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => {
                i += 2;
            }
            b'`' => {
                let tick_len = bytes[i..].iter().take_while(|&&b| b == b'`').count();
                let mut scan = i + tick_len;
                let mut found = false;
                while scan < bytes.len() {
                    if bytes[scan] == b'`' {
                        let run = bytes[scan..].iter().take_while(|&&b| b == b'`').count();
                        if run == tick_len {
                            i = scan + run;
                            found = true;
                            break;
                        }
                        scan += run;
                    } else {
                        scan += 1;
                    }
                }
                if !found {
                    i += tick_len;
                }
            }
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

fn find_simple_ref_label_end(src: &str, open_pos: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut i = open_pos + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => i += 2,
            b'[' => return None,
            b']' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn is_valid_ref_label(label: &str) -> bool {
    !label.trim().is_empty() && label.len() <= 999
}

fn parse_inline_link_dest(src: &str, open_paren: usize) -> Option<(String, Option<String>, usize)> {
    let bytes = src.as_bytes();
    let mut i = open_paren + 1;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n') {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    if bytes[i] == b')' {
        return Some((String::new(), None, i + 1));
    }

    let dest = if bytes[i] == b'<' {
        i += 1;
        let start = i;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' if i + 1 < bytes.len() => i += 2,
                b'>' => break,
                b'\n' | b'<' => return None,
                _ => i += 1,
            }
        }
        if i >= bytes.len() || bytes[i] != b'>' {
            return None;
        }
        let d = unescape_string(&src[start..i]);
        i += 1;
        d
    } else {
        let start = i;
        let mut paren_depth = 0usize;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' if i + 1 < bytes.len() => i += 2,
                b'(' => {
                    paren_depth += 1;
                    i += 1;
                }
                b')' => {
                    if paren_depth == 0 {
                        break;
                    }
                    paren_depth -= 1;
                    i += 1;
                }
                b' ' | b'\t' | b'\n' => break,
                b if b < 0x20 || b == 0x7f => return None,
                _ => i += 1,
            }
        }
        if paren_depth != 0 {
            return None;
        }
        unescape_string(&src[start..i])
    };

    let ws_start = i;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n') {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    if bytes[i] == b')' {
        return Some((dest, None, i + 1));
    }

    // Optional link title (must be preceded by whitespace)
    if i == ws_start {
        return None;
    }
    let close_quote = match bytes[i] {
        b'"' => b'"',
        b'\'' => b'\'',
        b'(' => b')',
        _ => return None,
    };
    i += 1;
    let title_start = i;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
        } else if bytes[i] == close_quote {
            break;
        } else {
            i += 1;
        }
    }
    if i >= bytes.len() || bytes[i] != close_quote {
        return None;
    }
    let title = unescape_string(&src[title_start..i]);
    i += 1;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n') {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b')' {
        return None;
    }
    Some((dest, Some(title), i + 1))
}

#[derive(Clone, Debug)]
enum InlineToken {
    Node(Inline),
    Delim {
        ch: u8,
        count: usize,
        orig_count: usize,
        can_open: bool,
        can_close: bool,
    },
}

fn push_text_token(tokens: &mut Vec<InlineToken>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(InlineToken::Node(Inline::Text(prev))) = tokens.last_mut() {
        prev.push_str(text);
    } else {
        tokens.push(InlineToken::Node(Inline::Text(text.to_string())));
    }
}

/// Resolves `*` and `_` delimiter runs using the CommonMark emphasis algorithm.
fn resolve_emphasis_tokens(mut tokens: Vec<InlineToken>) -> Vec<Inline> {
    let mut closer_idx = 0usize;
    while closer_idx < tokens.len() {
        let (close_ch, close_count, close_orig, close_can_open, close_can_close) =
            match tokens[closer_idx] {
                InlineToken::Delim {
                    ch,
                    count,
                    orig_count,
                    can_open,
                    can_close,
                } if count > 0 && can_close => (ch, count, orig_count, can_open, can_close),
                _ => {
                    closer_idx += 1;
                    continue;
                }
            };
        let _ = close_can_close;

        // Search backwards for matching opener
        let mut opener_opt = None;
        for oi in (0..closer_idx).rev() {
            if let InlineToken::Delim {
                ch,
                count,
                orig_count,
                can_open,
                can_close,
            } = tokens[oi]
                && ch == close_ch
                && count > 0
                && can_open
            {
                // CommonMark rule 9/10: if one delimiter can both open and close,
                // sum of original lengths cannot be multiple of 3 unless both are multiples of 3.
                if (can_close || close_can_open)
                    && (orig_count + close_orig) % 3 == 0
                    && !(orig_count % 3 == 0 && close_orig % 3 == 0)
                {
                    continue;
                }
                opener_opt = Some((oi, count));
                break;
            }
        }

        let Some((opener_idx, open_count)) = opener_opt else {
            closer_idx += 1;
            continue;
        };

        let use_delims = if open_count >= 2 && close_count >= 2 {
            2
        } else {
            1
        };

        if let InlineToken::Delim { count, .. } = &mut tokens[opener_idx] {
            *count -= use_delims;
        }
        if let InlineToken::Delim { count, .. } = &mut tokens[closer_idx] {
            *count -= use_delims;
        }

        // Convert inner tokens (between opener_idx and closer_idx) into Inline nodes
        let inner_raw: Vec<InlineToken> = tokens.drain(opener_idx + 1..closer_idx).collect();
        let inner_inlines = finalize_tokens(inner_raw);
        let wrapped = if use_delims == 2 {
            Inline::Strong(inner_inlines)
        } else {
            Inline::Emphasis(inner_inlines)
        };
        tokens.insert(opener_idx + 1, InlineToken::Node(wrapped));
        closer_idx = opener_idx + 2;
        // Stay on closer_idx if it still has remaining delimiters
        if let InlineToken::Delim { count, .. } = tokens[closer_idx]
            && count == 0
        {
            closer_idx += 1;
        }
    }

    finalize_tokens(tokens)
}

fn finalize_tokens(tokens: Vec<InlineToken>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::with_capacity(tokens.len());
    for tok in tokens {
        match tok {
            InlineToken::Node(Inline::Text(t)) => {
                if t.is_empty() {
                    continue;
                }
                if let Some(Inline::Text(prev)) = out.last_mut() {
                    prev.push_str(&t);
                } else {
                    out.push(Inline::Text(t));
                }
            }
            InlineToken::Node(node) => out.push(node),
            InlineToken::Delim { ch, count, .. } if count > 0 => {
                let s: String = std::iter::repeat_n(ch as char, count).collect();
                if let Some(Inline::Text(prev)) = out.last_mut() {
                    prev.push_str(&s);
                } else {
                    out.push(Inline::Text(s));
                }
            }
            InlineToken::Delim { .. } => {}
        }
    }
    out
}

fn is_ascii_punctuation(b: u8) -> bool {
    matches!(
        b,
        b'!'..=b'/' | b':'..=b'@' | b'['..=b'`' | b'{'..=b'~'
    )
}

fn is_cm_whitespace(ch: char) -> bool {
    ch.is_whitespace()
}

fn is_cm_punctuation(ch: char) -> bool {
    if ch.is_ascii() {
        is_ascii_punctuation(ch as u8)
    } else {
        !ch.is_alphanumeric() && !ch.is_whitespace()
    }
}

fn unescape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && is_ascii_punctuation(bytes[i + 1]) {
            out.push(bytes[i + 1] as char);
            i += 2;
        } else if bytes[i] == b'&'
            && let Some((dec, consumed)) = decode_html_entity(&s[i..])
        {
            out.push_str(&dec);
            i += consumed;
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn is_uri_autolink(s: &str) -> bool {
    let Some(colon) = s.find(':') else {
        return false;
    };
    let scheme = &s[..colon];
    if scheme.len() < 2 || scheme.len() > 32 {
        return false;
    }
    if !scheme.as_bytes()[0].is_ascii_alphabetic() {
        return false;
    }
    if !scheme
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'))
    {
        return false;
    }
    s[colon + 1..]
        .bytes()
        .all(|b| b > 0x20 && b != b'<' && b != b'>')
}

fn is_email_autolink(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return false;
    }
    local.bytes().all(|b| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'.' | b'!'
                    | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'/'
                    | b'='
                    | b'?'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'{'
                    | b'|'
                    | b'}'
                    | b'~'
                    | b'-'
            )
    }) && domain.split('.').all(|lbl| {
        !lbl.is_empty()
            && lbl.len() <= 63
            && !lbl.starts_with('-')
            && !lbl.ends_with('-')
            && lbl.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

fn match_raw_html_inline(s: &str) -> Option<usize> {
    if !s.starts_with('<') {
        return None;
    }
    // HTML comment
    if let Some(rest) = s.strip_prefix("<!--") {
        let end = rest.find("-->")?;
        return Some(4 + end + 3);
    }
    // Closing tag: </tagname \s*>
    if let Some(rest) = s.strip_prefix("</") {
        let bytes = rest.as_bytes();
        if bytes.is_empty() || !bytes[0].is_ascii_alphabetic() {
            return None;
        }
        let mut i = 1;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
            i += 1;
        }
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'>' {
            return Some(2 + i + 1);
        }
        return None;
    }
    // Opening tag: <tagname (attrs)* /?>
    let bytes = s.as_bytes();
    if bytes.len() < 3 || !bytes[1].is_ascii_alphabetic() {
        return None;
    }
    let mut i = 2;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
        i += 1;
    }
    loop {
        let ws_start = i;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n') {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        if bytes[i] == b'>' {
            return Some(i + 1);
        }
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'>' {
            return Some(i + 2);
        }
        // Attribute must be preceded by whitespace
        if i == ws_start {
            return None;
        }
        if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' || bytes[i] == b':') {
            return None;
        }
        i += 1;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'.' | b':' | b'-'))
        {
            i += 1;
        }
        let eq_scan = i;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n') {
                i += 1;
            }
            if i >= bytes.len() {
                return None;
            }
            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let q = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != q {
                    i += 1;
                }
                if i >= bytes.len() {
                    return None;
                }
                i += 1;
            } else {
                let vstart = i;
                while i < bytes.len()
                    && !matches!(
                        bytes[i],
                        b' ' | b'\t' | b'\n' | b'"' | b'\'' | b'=' | b'<' | b'>' | b'`'
                    )
                {
                    i += 1;
                }
                if i == vstart {
                    return None;
                }
            }
        } else {
            i = eq_scan;
        }
    }
}

fn decode_html_entity(s: &str) -> Option<(String, usize)> {
    let semi = s[1..].find(';')?;
    if semi == 0 || semi > 32 {
        return None;
    }
    let body = &s[1..1 + semi];
    let consumed = semi + 2;
    if let Some(num_str) = body.strip_prefix('#') {
        let cp = if let Some(hex) = num_str.strip_prefix(['x', 'X']) {
            if hex.is_empty() || hex.len() > 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            u32::from_str_radix(hex, 16).ok()?
        } else {
            if num_str.is_empty()
                || num_str.len() > 7
                || !num_str.bytes().all(|b| b.is_ascii_digit())
            {
                return None;
            }
            num_str.parse::<u32>().ok()?
        };
        let ch = if cp == 0 {
            '\u{FFFD}'
        } else {
            char::from_u32(cp).unwrap_or('\u{FFFD}')
        };
        return Some((ch.to_string(), consumed));
    }
    let resolved = match body {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{00A0}",
        "copy" => "\u{00A9}",
        "reg" => "\u{00AE}",
        "mdash" => "\u{2014}",
        "ndash" => "\u{2013}",
        "hellip" => "\u{2026}",
        "bull" => "\u{2022}",
        "middot" => "\u{00B7}",
        "trade" => "\u{2122}",
        "larr" => "\u{2190}",
        "rarr" => "\u{2192}",
        "laquo" => "\u{00AB}",
        "raquo" => "\u{00BB}",
        _ => return None,
    };
    Some((resolved.to_string(), consumed))
}

// ---------------------------------------------------------------------------
// HTML Renderer (exact `org.commonmark` 0.29.0 + `TablesExtension` + `MarkdownNodeRenderer`)
// ---------------------------------------------------------------------------

fn render_blocks(blocks: &[Block], tight_list: bool, out: &mut String) {
    for block in blocks {
        render_block(block, tight_list, out);
    }
}

fn render_block(block: &Block, tight_list: bool, out: &mut String) {
    match block {
        Block::Heading { level, inlines } => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("<h{level}>"));
            render_inlines(inlines, out);
            out.push_str(&format!("</h{level}>\n"));
        }
        Block::Paragraph(inlines) => {
            if tight_list {
                render_inlines(inlines, out);
            } else {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("<p>");
                render_inlines(inlines, out);
                out.push_str("</p>\n");
            }
        }
        Block::CodeBlock { info, literal } => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("<pre><code");
            if let Some(info_str) = info {
                let first_word = info_str.split_whitespace().next().unwrap_or("");
                if !first_word.is_empty() {
                    out.push_str(" class=\"language-");
                    escape_html_into(out, first_word);
                    out.push('"');
                }
            }
            out.push('>');
            escape_html_into(out, literal);
            out.push_str("</code></pre>\n");
        }
        Block::ThematicBreak => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("<hr />\n");
        }
        Block::BlockQuote(children) => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("<blockquote>\n");
            render_blocks(children, false, out);
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("</blockquote>\n");
        }
        Block::List {
            ordered,
            start,
            tight,
            items,
        } => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if *ordered {
                if *start != 1 {
                    out.push_str(&format!("<ol start=\"{start}\">\n"));
                } else {
                    out.push_str("<ol>\n");
                }
            } else {
                out.push_str("<ul>\n");
            }
            for item in items {
                out.push_str("<li>");
                if !item.is_empty() {
                    if !*tight {
                        out.push('\n');
                    }
                    for (idx, child) in item.iter().enumerate() {
                        if *tight && idx > 0 && !out.ends_with('\n') {
                            out.push('\n');
                        }
                        render_block(child, *tight, out);
                    }
                    if !*tight && !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                out.push_str("</li>\n");
            }
            if *ordered {
                out.push_str("</ol>\n");
            } else {
                out.push_str("</ul>\n");
            }
        }
        Block::Table { headers, rows } => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("<table>\n<thead>\n<tr>\n");
            for cell in headers {
                out.push_str("<th");
                write_align_attr(cell.alignment, out);
                out.push('>');
                render_inlines(&cell.inlines, out);
                out.push_str("</th>\n");
            }
            out.push_str("</tr>\n</thead>\n");
            if !rows.is_empty() {
                out.push_str("<tbody>\n");
                for row in rows {
                    out.push_str("<tr>\n");
                    for cell in row {
                        out.push_str("<td");
                        write_align_attr(cell.alignment, out);
                        out.push('>');
                        render_inlines(&cell.inlines, out);
                        out.push_str("</td>\n");
                    }
                    out.push_str("</tr>\n");
                }
                out.push_str("</tbody>\n");
            }
            out.push_str("</table>\n");
        }
        Block::HtmlBlock(raw) => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            escape_html_into(out, raw);
            out.push('\n');
        }
    }
}

fn write_align_attr(align: TableAlignment, out: &mut String) {
    match align {
        TableAlignment::None => {}
        TableAlignment::Left => out.push_str(" align=\"left\""),
        TableAlignment::Center => out.push_str(" align=\"center\""),
        TableAlignment::Right => out.push_str(" align=\"right\""),
    }
}

fn render_inlines(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        render_inline(inline, out);
    }
}

fn render_inline(inline: &Inline, out: &mut String) {
    match inline {
        Inline::Text(t) => escape_html_into(out, t),
        Inline::Code(c) => {
            out.push_str("<code>");
            escape_html_into(out, c);
            out.push_str("</code>");
        }
        Inline::Emphasis(children) => {
            out.push_str("<em>");
            render_inlines(children, out);
            out.push_str("</em>");
        }
        Inline::Strong(children) => {
            out.push_str("<strong>");
            render_inlines(children, out);
            out.push_str("</strong>");
        }
        Inline::Link {
            destination,
            title,
            pkldoc,
            children,
        } => {
            if let Some(meta) = pkldoc {
                // Exact `MarkdownNodeRenderer.visit(Link)` (`Markdown.kt:120-128`)
                let safe_dest = sanitize_href_url(destination);
                let is_short = children.len() == 1
                    && matches!(&children[0], Inline::Text(t) if t == &meta.label);
                if !is_short {
                    out.push_str("<a href=\"");
                    escape_html_into(out, safe_dest);
                    out.push_str("\">");
                    render_inlines(children, out);
                    out.push_str("</a>");
                } else if meta.code_only_if_short {
                    out.push_str("<code>");
                    escape_html_into(out, &meta.label);
                    out.push_str("</code>");
                } else {
                    out.push_str("<a href=\"");
                    escape_html_into(out, safe_dest);
                    out.push_str("\"><code>");
                    escape_html_into(out, &meta.label);
                    out.push_str("</code></a>");
                }
                return;
            }
            let safe_dest = sanitize_href_url(destination);
            out.push_str("<a href=\"");
            escape_html_into(out, safe_dest);
            out.push('"');
            if let Some(t) = title {
                out.push_str(" title=\"");
                escape_html_into(out, t);
                out.push('"');
            }
            out.push('>');
            render_inlines(children, out);
            out.push_str("</a>");
        }
        Inline::Image {
            destination,
            title,
            alt,
        } => {
            let safe_dest = sanitize_href_url(destination);
            out.push_str("<img src=\"");
            escape_html_into(out, safe_dest);
            out.push_str("\" alt=\"");
            let mut alt_plain = String::new();
            collect_alt_text(alt, &mut alt_plain);
            escape_html_into(out, &alt_plain);
            out.push('"');
            if let Some(t) = title {
                out.push_str(" title=\"");
                escape_html_into(out, t);
                out.push('"');
            }
            out.push_str(" />");
        }
        Inline::Autolink { destination, text } => {
            let safe_dest = sanitize_href_url(destination);
            out.push_str("<a href=\"");
            escape_html_into(out, safe_dest);
            out.push_str("\">");
            escape_html_into(out, text);
            out.push_str("</a>");
        }
        Inline::HardBreak => out.push_str("<br />\n"),
        Inline::SoftBreak => out.push('\n'),
        Inline::HtmlInline(raw) => escape_html_into(out, raw),
    }
}

fn collect_alt_text(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text(t) | Inline::Code(t) => out.push_str(t),
            Inline::Autolink { text, .. } => out.push_str(text),
            Inline::Emphasis(ch) | Inline::Strong(ch) | Inline::Link { children: ch, .. } => {
                collect_alt_text(ch, out);
            }
            Inline::Image { alt, .. } => collect_alt_text(alt, out),
            Inline::SoftBreak | Inline::HardBreak => out.push('\n'),
            Inline::HtmlInline(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_ref(label: &str) -> RefLinkResolution {
        RefLinkResolution {
            destination: "not_found".to_string(),
            label: label.to_string(),
            code_only_if_short: false,
        }
    }

    #[test]
    fn renders_gfm_table_matching_commonmark_java() {
        let md = "| Command | Description |\n| --- | --- |\n| `git status` | List all *new or modified* files |\n| `git diff` | Show file differences that **haven't been** staged |";
        let html = render_markdown_trimmed(md, dummy_ref).unwrap();
        let expected = "\
<table>
<thead>
<tr>
<th>Command</th>
<th>Description</th>
</tr>
</thead>
<tbody>
<tr>
<td><code>git status</code></td>
<td>List all <em>new or modified</em> files</td>
</tr>
<tr>
<td><code>git diff</code></td>
<td>Show file differences that <strong>haven't been</strong> staged</td>
</tr>
</tbody>
</table>";
        assert_eq!(html, expected);
    }

    #[test]
    fn renders_nested_list_and_fenced_code_matching_commonmark_java() {
        let md = "\
1. Install it
    1. I promise it's easy.
    2. You'll figure it out.
1. Install it
    ```
    name = \"Pigeon\"
    age = 42
    ```
1. Install it";
        let html = render_markdown_trimmed(md, dummy_ref).unwrap();
        let expected = "\
<ol>
<li>Install it
<ol>
<li>I promise it's easy.</li>
<li>You'll figure it out.</li>
</ol>
</li>
<li>Install it
<pre><code>name = &quot;Pigeon&quot;
age = 42
</code></pre>
</li>
<li>Install it</li>
</ol>";
        assert_eq!(html, expected);
    }

    #[test]
    fn renders_inline_markdown_unwrapping_single_paragraph() {
        assert_eq!(
            render_inline_markdown_text("Spelling mistake.", dummy_ref),
            "Spelling mistake."
        );
    }

    #[test]
    fn round4_escapes_raw_html_blocks_and_inline_html() {
        let block_html = render_markdown_trimmed("<script>alert(1)</script>", dummy_ref).unwrap();
        assert!(
            !block_html.contains("<script>"),
            "raw HTML block must be escaped, got: {block_html}"
        );
        assert!(block_html.contains("&lt;script&gt;"));

        let inline_html =
            render_inline_markdown_text("hello <img src=x onerror=alert(1)> world", dummy_ref);
        assert!(
            !inline_html.contains("<img"),
            "raw inline HTML must be escaped, got: {inline_html}"
        );
        assert!(inline_html.contains("&lt;img"));
    }
}
