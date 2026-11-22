//! Exact port of `PageGenerator.kt`, `MainOrPackagePageGenerator.kt`, `MainPageGenerator.kt`,
//! `PackagePageGenerator.kt`, `ModuleOrClassPageGenerator.kt`, `ModulePageGenerator.kt`,
//! `ClassPageGenerator.kt`, and `HtmlGenerator.kt`.

use crate::assets::write_all_static_resources;
use crate::html::HtmlWriter;
use crate::json::push_json_string;
use crate::markdown::{render_inline_markdown_text, render_markdown_trimmed};
use crate::model::{
    DocGeneratorError, DocModule, DocPackage, DocsiteInfo, PackageDependency,
    annotations_also_known_as, annotations_is_unlisted, class_has_listed_method,
    class_has_listed_property, create_deprecated_annotation, is_base_annotation,
    schema_has_listed_class, schema_has_listed_type_alias,
};
use crate::package_data::{ElementRef, PackageData};
use crate::scope::{
    DocScope, ModuleScope, PackageScope, SiteScope, make_class_scope, make_type_alias_scope,
};
use crate::util::{
    ParsedUri, as_module_name, encode_path, file_uri_to_path_buf, get_doc_comment_overflow,
    get_doc_comment_summary, maybe_quote_identifier, relativize_uri, resolve_uri,
    uri_encode_component,
};
use pkl_runtime::pkl_binary_decode::{DecodedObject, DecodedValue};
use pkl_runtime::schema::{
    Modifier, ModuleSchema, PClass, PClassMethod, PType, SchemaStore, SourceLocation, TypeAlias,
    TypeParameter, Variance,
};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const KNOWN_SUBTYPES: &str = "known-subtypes";
const KNOWN_USAGES: &str = "known-usages";
const KNOWN_VERSIONS: &str = "known-versions";
const RUNTIME_DATA_CLASSES: &str = "runtime-data hidden";

/// Generates all HTML pages and static web resources (`HtmlGenerator.kt`).
pub struct HtmlGenerator<'a> {
    docsite_info: &'a DocsiteInfo,
    site_scope: Arc<SiteScope>,
    output_dir: &'a Path,
    is_test_mode: bool,
    is_single_package_site: bool,
}

impl<'a> HtmlGenerator<'a> {
    pub fn new(
        docsite_info: &'a DocsiteInfo,
        doc_packages: Vec<DocPackage>,
        imported_modules: BTreeMap<String, ModuleSchema>,
        output_dir: &'a Path,
        is_test_mode: bool,
        is_single_package_site: bool,
    ) -> Self {
        let site_scope = SiteScope::new(
            doc_packages,
            docsite_info.overview_imports.clone(),
            imported_modules,
            output_dir.to_path_buf(),
        );
        Self {
            docsite_info,
            site_scope,
            output_dir,
            is_test_mode,
            is_single_package_site,
        }
    }

    pub(crate) fn collect_package_writes(
        &self,
        pkg_index: usize,
        writes: &mut Vec<crate::generator::PendingFile>,
        logs: &mut Vec<String>,
    ) -> Result<(), DocGeneratorError> {
        let pkg_scope = self.site_scope.package_scope(pkg_index);
        let doc_package = &self.site_scope.doc_packages[pkg_index];

        // Package page
        {
            let page_scope = DocScope::Package {
                site: Arc::clone(&self.site_scope),
                package: Arc::clone(&pkg_scope),
            };
            let html = render_package_page(
                self.docsite_info,
                &self.site_scope,
                doc_package,
                &pkg_scope,
                &page_scope,
                self.is_single_package_site,
            );
            queue_page(&page_scope.url(), html, writes, logs)?;
        }

        // Module pages & class pages
        for doc_module in &doc_package.doc_modules {
            if doc_module.is_unlisted {
                continue;
            }
            let module_scope = pkg_scope.get_module(doc_module.name());
            let mod_page_scope = DocScope::Module {
                site_pkg: Some(Arc::clone(&pkg_scope)),
                module: Arc::clone(&module_scope),
            };
            let mod_html = render_module_page(
                self.docsite_info,
                &self.site_scope,
                doc_package,
                doc_module,
                &pkg_scope,
                &module_scope,
                &mod_page_scope,
                self.is_test_mode,
                self.is_single_package_site,
            );
            queue_page(&mod_page_scope.url(), mod_html, writes, logs)?;

            for (_, cid) in &doc_module.schema.classes {
                let clazz = doc_module.schema.store.class(*cid);
                if annotations_is_unlisted(&clazz.annotations) {
                    continue;
                }
                let class_page_scope =
                    make_class_scope(&self.site_scope, &pkg_scope, &module_scope, clazz);
                let class_html = render_class_page(
                    self.docsite_info,
                    &self.site_scope,
                    doc_package,
                    doc_module,
                    clazz,
                    &pkg_scope,
                    &module_scope,
                    &class_page_scope,
                    self.is_test_mode,
                    self.is_single_package_site,
                );
                queue_page(&class_page_scope.url(), class_html, writes, logs)?;
            }
        }
        Ok(())
    }

    pub fn generate_package(
        &self,
        pkg_index: usize,
        console_out: &mut dyn Write,
    ) -> Result<(), DocGeneratorError> {
        let mut writes = Vec::new();
        let mut logs = Vec::new();
        self.collect_package_writes(pkg_index, &mut writes, &mut logs)?;
        crate::generator::write_files_parallel(&writes)?;
        for line in &logs {
            let _ = console_out.write_all(line.as_bytes());
        }
        Ok(())
    }

    pub(crate) fn collect_site_page_write(
        &self,
        packages: &[PackageData],
        writes: &mut Vec<crate::generator::PendingFile>,
        logs: &mut Vec<String>,
    ) -> Result<(), DocGeneratorError> {
        let html = render_main_page(
            self.docsite_info,
            packages,
            &self.site_scope,
            self.is_single_package_site,
        );
        queue_page(&self.site_scope.url, html, writes, logs)
    }

    pub fn generate_site_page_only(
        &self,
        packages: &[PackageData],
        console_out: &mut dyn Write,
    ) -> Result<(), DocGeneratorError> {
        let mut writes = Vec::new();
        let mut logs = Vec::new();
        self.collect_site_page_write(packages, &mut writes, &mut logs)?;
        crate::generator::write_files_parallel(&writes)?;
        for line in &logs {
            let _ = console_out.write_all(line.as_bytes());
        }
        Ok(())
    }

    pub fn generate_site(
        &self,
        packages: &[PackageData],
        console_out: &mut dyn Write,
    ) -> Result<(), DocGeneratorError> {
        self.generate_site_page_only(packages, console_out)?;
        write_all_static_resources(self.output_dir)
            .map_err(|e| DocGeneratorError::user(e.to_string()))?;
        Ok(())
    }

    pub(crate) fn doc_packages(&self) -> &[DocPackage] {
        &self.site_scope.doc_packages
    }
}

