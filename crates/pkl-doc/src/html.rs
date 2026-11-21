//! Exact `kotlinx.html` 0.12.0 `appendHTML()` HTML stream writer.
//!
//! Preserves 2-space indentation, inline-vs-block newline rules, void tag rendering
//! (no `/>`), boolean attribute rendering (`defer="defer"`), empty attribute retention
//! (`class=""`), and exact HTML entity escaping (`& < > "`).

/// Minimal HTML writer matching `kotlinx.html.stream.HTMLStreamBuilder(prettyPrint = true)`.
#[derive(Default)]
pub struct HtmlWriter {
    out: String,
    level: usize,
    ln: bool,
}

impl HtmlWriter {
    /// Creates a new document starting with `<!DOCTYPE html>\n` (matching `PageGenerator.kt:60-61`).
    pub fn new_document() -> Self {
        Self {
            out: "<!DOCTYPE html>\n".to_string(),
            level: 0,
            ln: true,
        }
    }

    pub fn finish(self) -> String {
        self.out
    }

    fn indent(&mut self) {
        for _ in 0..self.level {
            self.out.push_str("  ");
        }
    }

    fn write_attrs(&mut self, attrs: &[(&str, &str)]) {
        for &(k, v) in attrs {
            self.out.push(' ');
            self.out.push_str(k);
            self.out.push_str("=\"");
            escape_html_into(&mut self.out, v);
            self.out.push('"');
        }
    }

    /// Writes a block element with children: `<tag attrs>...children...</tag>\n`.
    pub fn block<F>(&mut self, tag: &str, attrs: &[(&str, &str)], body: F)
    where
        F: FnOnce(&mut Self),
    {
        if !self.ln {
            self.out.push('\n');
        }
        self.indent();
        self.out.push('<');
        self.out.push_str(tag);
        self.write_attrs(attrs);
        self.out.push('>');
        self.ln = false;
        self.level += 1;

        body(self);

        self.level -= 1;
        if self.ln {
            self.indent();
        }
        self.out.push_str("</");
        self.out.push_str(tag);
        self.out.push_str(">\n");
        self.ln = true;
    }

    /// Writes a block void element (e.g. `<meta ...>\n`, `<link ...>\n`).
    pub fn block_void(&mut self, tag: &str, attrs: &[(&str, &str)]) {
        if !self.ln {
            self.out.push('\n');
        }
        self.indent();
        self.out.push('<');
        self.out.push_str(tag);
        self.write_attrs(attrs);
        self.out.push_str(">\n");
        self.ln = true;
    }

    /// Writes an inline element with children (e.g. `<a>`, `<span>`, `<i>`, `<label>`, `<code>`).
    pub fn inline<F>(&mut self, tag: &str, attrs: &[(&str, &str)], body: F)
    where
        F: FnOnce(&mut Self),
    {
        self.out.push('<');
        self.out.push_str(tag);
        self.write_attrs(attrs);
        self.out.push('>');
        self.ln = false;

        body(self);

        self.out.push_str("</");
        self.out.push_str(tag);
        self.out.push('>');
        self.ln = false;
    }

    /// Writes an inline void element (e.g. `<input ...>`).
    pub fn inline_void(&mut self, tag: &str, attrs: &[(&str, &str)]) {
        self.out.push('<');
        self.out.push_str(tag);
        self.write_attrs(attrs);
        self.out.push('>');
        self.ln = false;
    }

    /// Writes escaped text content (`& < > "`).
    pub fn text(&mut self, text: &str) {
        escape_html_into(&mut self.out, text);
        self.ln = false;
    }

    /// Writes raw unescaped HTML (`unsafe { raw(...) }`).
    pub fn raw(&mut self, raw: &str) {
        self.out.push_str(raw);
        self.ln = false;
    }
}

/// Escapes HTML text and attribute values matching `kotlinx.html` (`& < > "`; leaves `'` unescaped).
pub fn escape_html_into(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_single_package_redirect_html_byte_for_byte() {
        let mut w = HtmlWriter::new_document();
        let url = "com.package1/current/index.html";
        w.block("html", &[("lang", "en-US")], |w| {
            w.block("head", &[], |w| {
                w.block_void("meta", &[("charset", "UTF-8")]);
                w.block("title", &[], |w| w.text("Single Package Docs"));
                w.block("script", &[], |w| {
                    w.raw(&format!("window.location.replace(\"{url}\");"))
                });
            });
            w.block("body", &[], |w| {
                w.block("main", &[], |w| {
                    w.block("p", &[], |w| {
                        w.inline("a", &[("href", url)], |w| w.text(url));
                    });
                });
            });
        });
        let expected = include_str!(
            "../../../external/pkl/pkl-doc/src/test/files/SinglePackageTest/output/index.html"
        );
        assert_eq!(w.finish(), expected);
    }
}