fn queue_page(
    url: &str,
    html: String,
    writes: &mut Vec<crate::generator::PendingFile>,
    logs: &mut Vec<String>,
) -> Result<(), DocGeneratorError> {
    let path: PathBuf = file_uri_to_path_buf(url)
        .ok_or_else(|| DocGeneratorError::bug(format!("Non-file page URI `{url}`")))?;
    let display_url = url
        .strip_prefix("file://")
        .map_or_else(|| url.to_string(), |rest| format!("file:{rest}"));
    writes.push(crate::generator::PendingFile {
        path,
        bytes: crate::generator::FileBytes::Owned(html.into_bytes()),
    });
    logs.push(format!("Wrote file {display_url}\r"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Extra member info representation (`MemberInfoKey` + content renderer)
// ---------------------------------------------------------------------------

enum ExtraInfoBody {
    Text(String),
    UriCopy(String),
    Link { href: String, text: String },
    Dependencies(Vec<(String, String, Option<String>)>),
    Examples(Vec<(String, String)>),
    RuntimeData { id: &'static str },
}

struct ExtraInfoItem {
    name: String,
    classes: &'static str,
    body: ExtraInfoBody,
}

impl ExtraInfoItem {
    fn plain(name: impl Into<String>, body: ExtraInfoBody) -> Self {
        Self {
            name: name.into(),
            classes: "",
            body,
        }
    }

    fn runtime(name: &'static str, id: &'static str) -> Self {
        Self {
            name: name.to_string(),
            classes: RUNTIME_DATA_CLASSES,
            body: ExtraInfoBody::RuntimeData { id },
        }
    }
}

// ---------------------------------------------------------------------------
// `MemberDocs` (`PageGenerator.kt:691-834`)
// ---------------------------------------------------------------------------

struct MemberDocs {
    summary: Option<String>,
    show_summary_only: bool,
    overflow: Option<String>,
    deprecated_replace_with: Option<String>,
    deprecated_message_html: Option<String>,
    is_deprecated_member: bool,
    also_known_as: Option<Vec<String>>,
    extra_member_info: Vec<ExtraInfoItem>,
    is_declaration: bool,
    summary_moves_down: bool,
    is_expandable: bool,
}

impl MemberDocs {
    fn new(
        site: &Arc<SiteScope>,
        page_scope: &DocScope,
        doc_comment: Option<&str>,
        doc_scope: &DocScope,
        annotations: &[DecodedObject],
        is_declaration: bool,
        extra_member_info: Vec<ExtraInfoItem>,
    ) -> Self {
        let summary = doc_comment
            .and_then(|c| get_doc_comment_summary(Some(c)))
            .and_then(|s| {
                let mut resolver = doc_scope.make_markdown_ref_resolver(site, page_scope);
                render_markdown_trimmed(&s, &mut resolver)
            });

        let show_summary_only = !is_declaration && doc_scope.is_page_scope();

        let overflow = if show_summary_only {
            None
        } else {
            doc_comment
                .and_then(|c| get_doc_comment_overflow(Some(c)))
                .and_then(|o| {
                    let mut resolver = doc_scope.make_markdown_ref_resolver(site, page_scope);
                    render_markdown_trimmed(&o, &mut resolver)
                })
        };

        let deprecated_ann = annotations
            .iter()
            .find(|a| is_base_annotation(a, "Deprecated"));
        let is_deprecated_member = deprecated_ann.is_some();
        let deprecated_message = deprecated_ann.and_then(|a| match a.get_property("message") {
            Some(DecodedValue::String(s)) => Some(s.to_string()),
            _ => None,
        });
        let deprecated_replace_with =
            deprecated_ann.and_then(|a| match a.get_property("replaceWith") {
                Some(DecodedValue::String(s)) => Some(s.to_string()),
                _ => None,
            });
        let deprecated_message_html = deprecated_message.as_deref().map(|msg| {
            let mut resolver = doc_scope.make_markdown_ref_resolver(site, page_scope);
            render_inline_markdown_text(msg, &mut resolver)
        });

        let also_known_as = annotations_also_known_as(annotations);
        let has_member_info = !extra_member_info.is_empty() || also_known_as.is_some();
        let summary_moves_down = summary.is_some() && is_deprecated_member;
        let is_expandable = !show_summary_only
            && (overflow.is_some() || summary_moves_down || (has_member_info && !is_declaration));

        Self {
            summary,
            show_summary_only,
            overflow,
            deprecated_replace_with,
            deprecated_message_html,
            is_deprecated_member,
            also_known_as,
            extra_member_info,
            is_declaration,
            summary_moves_down,
            is_expandable,
        }
    }

    fn render_expand_icon(&self, w: &mut HtmlWriter) {
        if self.is_expandable {
            w.inline(
                "i",
                &[("class", "material-icons expandable-docs-icon")],
                |w| w.text("expand_more"),
            );
        }
    }

    fn render_doc_comment(&self, w: &mut HtmlWriter) {
        if self.is_deprecated_member {
            w.block("div", &[("class", "doc-comment")], |w| {
                if let Some(ref html) = self.deprecated_message_html {
                    w.text("Deprecated: ");
                    w.raw(html);
                } else {
                    w.text("Deprecated.");
                }
                if let Some(ref repl) = self.deprecated_replace_with {
                    w.text(" Replace with: ");
                    w.inline("code", &[], |w| w.text(repl));
                }
            });
        } else if let Some(ref summary) = self.summary {
            w.block("div", &[("class", "doc-comment")], |w| {
                w.raw(summary);
            });
        }

        if self.show_summary_only {
            return;
        }

        let has_member_info = !self.extra_member_info.is_empty() || self.also_known_as.is_some();
        if has_member_info {
            let dl_class = if self.is_expandable && !self.is_declaration {
                "member-info expandable hidden collapsed"
            } else {
                "member-info"
            };
            w.block("dl", &[("class", dl_class)], |w| {
                for item in &self.extra_member_info {
                    w.block("dt", &[("class", item.classes)], |w| {
                        w.text(&item.name);
                        w.text(":");
                    });
                    match &item.body {
                        ExtraInfoBody::Text(t) => {
                            w.block("dd", &[], |w| w.text(t));
                        }
                        ExtraInfoBody::UriCopy(u) => {
                            w.block("dd", &[], |w| {
                                w.inline("span", &[("class", "import-uri")], |w| w.text(u));
                                w.inline(
                                    "i",
                                    &[("class", "copy-uri-button material-icons")],
                                    |w| w.text("content_copy"),
                                );
                            });
                        }
                        ExtraInfoBody::Link { href, text } => {
                            w.block("dd", &[], |w| {
                                w.inline("a", &[("href", href)], |w| w.text(text));
                            });
                        }
                        ExtraInfoBody::Dependencies(deps) => {
                            w.block("dd", &[], |w| {
                                for (idx, (name, version, link)) in deps.iter().enumerate() {
                                    if idx > 0 {
                                        w.text(", ");
                                    }
                                    if let Some(href) = link {
                                        w.inline("a", &[("href", href)], |w| {
                                            w.text(name);
                                            w.text(":");
                                            w.text(version);
                                        });
                                    } else {
                                        w.inline("span", &[], |w| {
                                            w.text(&format!("{name}:{version}"));
                                        });
                                    }
                                }
                            });
                        }
                        ExtraInfoBody::Examples(examples) => {
                            w.block("dd", &[], |w| {
                                for (idx, (href, short_name)) in examples.iter().enumerate() {
                                    if idx > 0 {
                                        w.text(", ");
                                    }
                                    w.inline("a", &[("href", href)], |w| w.text(short_name));
                                }
                            });
                        }
                        ExtraInfoBody::RuntimeData { id } => {
                            w.block("dd", &[("id", id), ("class", RUNTIME_DATA_CLASSES)], |_| {});
                        }
                    }
                }

                if let Some(ref names) = self.also_known_as {
                    w.block("dt", &[], |w| w.text("Also known as:"));
                    w.block("dd", &[], |w| {
                        for (idx, name) in names.iter().enumerate() {
                            if idx > 0 {
                                w.text(", ");
                            }
                            w.inline("code", &[], |w| w.text(name));
                        }
                    });
                }
            });
        }

        if self.summary_moves_down || self.overflow.is_some() {
            w.block(
                "div",
                &[("class", "doc-comment expandable hidden collapsed")],
                |w| {
                    if self.summary_moves_down
                        && let Some(ref summary) = self.summary
                    {
                        w.raw(summary);
                    }
                    if let Some(ref overflow) = self.overflow {
                        w.raw(overflow);
                    }
                },
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Shared `PageGenerator` rendering helpers
// ---------------------------------------------------------------------------

struct PageCtx<'a> {
    docsite_info: &'a DocsiteInfo,
    site: &'a Arc<SiteScope>,
    page_scope: &'a DocScope,
    package_version: Option<&'a str>,
    is_single_package_site: bool,
    is_test_mode: bool,
}

impl<'a> PageCtx<'a> {
    fn relative_site_url(&self) -> String {
        self.page_scope.relative_site_url(self.site)
    }

    fn render_html_head(&self, w: &mut HtmlWriter, page_title: &str) {
        let rel_site = self.relative_site_url();
        w.block("head", &[], |w| {
            w.block_void("meta", &[("charset", "UTF-8")]);
            w.block("title", &[], |w| w.text(page_title));
            let pkldoc_js = resolve_uri(&rel_site, "scripts/pkldoc.js");
            w.block(
                "script",
                &[("src", pkldoc_js.as_str()), ("defer", "defer")],
                |_| {},
            );

            if let Some(version) = self.package_version
                && let (Some(per_pkg), Some(per_pkg_ver)) = (
                    self.page_scope.per_package_data_url(),
                    self.page_scope.per_package_version_data_url(),
                )
            {
                let page_url = self.page_scope.url();
                let pkg_ver_url = relativize_uri(&per_pkg, &page_url);
                let per_pkg_ver_url = relativize_uri(&per_pkg_ver, &page_url);
                let mut version_json = String::new();
                push_json_string(&mut version_json, version);
                // Exact 12-space trailing indent before `</script>` matching `PageGenerator.kt:99-106`.
                let script_body = format!(
                    "\n      import perPackageData from \"{pkg_ver_url}\" with {{ type: \"json\" }}\n      import perPackageVersionData from \"{per_pkg_ver_url}\" with {{ type: \"json\" }}\n\n      runtimeData.knownVersions(perPackageData.knownVersions, {version_json});\n      runtimeData.knownUsagesOrSubtypes(\"known-subtypes\", perPackageVersionData.knownSubtypes);\n      runtimeData.knownUsagesOrSubtypes(\"known-usages\", perPackageVersionData.knownUsages);\n            "
                );
                w.block("script", &[("type", "module")], |w| w.raw(&script_body));
            }

            let scroll_js = resolve_uri(&rel_site, "scripts/scroll-into-view.min.js");
            w.block(
                "script",
                &[("src", scroll_js.as_str()), ("defer", "defer")],
                |_| {},
            );

            let css = resolve_uri(&rel_site, "styles/pkldoc.css");
            w.block_void(
                "link",
                &[
                    ("href", css.as_str()),
                    ("media", "screen"),
                    ("type", "text/css"),
                    ("rel", "stylesheet"),
                ],
            );

            let fav_svg = resolve_uri(&rel_site, "images/favicon.svg");
            w.block_void(
                "link",
                &[
                    ("rel", "icon"),
                    ("type", "image/svg+xml"),
                    ("href", fav_svg.as_str()),
                ],
            );

            let apple_icon = resolve_uri(&rel_site, "images/apple-touch-icon.png");
            w.block_void(
                "link",
                &[
                    ("rel", "apple-touch-icon"),
                    ("sizes", "180x180"),
                    ("href", apple_icon.as_str()),
                ],
            );

            let fav_32 = resolve_uri(&rel_site, "images/favicon-32x32.png");
            w.block_void(
                "link",
                &[
                    ("rel", "icon"),
                    ("type", "image/png"),
                    ("sizes", "32x32"),
                    ("href", fav_32.as_str()),
                ],
            );

            let fav_16 = resolve_uri(&rel_site, "images/favicon-16x16.png");
            w.block_void(
                "link",
                &[
                    ("rel", "icon"),
                    ("type", "image/png"),
                    ("sizes", "16x16"),
                    ("href", fav_16.as_str()),
                ],
            );
        });
    }

    fn render_page_header(
        &self,
        w: &mut HtmlWriter,
        package_name: Option<&str>,
        package_version: Option<&str>,
        module_name: Option<&str>,
        class_name: Option<&str>,
    ) {
        let rel_site = self.relative_site_url();
        let root_slash_count = ParsedUri::parse(&rel_site)
            .decoded_path()
            .matches('/')
            .count();
        let root_url_prefix = "../".repeat(root_slash_count);

        let pkg_url_prefix = if package_name.is_some() {
            let rel_pkg = self.page_scope.relative_package_url().unwrap_or_default();
            let count = ParsedUri::parse(&rel_pkg)
                .decoded_path()
                .matches('/')
                .count();
            Some("../".repeat(count))
        } else {
            None
        };

        w.block("header", &[], |w| {
            if let Some(ref title) = self.docsite_info.title {
                w.block("div", &[("id", "doc-title")], |w| {
                    w.inline("a", &[("href", rel_site.as_str())], |w| w.text(title));
                });
            }

            w.block("div", &[("id", "search")], |w| {
                w.inline("label", &[("for", "search-input")], |w| {
                    w.inline(
                        "i",
                        &[("id", "search-icon"), ("class", "material-icons")],
                        |w| w.text("search"),
                    );
                });

                let placeholder = if package_name.is_none() {
                    "Click or press 'S' to search"
                } else {
                    "Click or press 'S' to search this package"
                };

                let mut attrs: Vec<(&str, &str)> = vec![
                    ("id", "search-input"),
                    ("type", "search"),
                    ("placeholder", placeholder),
                    ("autocomplete", "off"),
                ];
                if let Some(pkg_name) = package_name {
                    attrs.push(("data-package-name", pkg_name));
                    attrs.push((
                        "data-package-version",
                        package_version.expect("package_version required when package_name set"),
                    ));
                    attrs.push((
                        "data-package-url-prefix",
                        pkg_url_prefix.as_deref().unwrap_or(""),
                    ));
                }
                if let Some(mname) = module_name {
                    attrs.push(("data-module-name", mname));
                }
                if let Some(cname) = class_name {
                    attrs.push(("data-class-name", cname));
                }
                attrs.push(("data-root-url-prefix", root_url_prefix.as_str()));
                w.inline_void("input", &attrs);
            });
        });
    }

    fn render_parent_links(
        &self,
        w: &mut HtmlWriter,
        parent_pkg: Option<&Arc<PackageScope>>,
        parent_module: Option<&Arc<ModuleScope>>,
    ) {
        let mut is_first = true;
        let mut render_link = |w: &mut HtmlWriter, text: &str, url: &str| {
            if is_first {
                is_first = false;
            } else {
                w.text(" > ");
            }
            w.inline(
                "a",
                &[("class", "declaration-parent-link"), ("href", url)],
                |w| w.text(text),
            );
        };

        if !self.is_single_package_site {
            let site_title = self.docsite_info.title.as_deref().unwrap_or("Pkldoc");
            let rel_site = self.relative_site_url();
            render_link(w, site_title, &rel_site);
        }
        if let Some(pkg) = parent_pkg {
            let rel = relativize_uri(&pkg.url, &self.page_scope.url());
            render_link(w, pkg.name(), &rel);
        }
        if let Some(module) = parent_module {
            let rel = relativize_uri(&module.url, &self.page_scope.url());
            render_link(w, &module.module.module_name, &rel);
        }
    }

    fn render_member_group_links(&self, w: &mut HtmlWriter, groups: &[(&str, &str, bool)]) {
        w.block("ul", &[("class", "member-group-links")], |w| {
            for &(name, href, show) in groups {
                if show {
                    w.block("li", &[], |w| {
                        w.inline("a", &[("href", href)], |w| w.text(name));
                    });
                }
            }
        });
    }

    fn render_anchor(&self, w: &mut HtmlWriter, anchor_id: &str, css_class: &str) {
        let id = uri_encode_component(anchor_id);
        w.block("div", &[("id", id.as_str()), ("class", css_class)], |w| {
            w.text(" ");
        });
    }

    fn render_class_anchors(&self, w: &mut HtmlWriter, clazz: &PClass) {
        for (idx, param) in clazz.type_parameters.iter().enumerate() {
            let cls = format!("anchor-param{}", idx + 1);
            self.render_anchor(w, &param.name, &cls);
        }
    }

    fn render_type_alias_anchors(&self, w: &mut HtmlWriter, alias: &TypeAlias) {
        let base_id = alias.simple_name();
        self.render_anchor(w, base_id, "anchor");
        for (idx, param) in alias.type_parameters.iter().enumerate() {
            let id = format!("{base_id}.{}", param.name);
            let cls = format!("anchor-param{}", idx + 1);
            self.render_anchor(w, &id, &cls);
        }
    }

    fn render_method_anchors(&self, w: &mut HtmlWriter, method: &PClassMethod) {
        let base_id = format!("{}()", method.simple_name);
        self.render_anchor(w, &base_id, "anchor");
        let mut idx = 0usize;
        for tp in &method.type_parameters {
            idx += 1;
            let id = format!("{base_id}.{}", tp.name);
            let cls = format!("anchor-param{idx}");
            self.render_anchor(w, &id, &cls);
        }
        for (pname, _) in &method.parameters {
            idx += 1;
            let id = format!("{base_id}.{pname}");
            let cls = format!("anchor-param{idx}");
            self.render_anchor(w, &id, &cls);
        }
    }

    fn render_self_link(&self, w: &mut HtmlWriter, member_name: &str) {
        let href = format!("#{}", uri_encode_component(member_name));
        w.inline(
            "a",
            &[
                ("class", "member-selflink material-icons"),
                ("href", href.as_str()),
            ],
            |w| w.text("link"),
        );
    }

    fn render_modifiers(&self, w: &mut HtmlWriter, modifiers: &[Modifier], additional: &[&str]) {
        for m in modifiers {
            w.text(m.as_str());
            w.text(" ");
        }
        for add in additional {
            w.text(add);
            w.text(" ");
        }
    }

    fn render_module_name(&self, w: &mut HtmlWriter, module_name: &str) {
        if let Some(rel_url) = self
            .page_scope
            .resolve_module_name_to_relative_doc_url(self.site, module_name)
        {
            w.inline(
                "a",
                &[("href", rel_url.as_str()), ("class", "name-ref")],
                |w| w.text(module_name),
            );
        } else {
            w.inline("span", &[("class", "member-ref")], |w| w.text(module_name));
        }
    }

    fn render_class_name(&self, w: &mut HtmlWriter, clazz: &PClass, css_class: &str) {
        let display = class_simple_display_name(clazz);
        let ident = maybe_quote_identifier(display);
        if let Some(mod_doc_url) = self
            .page_scope
            .resolve_module_name_to_doc_url(self.site, clazz.module_name())
        {
            let target_url = if clazz.is_module_class() {
                mod_doc_url
            } else {
                resolve_uri(
                    &mod_doc_url,
                    &format!(
                        "{}.html",
                        uri_encode_component(&encode_path(clazz.simple_name()))
                    ),
                )
            };
            let rel = relativize_uri(&target_url, &self.page_scope.url());
            w.inline("a", &[("href", rel.as_str()), ("class", css_class)], |w| {
                w.text(&ident);
            });
        } else {
            w.inline("span", &[("class", css_class)], |w| w.text(&ident));
        }
    }

    fn render_type_alias_name(&self, w: &mut HtmlWriter, alias: &TypeAlias, css_class: &str) {
        let ident = maybe_quote_identifier(alias.simple_name());
        if let Some(mod_doc_url) = self
            .page_scope
            .resolve_module_name_to_doc_url(self.site, alias.module_name())
        {
            let target_url = resolve_uri(&mod_doc_url, &format!("#{}", alias.simple_name()));
            let rel = relativize_uri(&target_url, &self.page_scope.url());
            w.inline("a", &[("href", rel.as_str()), ("class", css_class)], |w| {
                w.text(&ident);
            });
        } else {
            w.inline("span", &[("class", css_class)], |w| w.text(&ident));
        }
    }

    fn render_class_extends_clause(
        &self,
        w: &mut HtmlWriter,
        store: &SchemaStore,
        clazz: &PClass,
        curr_scope: &DocScope,
    ) {
        let Some(sup_id) = clazz.superclass else {
            return;
        };
        let sup = store.class(sup_id);
        if sup.info.module_uri == "pkl:base" && sup.info.simple_name == "Typed" {
            return;
        }
        w.text(" extends ");
        if let Some(ref supertype) = clazz.supertype {
            self.render_type(w, store, supertype, curr_scope, false);
        }
    }

    fn render_module_amends_or_extends_clause(&self, w: &mut HtmlWriter, module: &ModuleSchema) {
        if let Some(ref sup) = module.supermodule {
            if module.is_amend {
                w.text(" amends ");
            } else {
                w.text(" extends ");
            }
            self.render_module_name(w, &sup.module_name);
        }
    }

    fn render_type(
        &self,
        w: &mut HtmlWriter,
        store: &SchemaStore,
        ty: &PType,
        curr_scope: &DocScope,
        is_nested: bool,
    ) {
        match ty {
            PType::Unknown => w.text("unknown"),
            PType::Nothing => w.text("nothing"),
            PType::Module => w.text("module"),
            PType::StringLiteral(lit) => w.text(&format!("\"{lit}\"")),
            PType::Class {
                class_id,
                type_arguments,
                ..
            } => {
                let pclass = store.class(*class_id);
                self.render_class_name(w, pclass, "name-ref");
                self.render_type_arguments(w, store, type_arguments, curr_scope);
            }
            PType::Nullable(inner) => {
                self.render_type(w, store, inner, curr_scope, true);
                w.text("?");
            }
            PType::Union(elems) => {
                if is_nested {
                    w.text("(");
                }
                for (idx, elem) in elems.iter().enumerate() {
                    if idx > 0 {
                        w.text("|");
                    }
                    self.render_type(w, store, elem, curr_scope, true);
                }
                if is_nested {
                    w.text(")");
                }
            }
            PType::Function {
                parameter_types,
                return_type,
            } => {
                w.text("(");
                for (idx, pt) in parameter_types.iter().enumerate() {
                    if idx > 0 {
                        w.text(", ");
                    }
                    self.render_type(w, store, pt, curr_scope, true);
                }
                w.text(")");
                w.text(" -> ");
                self.render_type(w, store, return_type, curr_scope, true);
            }
            PType::Constrained {
                base_type,
                constraints,
            } => {
                self.render_type(w, store, base_type, curr_scope, true);
                w.text("(");
                for (idx, c) in constraints.iter().enumerate() {
                    if idx > 0 {
                        w.text(", ");
                    }
                    w.text(c);
                }
                w.text(")");
            }
            PType::Alias {
                alias_id,
                type_arguments,
                ..
            } => {
                let alias = store.type_alias(*alias_id);
                self.render_type_alias_name(w, alias, "name-ref");
                self.render_type_arguments(w, store, type_arguments, curr_scope);
            }
            PType::TypeVariable(tp) => {
                if let Some(resolved) = curr_scope.resolve_variable(self.site, &tp.name)
                    && resolved.is_parameter_scope()
                {
                    let href = resolved.url_relative_to(self.page_scope);
                    w.inline(
                        "a",
                        &[("href", href.as_str()), ("class", "name-ref")],
                        |w| {
                            w.text(&tp.name);
                        },
                    );
                } else {
                    w.inline("span", &[("class", "name-ref")], |w| w.text(&tp.name));
                }
            }
        }
    }

    fn render_type_arguments(
        &self,
        w: &mut HtmlWriter,
        store: &SchemaStore,
        args: &[PType],
        curr_scope: &DocScope,
    ) {
        if args.is_empty() {
            return;
        }
        w.text("<");
        for (idx, arg) in args.iter().enumerate() {
            if idx > 0 {
                w.text(", ");
            }
            self.render_type(w, store, arg, curr_scope, true);
        }
        w.text(">");
    }

    fn render_type_parameters(&self, w: &mut HtmlWriter, params: &[TypeParameter]) {
        if params.is_empty() {
            return;
        }
        w.text("<");
        for (idx, param) in params.iter().enumerate() {
            if idx > 0 {
                w.text(", ");
            }
            match param.variance {
                Variance::Contravariant => w.text("in "),
                Variance::Covariant => w.text("out "),
                Variance::Invariant => {}
            }
            let cls = format!("param{}", idx + 1);
            w.inline("a", &[("class", cls.as_str())], |w| w.text(&param.name));
        }
        w.text(">");
    }

    fn render_method_parameters(
        &self,
        w: &mut HtmlWriter,
        store: &SchemaStore,
        method: &PClassMethod,
        method_scope: &DocScope,
    ) {
        w.text("(");
        let index_offset = method.type_parameters.len();
        for (idx, (name, ty)) in method.parameters.iter().enumerate() {
            if idx > 0 {
                w.text(", ");
            }
            let cls = format!("param{}", index_offset + idx + 1);
            w.inline("span", &[("class", cls.as_str())], |w| w.text(name));
            w.text(": ");
            self.render_type(w, store, ty, method_scope, false);
        }
        w.text(")");
    }

    fn render_member_source_link(
        &self,
        w: &mut HtmlWriter,
        module_name: &str,
        source_location: SourceLocation,
    ) {
        let loc = if self.is_test_mode {
            SourceLocation {
                start_line: 123,
                end_line: 456,
            }
        } else {
            source_location
        };
        let Some(url) =
            self.page_scope
                .resolve_module_name_to_source_url(self.site, module_name, loc)
        else {
            return;
        };
        w.inline(
            "a",
            &[("class", "member-source-link"), ("href", url.as_str())],
            |w| w.text("Source"),
        );
    }

    fn render_show_inherited_button(&self, w: &mut HtmlWriter) {
        w.inline("span", &[("class", "toggle-inherited-members")], |w| {
            w.text("(");
            w.inline(
                "span",
                &[("class", "toggle-inherited-members-link button-link")],
                |w| w.text("show inherited"),
            );
            w.text(")");
        });
    }

    fn render_class_context(&self, w: &mut HtmlWriter, owner: &PClass) {
        if matches!(self.page_scope, DocScope::Class { .. }) {
            w.inline("span", &[("class", "context")], |w| {
                self.render_class_name(w, owner, "name-ref");
                w.text(".");
            });
        }
    }

    fn render_module_context(&self, w: &mut HtmlWriter, module_name: &str) {
        w.inline("span", &[("class", "context")], |w| {
            w.text(" (");
            self.render_module_name(w, module_name);
            w.text(")");
        });
    }

    fn render_module_or_package(
        &self,
        w: &mut HtmlWriter,
        name: &str,
        target_scope: &DocScope,
        member_docs: &MemberDocs,
    ) {
        let is_package = matches!(target_scope, DocScope::Package { .. });
        let is_site = matches!(self.page_scope, DocScope::Site(_));
        let mut div_class = String::from("member with-page-link");
        if member_docs.is_expandable {
            div_class.push_str(" with-expandable-docs");
        }
        w.block("li", &[], |w| {
            self.render_anchor(w, name, "anchor");
            w.block("div", &[("class", div_class.as_str())], |w| {
                member_docs.render_expand_icon(w);
                self.render_self_link(w, name);

                w.block("div", &[("class", "member-left")], |w| {
                    let mod_cls = if member_docs.is_deprecated_member {
                        "member-modifiers member-deprecated"
                    } else {
                        "member-modifiers"
                    };
                    w.block("div", &[("class", mod_cls)], |w| {
                        self.render_modifiers(
                            w,
                            &[],
                            &[if is_package { "package" } else { "module" }],
                        );
                    });
                });

                w.block("div", &[("class", "member-main")], |w| {
                    let sig_cls = if member_docs.is_deprecated_member {
                        "member-signature member-deprecated"
                    } else {
                        "member-signature"
                    };
                    w.block("div", &[("class", sig_cls)], |w| {
                        let raw_link =
                            format!("./{}", target_scope.url_relative_to(self.page_scope));
                        let href =
                            if is_site && let DocScope::Package { package, .. } = target_scope {
                                raw_link.replacen(package.version(), "current", 1)
                            } else {
                                raw_link
                            };
                        w.inline(
                            "a",
                            &[("class", "name-decl"), ("href", href.as_str())],
                            |w| {
                                if matches!(target_scope, DocScope::Module { .. }) {
                                    w.text(&as_module_name(name));
                                } else {
                                    w.text(name);
                                }
                            },
                        );
                    });
                    member_docs.render_doc_comment(w);
                });
            });
        });
    }

    fn render_properties(&self, w: &mut HtmlWriter, schema: &ModuleSchema, clazz: &PClass) {
        if !class_has_listed_property(schema, clazz) {
            return;
        }
        let super_has_listed = clazz
            .superclass
            .is_some_and(|sid| class_has_listed_property(schema, schema.store.class(sid)));

        w.block("div", &[("class", "member-group")], |w| {
            self.render_anchor(w, "_properties", "anchor");
            w.block("h2", &[("class", "member-group-title")], |w| {
                w.text("Properties");
                if super_has_listed {
                    self.render_show_inherited_button(w);
                }
            });

            w.block("ul", &[], |w| {
                for (prop_name, prop) in schema.store.all_properties(clazz.id) {
                    if annotations_is_unlisted(&prop.annotations) {
                        continue;
                    }
                    let is_inherited = prop.owner_info != clazz.info;
                    let prop_scope = DocScope::Property {
                        site: Arc::clone(self.site),
                        property: prop.clone(),
                        parent: Box::new(self.page_scope.clone()),
                    };
                    let inherited_doc = schema
                        .store
                        .inherited_property_doc_comment(prop.owner, &prop.simple_name);
                    let member_docs = MemberDocs::new(
                        self.site,
                        self.page_scope,
                        inherited_doc,
                        &prop_scope,
                        &prop.annotations,
                        false,
                        Vec::new(),
                    );
                    let mut div_class = if is_inherited {
                        String::from("member inherited expandable hidden collapsed")
                    } else if prop.is_hidden() {
                        String::from("member hidden-member")
                    } else {
                        String::from("member")
                    };
                    if member_docs.is_expandable {
                        div_class.push_str(" with-expandable-docs");
                    }

                    w.block("li", &[], |w| {
                        self.render_anchor(w, &prop_name, "anchor");
                        w.block("div", &[("class", div_class.as_str())], |w| {
                            member_docs.render_expand_icon(w);
                            self.render_self_link(w, &prop_name);

                            w.block("div", &[("class", "member-left")], |w| {
                                let mod_cls = if member_docs.is_deprecated_member {
                                    "member-modifiers member-deprecated"
                                } else {
                                    "member-modifiers"
                                };
                                w.block("div", &[("class", mod_cls)], |w| {
                                    self.render_modifiers(w, &prop.modifiers, &[]);
                                });
                            });

                            w.block("div", &[("class", "member-main")], |w| {
                                let sig_cls = if member_docs.is_deprecated_member {
                                    "member-signature member-deprecated"
                                } else {
                                    "member-signature"
                                };
                                w.block("div", &[("class", sig_cls)], |w| {
                                    if is_inherited {
                                        self.render_class_context(
                                            w,
                                            schema.store.class(prop.owner),
                                        );
                                    }
                                    w.inline("span", &[("class", "name-decl")], |w| {
                                        w.text(&maybe_quote_identifier(&prop_name));
                                    });
                                    w.text(": ");
                                    self.render_type(
                                        w,
                                        &schema.store,
                                        &prop.ty,
                                        &prop_scope,
                                        false,
                                    );
                                    if is_inherited {
                                        self.render_module_context(w, prop.module_name());
                                    }
                                    self.render_member_source_link(
                                        w,
                                        prop.module_name(),
                                        prop.source_location,
                                    );
                                });
                                member_docs.render_doc_comment(w);
                            });
                        });
                    });
                }
            });
        });
    }

    fn render_methods(&self, w: &mut HtmlWriter, schema: &ModuleSchema, clazz: &PClass) {
        if !class_has_listed_method(schema, clazz) {
            return;
        }
        let super_has_listed = clazz
            .superclass
            .is_some_and(|sid| class_has_listed_method(schema, schema.store.class(sid)));

        w.block("div", &[("class", "member-group")], |w| {
            self.render_anchor(w, "_methods", "anchor");
            w.block("h2", &[("class", "member-group-title")], |w| {
                w.text("Methods");
                if super_has_listed {
                    self.render_show_inherited_button(w);
                }
            });

            w.block("ul", &[], |w| {
                for (method_name, method) in schema.store.all_methods(clazz.id) {
                    if annotations_is_unlisted(&method.annotations) {
                        continue;
                    }
                    let is_inherited = method.owner_info != clazz.info;
                    let method_scope = DocScope::Method {
                        site: Arc::clone(self.site),
                        method: method.clone(),
                        parent: Box::new(self.page_scope.clone()),
                    };
                    let inherited_doc = schema
                        .store
                        .inherited_method_doc_comment(method.owner, &method.simple_name);
                    let member_docs = MemberDocs::new(
                        self.site,
                        self.page_scope,
                        inherited_doc,
                        &method_scope,
                        &method.annotations,
                        false,
                        Vec::new(),
                    );
                    let mut div_class = if is_inherited {
                        String::from("member inherited expandable hidden collapsed")
                    } else {
                        String::from("member")
                    };
                    if member_docs.is_expandable {
                        div_class.push_str(" with-expandable-docs");
                    }

                    w.block("li", &[], |w| {
                        self.render_method_anchors(w, method);
                        w.block("div", &[("class", div_class.as_str())], |w| {
                            member_docs.render_expand_icon(w);
                            self.render_self_link(w, &format!("{method_name}()"));

                            w.block("div", &[("class", "member-left")], |w| {
                                let mod_cls = if member_docs.is_deprecated_member {
                                    "member-modifiers member-deprecated"
                                } else {
                                    "member-modifiers"
                                };
                                w.block("div", &[("class", mod_cls)], |w| {
                                    self.render_modifiers(w, &method.modifiers, &["function"]);
                                });
                            });

                            w.block("div", &[("class", "member-main")], |w| {
                                let sig_cls = if member_docs.is_deprecated_member {
                                    "member-signature member-deprecated"
                                } else {
                                    "member-signature"
                                };
                                w.block("div", &[("class", sig_cls)], |w| {
                                    if is_inherited {
                                        self.render_class_context(
                                            w,
                                            schema.store.class(method.owner),
                                        );
                                    }
                                    w.inline("span", &[("class", "name-decl")], |w| {
                                        w.text(&maybe_quote_identifier(&method.simple_name));
                                    });
                                    self.render_type_parameters(w, &method.type_parameters);
                                    self.render_method_parameters(
                                        w,
                                        &schema.store,
                                        method,
                                        &method_scope,
                                    );
                                    w.text(": ");
                                    self.render_type(
                                        w,
                                        &schema.store,
                                        &method.return_type,
                                        &method_scope,
                                        false,
                                    );
                                    if is_inherited {
                                        self.render_module_context(w, method.module_name());
                                    }
                                    self.render_member_source_link(
                                        w,
                                        method.module_name(),
                                        method.source_location,
                                    );
                                });
                                member_docs.render_doc_comment(w);
                            });
                        });
                    });
                }
            });
        });
    }

    fn collect_member_info_for_package(&self, doc_package: &DocPackage) -> Vec<ExtraInfoItem> {
        let mut out = Vec::new();
        let info = &doc_package.doc_package_info;

        if let Some(ref min_ver) = doc_package.min_pkl_version {
            out.push(ExtraInfoItem::plain(
                "Pkl version",
                ExtraInfoBody::Text(format!("{min_ver} or higher")),
            ));
        }

        if let Some(ref uri) = info.uri {
            out.push(ExtraInfoItem::plain(
                "URI",
                ExtraInfoBody::UriCopy(uri.clone()),
            ));
        }

        if let Some(ref authors) = info.authors
            && !authors.is_empty()
        {
            out.push(ExtraInfoItem::plain(
                "Authors",
                ExtraInfoBody::Text(authors.join(", ")),
            ));
        }

        out.push(ExtraInfoItem::plain(
            "Version",
            ExtraInfoBody::Text(info.version.clone()),
        ));

        if let Some(ref src) = info.source_code {
            out.push(ExtraInfoItem::plain(
                "Source code",
                ExtraInfoBody::Link {
                    href: src.clone(),
                    text: src.clone(),
                },
            ));
        }

        if let Some(ref issues) = info.issue_tracker {
            out.push(ExtraInfoItem::plain(
                "Issue tracker",
                ExtraInfoBody::Link {
                    href: issues.clone(),
                    text: issues.clone(),
                },
            ));
        }

        let deps: Vec<&PackageDependency> = info
            .dependencies
            .iter()
            .filter(|d| d.name != "pkl")
            .collect();
        if !deps.is_empty() {
            let rendered_deps = deps
                .into_iter()
                .map(|dep| {
                    let link = self.get_dependency_link(dep);
                    (dep.name.clone(), dep.version.clone(), link)
                })
                .collect();
            out.push(ExtraInfoItem::plain(
                "Dependencies",
                ExtraInfoBody::Dependencies(rendered_deps),
            ));
        }

        for (k, v) in &info.extra_attributes {
            out.push(ExtraInfoItem::plain(
                k.clone(),
                ExtraInfoBody::Text(v.clone()),
            ));
        }

        out.push(ExtraInfoItem::runtime("Known usages", KNOWN_USAGES));
        out.push(ExtraInfoItem::runtime("All versions", KNOWN_VERSIONS));
        out
    }

    fn get_dependency_link(&self, dep: &PackageDependency) -> Option<String> {
        let qname = dep.qualified_name();
        if let Some(pscope) = self.site.package_scopes.iter().find(|p| p.name() == qname) {
            return Some(pscope.url_for_version_relative_to(
                self.site,
                self.page_scope,
                &dep.version,
            ));
        }
        // Fallback: check if directory already exists on disk (`PageGenerator.kt:601-617`).
        let pkg_path = self.site.output_dir.join(encode_path(&qname));
        if pkg_path.exists() {
            let target_uri = resolve_uri(
                &self.site.url,
                &format!("./{}/{}/index.html", encode_path(&qname), dep.version),
            );
            return Some(relativize_uri(&target_uri, &self.page_scope.url()));
        }
        None
    }

    fn collect_member_info_for_module(&self, doc_module: &DocModule) -> Vec<ExtraInfoItem> {
        let mut out = Vec::new();
        out.push(ExtraInfoItem::plain(
            "Module URI",
            ExtraInfoBody::UriCopy(doc_module.import_uri.clone()),
        ));

        if let Some(ann) = doc_module
            .schema
            .annotations
            .iter()
            .find(|a| is_base_annotation(a, "ModuleInfo"))
            && let Some(DecodedValue::String(min_ver)) = ann.get_property("minPklVersion")
        {
            out.push(ExtraInfoItem::plain(
                "Pkl version",
                ExtraInfoBody::Text(format!("{min_ver} or higher")),
            ));
        }

        if let Some(ref source_url) = doc_module.source_url {
            let path = ParsedUri::parse(source_url).decoded_path();
            let name = match path.rfind('/') {
                Some(idx) => &path[idx + 1..],
                None => &path,
            };
            out.push(ExtraInfoItem::plain(
                "Source code",
                ExtraInfoBody::Link {
                    href: source_url.clone(),
                    text: name.to_string(),
                },
            ));
        }

        if !doc_module.examples.is_empty()
            && doc_module.parent_info.source_code_url_scheme.is_some()
        {
            let exs = doc_module
                .examples
                .iter()
                .filter_map(|ex| {
                    let href = doc_module
                        .parent_info
                        .get_module_source_code(&ex.module_name)?;
                    Some((href, ex.short_module_name().to_string()))
                })
                .collect();
            out.push(ExtraInfoItem::plain(
                "Examples",
                ExtraInfoBody::Examples(exs),
            ));
        }

        out.push(ExtraInfoItem::runtime(
            "Known subtypes in package",
            KNOWN_SUBTYPES,
        ));
        out.push(ExtraInfoItem::runtime(
            "Known usages in package",
            KNOWN_USAGES,
        ));
        out.push(ExtraInfoItem::runtime("All versions", KNOWN_VERSIONS));
        out
    }
}

// ---------------------------------------------------------------------------
// 1. MainPageGenerator (`MainPageGenerator.kt`)
// ---------------------------------------------------------------------------

fn render_main_page(
    docsite_info: &DocsiteInfo,
    packages_data: &[PackageData],
    site: &Arc<SiteScope>,
    is_single_package_site: bool,
) -> String {
    let page_title = docsite_info.title.as_deref().unwrap_or("Pkldoc");
    let mut w = HtmlWriter::new_document();

    if is_single_package_site {
        let single_pkg = &packages_data[0];
        let pkg_url = format!("{}/current/index.html", single_pkg.r#ref.base_path());
        let mut pkg_url_json = String::new();
        push_json_string(&mut pkg_url_json, &pkg_url);
        let script = format!("window.location.replace({pkg_url_json});");
        w.block("html", &[("lang", "en-US")], |w| {
            w.block("head", &[], |w| {
                w.block_void("meta", &[("charset", "UTF-8")]);
                w.block("title", &[], |w| w.text(page_title));
                w.block("script", &[], |w| w.raw(&script));
            });
            w.block("body", &[], |w| {
                w.block("main", &[], |w| {
                    w.block("p", &[], |w| {
                        w.inline("a", &[("href", pkg_url.as_str())], |w| w.text(&pkg_url));
                    });
                });
            });
        });
        return w.finish();
    }

    let page_scope = site.as_doc_scope();
    let ctx = PageCtx {
        docsite_info,
        site,
        page_scope: &page_scope,
        package_version: None,
        is_single_package_site: false,
        is_test_mode: false,
    };

    w.block("html", &[("lang", "en-US")], |w| {
        ctx.render_html_head(w, page_title);
        w.block("body", &[("onload", "onLoad()")], |w| {
            ctx.render_page_header(w, None, None, None, None);
            w.block("main", &[], |w| {
                w.block("h1", &[("id", "declaration-title")], |w| {
                    w.text(docsite_info.title.as_deref().unwrap_or(""));
                });

                let member_docs = MemberDocs::new(
                    site,
                    &page_scope,
                    docsite_info.overview.as_deref(),
                    &page_scope,
                    &[],
                    true,
                    Vec::new(),
                );

                ctx.render_member_group_links(
                    w,
                    &[
                        ("Overview", "#_overview", member_docs.is_expandable),
                        ("Packages", "#_packages", !packages_data.is_empty()),
                    ],
                );

                if docsite_info.overview.is_some() {
                    ctx.render_anchor(w, "_overview", "anchor");
                    let mut div_class = String::from("member");
                    if member_docs.is_expandable {
                        div_class.push_str(" with-expandable-docs");
                    }
                    w.block(
                        "div",
                        &[("id", "_declaration"), ("class", div_class.as_str())],
                        |w| {
                            member_docs.render_expand_icon(w);
                            member_docs.render_doc_comment(w);
                        },
                    );
                }

                if !packages_data.is_empty() {
                    let mut sorted: Vec<&PackageData> = packages_data.iter().collect();
                    sorted.sort_by(|a, b| {
                        (a.r#ref.pkg != "pkl", &a.r#ref.pkg)
                            .cmp(&(b.r#ref.pkg != "pkl", &b.r#ref.pkg))
                    });
                    w.block("div", &[("class", "member-group")], |w| {
                        ctx.render_anchor(w, "_packages", "anchor");
                        w.block("h2", &[("class", "member-group-title")], |w| {
                            w.text("Packages");
                        });
                        w.block("ul", &[], |w| {
                            for pkg in sorted {
                                let empty_pkg = site.create_empty_package_scope(
                                    pkg.r#ref.pkg.clone(),
                                    pkg.r#ref.version.clone(),
                                    pkg.source_code_url_scheme.clone(),
                                    pkg.source_code.clone(),
                                );
                                let target_scope = DocScope::Package {
                                    site: Arc::clone(site),
                                    package: empty_pkg,
                                };
                                let anns: Vec<DecodedObject> = pkg
                                    .deprecation
                                    .as_deref()
                                    .map(create_deprecated_annotation)
                                    .into_iter()
                                    .collect();
                                let pkg_docs = MemberDocs::new(
                                    site,
                                    &page_scope,
                                    pkg.summary.as_deref(),
                                    &target_scope,
                                    &anns,
                                    false,
                                    Vec::new(),
                                );
                                ctx.render_module_or_package(
                                    w,
                                    &pkg.r#ref.pkg,
                                    &target_scope,
                                    &pkg_docs,
                                );
                            }
                        });
                    });
                }
            });
        });
    });

    w.finish()
}

// ---------------------------------------------------------------------------
// 2. PackagePageGenerator (`PackagePageGenerator.kt`)
// ---------------------------------------------------------------------------

fn render_package_page(
    docsite_info: &DocsiteInfo,
    site: &Arc<SiteScope>,
    doc_package: &DocPackage,
    pkg_scope: &Arc<PackageScope>,
    page_scope: &DocScope,
    is_single_package_site: bool,
) -> String {
    let page_title = format!(
        "{} ({}) • {}",
        pkg_scope.name(),
        pkg_scope.version(),
        docsite_info.title.as_deref().unwrap_or("Pkldoc")
    );
    let ctx = PageCtx {
        docsite_info,
        site,
        page_scope,
        package_version: Some(pkg_scope.version()),
        is_single_package_site,
        is_test_mode: false,
    };

    let mut w = HtmlWriter::new_document();
    w.block("html", &[("lang", "en-US")], |w| {
        ctx.render_html_head(w, &page_title);
        w.block("body", &[("onload", "onLoad()")], |w| {
            ctx.render_page_header(
                w,
                Some(doc_package.name()),
                Some(doc_package.version()),
                None,
                None,
            );
            w.block("main", &[], |w| {
                ctx.render_parent_links(w, None, None);

                w.block("h1", &[("id", "declaration-title")], |w| {
                    w.text(doc_package.name());
                    w.inline("span", &[("id", "declaration-version")], |w| {
                        w.text(doc_package.version());
                    });
                });

                let info = &doc_package.doc_package_info;
                let extra = ctx.collect_member_info_for_package(doc_package);
                let member_docs = MemberDocs::new(
                    site,
                    page_scope,
                    info.overview.as_deref(),
                    page_scope,
                    &info.annotations,
                    true,
                    extra,
                );

                ctx.render_member_group_links(
                    w,
                    &[
                        ("Overview", "#_overview", member_docs.is_expandable),
                        ("Modules", "#_modules", doc_package.has_listed_module),
                    ],
                );

                ctx.render_anchor(w, "_overview", "anchor");
                let mut decl_class = String::from("member");
                if member_docs.is_expandable {
                    decl_class.push_str(" with-expandable-docs");
                }
                w.block(
                    "div",
                    &[("id", "_declaration"), ("class", decl_class.as_str())],
                    |w| {
                        member_docs.render_expand_icon(w);
                        w.block("div", &[("class", "member-signature")], |w| {
                            ctx.render_modifiers(w, &[], &["package"]);
                            w.inline("span", &[("class", "name-decl")], |w| {
                                w.text(doc_package.name());
                            });
                        });
                        member_docs.render_doc_comment(w);
                    },
                );

                if doc_package.has_listed_module {
                    w.block("div", &[("class", "member-group")], |w| {
                        ctx.render_anchor(w, "_modules", "anchor");
                        w.block("h2", &[("class", "member-group-title")], |w| {
                            w.text("Modules");
                        });
                        w.block("ul", &[], |w| {
                            for doc_module in &doc_package.doc_modules {
                                if doc_module.is_unlisted {
                                    continue;
                                }
                                let module_scope = pkg_scope.get_module(doc_module.name());
                                let target_scope = DocScope::Module {
                                    site_pkg: Some(Arc::clone(pkg_scope)),
                                    module: module_scope,
                                };
                                let mod_extra = ctx.collect_member_info_for_module(doc_module);
                                let mod_docs = MemberDocs::new(
                                    site,
                                    page_scope,
                                    doc_module.schema.doc_comment.as_deref(),
                                    &target_scope,
                                    &doc_module.schema.annotations,
                                    false,
                                    mod_extra,
                                );
                                ctx.render_module_or_package(
                                    w,
                                    doc_module.name(),
                                    &target_scope,
                                    &mod_docs,
                                );
                            }
                        });
                    });
                }
            });
        });
    });

    w.finish()
}

// ---------------------------------------------------------------------------
// 3. ModulePageGenerator (`ModulePageGenerator.kt`)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_module_page(
    docsite_info: &DocsiteInfo,
    site: &Arc<SiteScope>,
    doc_package: &DocPackage,
    doc_module: &DocModule,
    pkg_scope: &Arc<PackageScope>,
    module_scope: &Arc<ModuleScope>,
    page_scope: &DocScope,
    is_test_mode: bool,
    is_single_package_site: bool,
) -> String {
    let schema = &doc_module.schema;
    let clazz = schema.module_class();
    let short_name = match schema.module_name.rfind('.') {
        Some(idx) => &schema.module_name[idx + 1..],
        None => &schema.module_name,
    };
    let dropped: String = schema
        .module_name
        .chars()
        .skip(pkg_scope.name().chars().count())
        .collect();
    let mid_path = match dropped.rfind('.') {
        Some(idx) => dropped[..idx].replace('.', "/"),
        None => dropped.replace('.', "/"),
    };
    let page_title = format!(
        "{short_name} ({}{mid_path}:{}) • {}",
        pkg_scope.name(),
        pkg_scope.version(),
        docsite_info.title.as_deref().unwrap_or("Pkldoc")
    );

    let ctx = PageCtx {
        docsite_info,
        site,
        page_scope,
        package_version: Some(pkg_scope.version()),
        is_single_package_site,
        is_test_mode,
    };

    let mut w = HtmlWriter::new_document();
    w.block("html", &[("lang", "en-US")], |w| {
        ctx.render_html_head(w, &page_title);
        w.block("body", &[("onload", "onLoad()")], |w| {
            ctx.render_page_header(
                w,
                Some(doc_package.name()),
                Some(doc_package.version()),
                Some(doc_module.name()),
                None,
            );
            w.block("main", &[], |w| {
                ctx.render_parent_links(w, Some(pkg_scope), None);

                let mod_display_name = as_module_name(doc_module.name());
                w.block("h1", &[("id", "declaration-title")], |w| {
                    w.text(&mod_display_name);
                    w.inline("span", &[("id", "declaration-version")], |w| {
                        w.text(doc_package.version());
                    });
                });

                let extra = ctx.collect_member_info_for_module(doc_module);
                let member_docs = MemberDocs::new(
                    site,
                    page_scope,
                    clazz.doc_comment.as_deref(),
                    page_scope,
                    &clazz.annotations,
                    true,
                    extra,
                );

                ctx.render_member_group_links(
                    w,
                    &[
                        ("Overview", "#_overview", member_docs.is_expandable),
                        (
                            "Properties",
                            "#_properties",
                            class_has_listed_property(schema, clazz),
                        ),
                        (
                            "Methods",
                            "#_methods",
                            class_has_listed_method(schema, clazz),
                        ),
                        ("Classes", "#_classes", schema_has_listed_class(schema)),
                        (
                            "Type Aliases",
                            "#_type-aliases",
                            schema_has_listed_type_alias(schema),
                        ),
                    ],
                );

                ctx.render_anchor(w, "_overview", "anchor");
                let mut decl_class = String::from("member");
                if member_docs.is_expandable {
                    decl_class.push_str(" with-expandable-docs");
                }
                w.block(
                    "div",
                    &[("id", "_declaration"), ("class", decl_class.as_str())],
                    |w| {
                        member_docs.render_expand_icon(w);
                        w.block("div", &[("class", "member-signature")], |w| {
                            ctx.render_modifiers(w, &clazz.modifiers, &["module"]);
                            w.inline("span", &[("class", "name-decl")], |w| {
                                w.text(&mod_display_name);
                            });
                            ctx.render_module_amends_or_extends_clause(w, schema);
                        });
                        member_docs.render_doc_comment(w);
                    },
                );

                ctx.render_properties(w, schema, clazz);
                ctx.render_methods(w, schema, clazz);
                render_module_classes(&ctx, w, schema, pkg_scope, module_scope);
                render_module_type_aliases(&ctx, w, schema, pkg_scope, module_scope);
            });
        });
    });

    w.finish()
}

fn render_module_classes(
    ctx: &PageCtx<'_>,
    w: &mut HtmlWriter,
    schema: &ModuleSchema,
    pkg_scope: &Arc<PackageScope>,
    module_scope: &Arc<ModuleScope>,
) {
    if !schema_has_listed_class(schema) {
        return;
    }
    let super_has_listed = schema
        .supermodule
        .as_deref()
        .is_some_and(schema_has_listed_class);

    w.block("div", &[("class", "member-group")], |w| {
        ctx.render_anchor(w, "_classes", "anchor");
        w.block("h2", &[("class", "member-group-title")], |w| {
            w.text("Classes");
            if super_has_listed {
                ctx.render_show_inherited_button(w);
            }
        });

        w.block("ul", &[], |w| {
            for (class_name, cid) in schema.all_classes() {
                let clazz = schema.store.class(cid);
                if annotations_is_unlisted(&clazz.annotations) {
                    continue;
                }
                let is_inherited = !schema.classes.iter().any(|(k, _)| k == &class_name);
                let class_scope = make_class_scope(ctx.site, pkg_scope, module_scope, clazz);
                let member_docs = MemberDocs::new(
                    ctx.site,
                    ctx.page_scope,
                    clazz.doc_comment.as_deref(),
                    &class_scope,
                    &clazz.annotations,
                    false,
                    Vec::new(),
                );
                let mut div_class = if is_inherited {
                    String::from("member with-page-link inherited expandable hidden collapsed")
                } else {
                    String::from("member with-page-link")
                };
                if member_docs.is_expandable {
                    div_class.push_str(" with-expandable-docs");
                }

                w.block("li", &[], |w| {
                    ctx.render_anchor(w, &class_name, "anchor");
                    w.block("div", &[("class", div_class.as_str())], |w| {
                        member_docs.render_expand_icon(w);
                        ctx.render_self_link(w, &class_name);

                        w.block("div", &[("class", "member-left")], |w| {
                            let mod_cls = if member_docs.is_deprecated_member {
                                "member-modifiers member-deprecated"
                            } else {
                                "member-modifiers"
                            };
                            w.block("div", &[("class", mod_cls)], |w| {
                                ctx.render_modifiers(w, &clazz.modifiers, &["class"]);
                            });
                        });

                        w.block("div", &[("class", "member-main")], |w| {
                            let sig_cls = if member_docs.is_deprecated_member {
                                "member-signature member-deprecated"
                            } else {
                                "member-signature"
                            };
                            w.block("div", &[("class", sig_cls)], |w| {
                                ctx.render_class_name(w, clazz, "name-decl");
                                ctx.render_type_parameters(w, &clazz.type_parameters);
                                ctx.render_class_extends_clause(
                                    w,
                                    &schema.store,
                                    clazz,
                                    &class_scope,
                                );
                                if is_inherited {
                                    ctx.render_module_context(w, clazz.module_name());
                                }
                                ctx.render_member_source_link(
                                    w,
                                    clazz.module_name(),
                                    clazz.source_location,
                                );
                            });
                            member_docs.render_doc_comment(w);
                        });
                    });
                });
            }
        });
    });
}

fn render_module_type_aliases(
    ctx: &PageCtx<'_>,
    w: &mut HtmlWriter,
    schema: &ModuleSchema,
    pkg_scope: &Arc<PackageScope>,
    module_scope: &Arc<ModuleScope>,
) {
    if !schema_has_listed_type_alias(schema) {
        return;
    }
    let super_has_listed = schema
        .supermodule
        .as_deref()
        .is_some_and(schema_has_listed_type_alias);

    w.block("div", &[("class", "member-group")], |w| {
        ctx.render_anchor(w, "_type-aliases", "anchor");
        w.block("h2", &[("class", "member-group-title")], |w| {
            w.text("Type Aliases");
            if super_has_listed {
                ctx.render_show_inherited_button(w);
            }
        });

        w.block("ul", &[], |w| {
            for (alias_name, aid) in schema.all_type_aliases() {
                let alias = schema.store.type_alias(aid);
                if annotations_is_unlisted(&alias.annotations) {
                    continue;
                }
                let is_inherited = !schema.type_aliases.iter().any(|(k, _)| k == &alias_name);
                let alias_scope = make_type_alias_scope(ctx.site, pkg_scope, module_scope, alias);
                let member_docs = MemberDocs::new(
                    ctx.site,
                    ctx.page_scope,
                    alias.doc_comment.as_deref(),
                    &alias_scope,
                    &alias.annotations,
                    false,
                    Vec::new(),
                );
                let mut div_class = if is_inherited {
                    String::from("member inherited expandable hidden collapsed")
                } else {
                    String::from("member")
                };
                if member_docs.is_expandable {
                    div_class.push_str(" with-expandable-docs");
                }

                w.block("li", &[], |w| {
                    ctx.render_type_alias_anchors(w, alias);
                    w.block("div", &[("class", div_class.as_str())], |w| {
                        member_docs.render_expand_icon(w);
                        ctx.render_self_link(w, &alias_name);

                        w.block("div", &[("class", "member-left")], |w| {
                            let mod_cls = if member_docs.is_deprecated_member {
                                "member-modifiers member-deprecated"
                            } else {
                                "member-modifiers"
                            };
                            w.block("div", &[("class", mod_cls)], |w| {
                                ctx.render_modifiers(w, &alias.modifiers, &["typealias"]);
                            });
                        });

                        w.block("div", &[("class", "member-main")], |w| {
                            let sig_cls = if member_docs.is_deprecated_member {
                                "member-signature member-deprecated"
                            } else {
                                "member-signature"
                            };
                            w.block("div", &[("class", sig_cls)], |w| {
                                ctx.render_type_alias_name(w, alias, "name-decl");
                                ctx.render_type_parameters(w, &alias.type_parameters);
                                w.text(" = ");
                                ctx.render_type(
                                    w,
                                    &schema.store,
                                    &alias.aliased_type,
                                    &alias_scope,
                                    false,
                                );
                                if is_inherited {
                                    ctx.render_module_context(w, alias.module_name());
                                }
                                ctx.render_member_source_link(
                                    w,
                                    alias.module_name(),
                                    alias.source_location,
                                );
                            });
                            member_docs.render_doc_comment(w);
                        });
                    });
                });
            }
        });
    });
}

// ---------------------------------------------------------------------------
// 4. ClassPageGenerator (`ClassPageGenerator.kt`)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_class_page(
    docsite_info: &DocsiteInfo,
    site: &Arc<SiteScope>,
    doc_package: &DocPackage,
    doc_module: &DocModule,
    clazz: &PClass,
    pkg_scope: &Arc<PackageScope>,
    module_scope: &Arc<ModuleScope>,
    page_scope: &DocScope,
    is_test_mode: bool,
    is_single_package_site: bool,
) -> String {
    let schema = &doc_module.schema;
    let dropped: String = module_scope
        .module
        .module_name
        .chars()
        .skip(pkg_scope.name().chars().count())
        .collect::<String>()
        .replace('.', "/");
    let page_title = format!(
        "{} ({}{dropped}:{}) • {}",
        clazz.simple_name(),
        pkg_scope.name(),
        pkg_scope.version(),
        docsite_info.title.as_deref().unwrap_or("Pkldoc")
    );

    let ctx = PageCtx {
        docsite_info,
        site,
        page_scope,
        package_version: Some(pkg_scope.version()),
        is_single_package_site,
        is_test_mode,
    };

    let mut w = HtmlWriter::new_document();
    w.block("html", &[("lang", "en-US")], |w| {
        ctx.render_html_head(w, &page_title);
        w.block("body", &[("onload", "onLoad()")], |w| {
            ctx.render_page_header(
                w,
                Some(doc_package.name()),
                Some(doc_package.version()),
                Some(clazz.module_name()),
                Some(clazz.simple_name()),
            );
            w.block("main", &[], |w| {
                ctx.render_parent_links(w, Some(pkg_scope), Some(module_scope));
                ctx.render_class_anchors(w, clazz);

                let ident = maybe_quote_identifier(clazz.simple_name());
                w.block("h1", &[("id", "declaration-title")], |w| {
                    w.text(&ident);
                    w.inline("span", &[("id", "declaration-version")], |w| {
                        w.text(doc_package.version());
                    });
                });

                let extra = vec![
                    ExtraInfoItem::runtime("Known subtypes in package", KNOWN_SUBTYPES),
                    ExtraInfoItem::runtime("Known usages in package", KNOWN_USAGES),
                    ExtraInfoItem::runtime("All versions", KNOWN_VERSIONS),
                ];
                let member_docs = MemberDocs::new(
                    site,
                    page_scope,
                    clazz.doc_comment.as_deref(),
                    page_scope,
                    &clazz.annotations,
                    true,
                    extra,
                );

                ctx.render_member_group_links(
                    w,
                    &[
                        ("Overview", "#_overview", member_docs.is_expandable),
                        (
                            "Properties",
                            "#_properties",
                            class_has_listed_property(schema, clazz),
                        ),
                        (
                            "Methods",
                            "#_methods",
                            class_has_listed_method(schema, clazz),
                        ),
                    ],
                );

                ctx.render_anchor(w, "_overview", "anchor");
                let mut decl_class = String::from("member");
                if member_docs.is_expandable {
                    decl_class.push_str(" with-expandable-docs");
                }
                w.block(
                    "div",
                    &[("id", "_declaration"), ("class", decl_class.as_str())],
                    |w| {
                        member_docs.render_expand_icon(w);
                        let sig_cls = if member_docs.is_deprecated_member {
                            "member-signature member-deprecated"
                        } else {
                            "member-signature"
                        };
                        w.block("div", &[("class", sig_cls)], |w| {
                            ctx.render_modifiers(w, &clazz.modifiers, &["class"]);
                            w.inline("span", &[("class", "name-decl")], |w| {
                                w.text(&ident);
                            });
                            ctx.render_type_parameters(w, &clazz.type_parameters);
                            ctx.render_class_extends_clause(w, &schema.store, clazz, page_scope);
                        });
                        member_docs.render_doc_comment(w);
                    },
                );

                ctx.render_properties(w, schema, clazz);
                ctx.render_methods(w, schema, clazz);
            });
        });
    });

    w.finish()
}

fn class_simple_display_name(clazz: &PClass) -> &str {
    if clazz.is_module_class() {
        match clazz.module_name().rfind('.') {
            Some(idx) => &clazz.module_name()[idx + 1..],
            None => clazz.module_name(),
        }
    } else {
        clazz.simple_name()
    }
}
