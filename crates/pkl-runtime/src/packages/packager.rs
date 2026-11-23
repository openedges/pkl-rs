//! Pure `ProjectPackager` (`pkl project package`) core: exclude glob filtering,
//! AST-based `ImportsAndReadsValidator`, recursive local-dep packaging, publish check,
//! and deterministic 4-artifact generation (`.zip`, `.zip.sha256`, `<leaf>`, `<leaf>.sha256`).
//!
//! Matches `org.pkl.core.project.ProjectPackager` (`ProjectPackager.java:82-501`),
//! `ImportsAndReadsParser.java:55-161`, and `CliProjectPackager.kt:28-92`.
use crate::diagnostic::Diagnostic;
use crate::java_regex::{Limits, Utf16Regex};
use crate::modules::ModuleHostError;
use crate::packages::PackageUri;
use crate::packages::http::{PackageHttpError, PackageTransport, get_metadata_request_uri};
use crate::packages::metadata::{DependencyMetadata, RemoteDependency, sha256_hex};
use crate::packages::mvs::relativize_paths;
use crate::packages::project::{
    PKL_PROJECT_DEPS_FILENAME, PackageSpec, Project, ProjectDepEntry, ProjectDeps,
};
use crate::packages::zip::build_deterministic_package_zip;
use crate::source::{SourceText, Span};
use crate::syntax::*;
use crate::value::PklString;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// Parsed import/read reference collected by `parse_imports_and_reads` (`ImportsAndReadsParser.java:57-63`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportOrReadEntry {
    pub is_module: bool,
    pub is_glob: bool,
    pub is_extends: bool,
    pub is_amends: bool,
    pub string_value: PklString,
    pub span: Span,
}

/// Pure AST parser collecting `extends`/`amends`, `import`/`import*` declarations & expressions,
/// and single-line non-interpolated string-literal `read`/`read?`/`read*` expressions
/// (`ImportsAndReadsParser.java:55-161`).
pub fn parse_imports_and_reads(source: &SourceText) -> Result<Vec<ImportOrReadEntry>, Diagnostic> {
    let module = crate::parser::parse_module_for_evaluation(source.entire())
        .map_err(|failure| failure.diagnostic)?;
    let mut entries = Vec::new();
    enum Work<'a> {
        Expr(&'a Expr),
        Member(&'a Member),
        Type(&'a Type),
        Object(&'a ObjectMember),
        Parameter(&'a Parameter),
    }
    let mut work = Vec::new();
    if let Some(declaration) = &module.declaration {
        work.extend(declaration.annotations.iter().map(Work::Expr));
        if let Some(parent) = &declaration.parent {
            let is_extends = matches!(parent.kind, ModuleParentKind::Extends);
            let is_amends = matches!(parent.kind, ModuleParentKind::Amends);
            entries.push(ImportOrReadEntry {
                is_module: true,
                is_glob: false,
                is_extends,
                is_amends,
                string_value: parent.uri.clone(),
                span: parent.span,
            });
        }
    }
    work.extend(module.members.iter().map(Work::Member));

    while let Some(next) = work.pop() {
        match next {
            Work::Member(member) => {
                work.extend(member.annotations.iter().map(Work::Expr));
                work.extend(member.ty.iter().map(Work::Type));
                work.push(Work::Expr(&member.expr));
            }
            Work::Parameter(parameter) => work.extend(parameter.ty.iter().map(Work::Type)),
            Work::Type(ty) => match &ty.kind {
                TypeKind::Named { arguments, .. } => {
                    work.extend(arguments.iter().map(Work::Type));
                }
                TypeKind::Nullable(inner) => work.push(Work::Type(inner)),
                TypeKind::Union { types, .. } => work.extend(types.iter().map(Work::Type)),
                TypeKind::Function {
                    parameters,
                    return_type,
                } => {
                    work.extend(parameters.iter().map(Work::Type));
                    work.push(Work::Type(return_type));
                }
                TypeKind::Constrained { base, constraints } => {
                    work.push(Work::Type(base));
                    work.extend(constraints.iter().map(Work::Expr));
                }
                _ => {}
            },
            Work::Object(member) => match member {
                ObjectMember::Property(member) => work.push(Work::Member(member)),
                ObjectMember::Element { value, .. } | ObjectMember::Spread { value, .. } => {
                    work.push(Work::Expr(value));
                }
                ObjectMember::Entry { key, value, .. } => {
                    work.extend([Work::Expr(key), Work::Expr(value)]);
                }
                ObjectMember::Predicate {
                    condition, value, ..
                } => work.extend([Work::Expr(condition), Work::Expr(value)]),
                ObjectMember::When {
                    then_parameters,
                    else_parameters,
                    condition,
                    then_members,
                    else_members,
                    ..
                } => {
                    work.extend(
                        then_parameters
                            .iter()
                            .chain(else_parameters)
                            .map(Work::Parameter),
                    );
                    work.push(Work::Expr(condition));
                    work.extend(then_members.iter().chain(else_members).map(Work::Object));
                }
                ObjectMember::For {
                    parameters,
                    body_parameters,
                    iterable,
                    members,
                    ..
                } => {
                    work.extend(
                        parameters
                            .iter()
                            .chain(body_parameters)
                            .map(Work::Parameter),
                    );
                    work.push(Work::Expr(iterable));
                    work.extend(members.iter().map(Work::Object));
                }
            },
            Work::Expr(expr) => {
                match &expr.kind {
                    ExprKind::Import {
                        uri,
                        uri_span,
                        glob,
                        ..
                    } => {
                        entries.push(ImportOrReadEntry {
                            is_module: true,
                            is_glob: *glob,
                            is_extends: false,
                            is_amends: false,
                            string_value: uri.clone(),
                            span: *uri_span,
                        });
                    }
                    ExprKind::ReadResource(inner)
                    | ExprKind::ReadResourceOrNull(inner)
                    | ExprKind::ReadResourceGlob(inner) => {
                        let is_glob = matches!(expr.kind, ExprKind::ReadResourceGlob(_));
                        if let ExprKind::String(ref literal) = inner.kind {
                            // Single-line string literals only (`ImportsAndReadsParser.java:125`).
                            let raw_slice = source.entire().range(inner.span.start..inner.span.end);
                            let raw_text = raw_slice.to_pkl_string();
                            let is_multiline =
                                raw_text.as_str().is_some_and(|s| s.contains("\"\"\""));
                            if !is_multiline {
                                entries.push(ImportOrReadEntry {
                                    is_module: false,
                                    is_glob,
                                    is_extends: false,
                                    is_amends: false,
                                    string_value: literal.clone(),
                                    span: inner.span,
                                });
                            }
                        }
                        continue;
                    }
                    ExprKind::TypeCheck { ty, .. }
                    | ExprKind::TypeCast { ty, .. }
                    | ExprKind::TypeAlias { ty, .. } => work.push(Work::Type(ty)),
                    ExprKind::Let { ty, .. } => work.extend(ty.iter().map(Work::Type)),
                    ExprKind::Function {
                        parameters,
                        return_type,
                        ..
                    } => {
                        work.extend(parameters.iter().map(Work::Parameter));
                        work.extend(return_type.iter().map(Work::Type));
                    }
                    ExprKind::Class {
                        superclass,
                        members,
                        ..
                    } => {
                        work.extend(superclass.iter().map(Work::Type));
                        work.extend(members.iter().map(Work::Member));
                        continue;
                    }
                    ExprKind::Object {
                        parameters,
                        parent,
                        members,
                    } => {
                        work.extend(parameters.iter().map(Work::Parameter));
                        work.extend(parent.iter().map(|p| Work::Expr(p)));
                        work.extend(members.iter().map(Work::Object));
                        continue;
                    }
                    ExprKind::New {
                        parameters,
                        ty,
                        members,
                    }
                    | ExprKind::Annotation {
                        parameters,
                        ty,
                        members,
                    } => {
                        work.extend(parameters.iter().map(Work::Parameter));
                        work.push(Work::Type(ty));
                        work.extend(members.iter().map(Work::Object));
                        continue;
                    }
                    _ => {}
                }
                work.extend(expr.expression_children().into_iter().map(Work::Expr));
            }
        }
    }

    entries.sort_by_key(|entry| entry.span.start);
    Ok(entries)
}

/// Compiled glob matcher matching `GlobResolver.toRegexPattern` (`GlobResolver.java:88-240`).
pub struct ExcludeGlobMatcher {
    regex: Utf16Regex,
}

impl ExcludeGlobMatcher {
    pub fn compile(glob_pattern: &str) -> Result<Self, ModuleHostError> {
        let regex_str = glob_to_regex_string(glob_pattern)?;
        let units: Vec<u16> = regex_str.encode_utf16().collect();
        let regex =
            crate::regex::compile_utf16(&units, "").map_err(|err| ModuleHostError::EvalError {
                message: format!("Invalid glob pattern `{glob_pattern}`: {}", err.message).into(),
                hint: None,
            })?;
        Ok(Self { regex })
    }

    pub fn matches(&self, text: &str) -> bool {
        let units: Vec<u16> = text.encode_utf16().collect();
        self.regex
            .captures_entire(&units, Limits::default())
            .ok()
            .flatten()
            .is_some()
    }
}

fn glob_to_regex_string(pattern: &str) -> Result<String, ModuleHostError> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::from("^");
    let mut in_group = false;
    let mut i = 0;

    let invalid = |msg: &str| ModuleHostError::EvalError {
        message: format!("Invalid glob pattern `{pattern}`: {msg}").into(),
        hint: None,
    };

    while i < chars.len() {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();
        match ch {
            '{' => {
                if in_group {
                    return Err(invalid(
                        "Sub-patterns cannot be nested. To fix, remove or escape the inner `{` character.",
                    ));
                }
                in_group = true;
                out.push_str("(?:(?:");
            }
            '}' => {
                if in_group {
                    in_group = false;
                    out.push_str("))");
                } else {
                    out.push('}');
                }
            }
            ',' => {
                if in_group {
                    out.push_str(")|(?:");
                } else {
                    out.push(',');
                }
            }
            '\\' => {
                let Some(n) = next else {
                    return Err(invalid(
                        "The backslash (`\\`) is the escape character, and cannot terminate a glob pattern.",
                    ));
                };
                if !matches!(n, '?' | '*' | '[' | '{' | '\\') {
                    return Err(invalid(&format!("Invalid escape character `\\{n}`.")));
                }
                out.push('\\');
                out.push(n);
                i += 1;
            }
            '[' => {
                out.push_str("[[^/]&&[");
                match next {
                    Some('^') => {
                        out.push_str("\\^");
                        i += 1;
                    }
                    Some('!') => {
                        out.push('^');
                        i += 1;
                    }
                    Some(']') => {
                        out.push(']');
                        i += 1;
                    }
                    None => {
                        return Err(invalid("Missing character class terminator `]`."));
                    }
                    Some(_) => {}
                }
                i += 1;
                if i >= chars.len() {
                    return Err(invalid("Missing character class terminator `]`."));
                }
                while chars[i] != ']' {
                    let c = chars[i];
                    if c == '[' && matches!(chars.get(i + 1), Some(':' | '=' | '.')) {
                        return Err(invalid("Character class feature is not supported."));
                    }
                    if c == '/' {
                        return Err(invalid("Invalid character `/` inside character class."));
                    } else if c == '\\' {
                        out.push_str("\\\\");
                    } else {
                        out.push(c);
                    }
                    i += 1;
                    if i >= chars.len() {
                        return Err(invalid("Missing character class terminator `]`."));
                    }
                }
                out.push_str("]]");
            }
            '?' => {
                if next == Some('(') {
                    return Err(invalid("Extended globbing features are not supported."));
                }
                out.push('.');
            }
            '*' => {
                if next == Some('(') {
                    return Err(invalid("Extended globbing features are not supported."));
                } else if next == Some('*') {
                    out.push_str(".*");
                    i += 1;
                } else {
                    out.push_str("[^/]*");
                }
            }
            '+' | '@' => {
                if next == Some('(') {
                    return Err(invalid("Extended globbing features are not supported."));
                }
                out.push_str("\\+");
            }
            '!' => {
                if next == Some('(') {
                    return Err(invalid("Extended globbing features are not supported."));
                }
                out.push('!');
            }
            '.' | '(' | '%' | '^' | '$' | '|' => {
                out.push('\\');
                out.push(ch);
            }
            other => out.push(other),
        }
        i += 1;
    }

    if in_group {
        return Err(invalid("invalidGlobUnclosedSubpattern"));
    }
    out.push('$');
    Ok(out)
}

/// Checks whether a project-relative file path is excluded by any `package.exclude` glob pattern
/// (`ProjectPackager.java:359-372`: matches EITHER leaf filename OR normalized relative path).
pub fn is_file_excluded(rel_path: &str, matchers: &[ExcludeGlobMatcher]) -> bool {
    let normalized = rel_path.replace('\\', "/");
    let leaf = normalized.rsplit('/').next().unwrap_or(&normalized);
    matchers
        .iter()
        .any(|m| m.matches(leaf) || m.matches(&normalized))
}

fn is_absolute_import(import_str: &str) -> bool {
    if import_str.starts_with('@') {
        return true;
    }
    if let Some(colon) = import_str.find(':') {
        let mut chars = import_str[..colon].chars();
        chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    } else {
        false
    }
}

fn sanitize_path_segment(segment: &str) -> String {
    segment
        .chars()
        .map(|c| {
            if matches!(c, '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// Validates every import/read in a `.pkl` source file inside a package so no relative path
/// escapes `project_dir` even temporarily (`ProjectPackager.java:394-443`).
pub fn validate_pkl_imports_and_reads(
    project_dir: &Path,
    module_rel_path: &str,
    module_source: &str,
) -> Result<(), ModuleHostError> {
    let project_root = normalize_lexical(project_dir);
    let module_full_path = normalize_lexical(&project_root.join(module_rel_path));
    let module_parent = module_full_path
        .parent()
        .unwrap_or(&project_root)
        .to_path_buf();

    let source_text = SourceText::from(module_source);
    let entries = parse_imports_and_reads(&source_text).map_err(|diag| {
        let msg = diag.message.to_java_utf8();
        let hint = diag.hint().map(|h| h.to_java_utf8());
        render_packager_eval_error(
            &msg,
            hint.as_deref(),
            &module_full_path,
            module_source,
            diag.span,
        )
    })?;

    for entry in entries {
        let import_str = entry.string_value.as_str().unwrap_or("");
        if is_absolute_import(import_str) {
            continue;
        }
        // Check URI syntax via package_components / ResourceUri (escape spaces/glob chars first
        // matching `IoUtils.toUri` which percent-encodes glob characters like `*`, `?`, `[`, `]`, `{`, `}`).
        let encoded_for_uri = encode_uri_for_packager(import_str);
        let uri_pkl = PklString::from(encoded_for_uri.as_str());
        let components = crate::host::resources::package_components(&uri_pkl).map_err(|_| {
            render_packager_eval_error(
                &format!("Invalid module URI `{import_str}`."),
                None,
                &module_full_path,
                module_source,
                entry.span,
            )
        })?;

        if import_str.starts_with('/') && project_root != Path::new("/") {
            return Err(render_packager_eval_error(
                &format!(
                    "Path `{import_str}` includes path segments that are outside the project root directory."
                ),
                None,
                &module_full_path,
                module_source,
                entry.span,
            ));
        }

        let import_path = components.path.as_str().unwrap_or(import_str);
        let mut current = module_parent.clone();
        for segment in import_path.split('/') {
            if segment.is_empty() {
                continue;
            }
            let sanitized = sanitize_path_segment(segment);
            current.push(&sanitized);
            current = normalize_lexical(&current);
            if !current.starts_with(&project_root) {
                return Err(render_packager_eval_error(
                    &format!(
                        "Path `{import_str}` includes path segments that are outside the project root directory."
                    ),
                    None,
                    &module_full_path,
                    module_source,
                    entry.span,
                ));
            }
        }
    }

    Ok(())
}

fn render_packager_eval_error(
    message: &str,
    hint: Option<&str>,
    module_full_path: &Path,
    module_source: &str,
    span: Span,
) -> ModuleHostError {
    let file_uri = crate::packages::resolver::file_uri_from_path(module_full_path)
        .unwrap_or_else(|_| module_full_path.to_string_lossy().into_owned());
    let mut sources = crate::source::SourceMap::default();
    let id = sources.add(file_uri.as_str(), module_source);
    let source = sources.get(id).expect("inserted source");
    let line = source.position(span.start).map_or(1, |pos| pos.line);
    let display_uri = if file_uri.starts_with("file:") {
        format!("{file_uri}, line {line}")
    } else {
        file_uri
    };
    let member = PklString::from("<unknown>");
    let frame = crate::diagnostic::RenderFrame {
        source,
        uri: PklString::from(display_uri.as_str()),
        member: &member,
        span,
        highlight_length_utf16: None,
    };
    let rendered = crate::diagnostic::render_error(message, &[frame], hint);
    ModuleHostError::EvalError {
        message: rendered.trim_end_matches('\n').into(),
        hint: None,
    }
}

fn encode_uri_for_packager(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if matches!(
            byte,
            b' ' | b'[' | b']' | b'{' | b'}' | b'\\' | b'^' | b'`' | b'|' | b'<' | b'>' | b'"'
        ) {
            use std::fmt::Write;
            let _ = write!(&mut out, "%{byte:02X}");
        } else {
            out.push(char::from(byte));
        }
    }
    out
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(comp);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Validates that every direct local project dependency of every project in `projects` is also
/// included in `projects` (`CliProjectPackager.kt:69-78`).
pub fn validate_local_projects_included(projects: &[Project]) -> Result<(), ModuleHostError> {
    let included_dirs: Vec<PathBuf> = projects
        .iter()
        .map(|p| normalize_lexical(&p.project_dir))
        .collect();
    for proj in projects {
        let proj_dir = normalize_lexical(&proj.project_dir);
        for (_, local_dep) in &proj.declared_dependencies.local_dependencies {
            let local_raw = local_dep
                .project_file_uri
                .strip_prefix("file://")
                .or_else(|| local_dep.project_file_uri.strip_prefix("file:"))
                .unwrap_or(&local_dep.project_file_uri);
            let local_file = PathBuf::from(crate::packages::zip::percent_decode(local_raw));
            let dep_dir = normalize_lexical(local_file.parent().unwrap_or(&local_file));
            if !included_dirs.iter().any(|d| d == &dep_dir) {
                return Err(ModuleHostError::EvalError {
                    message: format!(
                        "Cannot package project `{}` because it depends on local project `{}`, which is not included for packaging.\n\nProvide both projects as arguments to the `pkl project package` command.\n\nExample: `pkl project package \"{}\" \"{}\"`",
                        proj_dir.display(),
                        dep_dir.display(),
                        proj_dir.display(),
                        dep_dir.display(),
                    )
                    .into(),
                    hint: None,
                });
            }
        }
    }
    Ok(())
}

/// Abstract filesystem / lockfile / publish-check provider for Wasm-pure `ProjectPackager`.
pub trait PackagerHost {
    /// Lists all regular files under `project_dir` as `(normalized_rel_path, raw_bytes)`.
    fn list_project_files(
        &self,
        project_dir: &Path,
    ) -> Result<Vec<(String, Vec<u8>)>, ModuleHostError>;

    /// Reads `PklProject.deps.json` text for `project_dir` if present.
    fn read_project_deps_json(&self, project_dir: &Path)
    -> Result<Option<String>, ModuleHostError>;

    /// Writes an artifact file (creating parent directories if needed).
    fn write_artifact(&self, path: &Path, bytes: &[u8]) -> Result<(), ModuleHostError>;

    /// Optional transport used for publish checks when `skip_publish_check == false`.
    fn transport(&self) -> Option<&dyn PackageTransport> {
        None
    }
}

/// Result of packaging a single project.
///
/// Matches the **exact Pkl 0.32.0 stdout emission order** (`ProjectPackager.java:136-139,178-179,455-460`,
/// verified by `CliProjectPackagerTest.kt:929-932`):
/// 1. `zip_file`
/// 2. `zip_checksum_file`
/// 3. `metadata_file`
/// 4. `metadata_checksum_file`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageArtifactResult {
    pub zip_file: PathBuf,
    pub zip_checksum_file: PathBuf,
    pub metadata_file: PathBuf,
    pub metadata_checksum_file: PathBuf,
    pub zip_sha256: String,
    pub metadata_sha256: String,
    pub metadata_json: String,
    pub zip_bytes: Vec<u8>,
}

impl PackageArtifactResult {
    /// Returns the 4 stdout output lines relative to `working_dir` in exact Pkl 0.32.0 order.
    pub fn stdout_lines(&self, working_dir: &Path) -> [String; 4] {
        [
            relativize_paths(&self.zip_file, working_dir),
            relativize_paths(&self.zip_checksum_file, working_dir),
            relativize_paths(&self.metadata_file, working_dir),
            relativize_paths(&self.metadata_checksum_file, working_dir),
        ]
    }
}

/// Pure `ProjectPackager` orchestrator (`ProjectPackager.java:82-501`).
pub struct ProjectPackager<'a, H: PackagerHost + ?Sized> {
    projects: &'a [Project],
    working_dir: PathBuf,
    output_path_pattern: String,
    skip_publish_check: bool,
    host: &'a H,
    results: BTreeMap<PackageUri, PackageArtifactResult>,
}

impl<'a, H: PackagerHost + ?Sized> ProjectPackager<'a, H> {
    pub fn new(
        projects: &'a [Project],
        working_dir: PathBuf,
        output_path_pattern: impl Into<String>,
        skip_publish_check: bool,
        host: &'a H,
    ) -> Self {
        Self {
            projects,
            working_dir,
            output_path_pattern: output_path_pattern.into(),
            skip_publish_check,
            host,
            results: BTreeMap::new(),
        }
    }

    /// Packages all configured projects and returns `(ordered_results, stdout_lines)`.
    pub fn create_packages(
        mut self,
    ) -> Result<(Vec<PackageArtifactResult>, Vec<String>), ModuleHostError> {
        // Validate every project has a `package` section first (`CliProjectPackager.kt:60-63`)
        for project in self.projects {
            if project.package.is_none() {
                return Err(ModuleHostError::EvalError {
                    message: format!(
                        "No package was declared in project `{}`.\n\nAdd a `package` section to the PklProject file.",
                        project.project_file_uri
                    )
                    .into(),
                    hint: None,
                });
            }
        }
        validate_local_projects_included(self.projects)?;

        let mut ordered = Vec::with_capacity(self.projects.len());
        let mut stdout_lines = Vec::with_capacity(self.projects.len() * 4);
        for project in self.projects {
            let res = self.do_package(project)?;
            stdout_lines.extend(res.stdout_lines(&self.working_dir));
            ordered.push(res);
        }
        Ok((ordered, stdout_lines))
    }

    fn resolve_output_dir(&self, pkg: &PackageSpec) -> PathBuf {
        let substituted = self
            .output_path_pattern
            .replace("%{name}", &pkg.name)
            .replace("%{version}", &pkg.version.to_string());
        self.working_dir.join(substituted)
    }

    fn do_package(&mut self, project: &Project) -> Result<PackageArtifactResult, ModuleHostError> {
        let Some(ref pkg) = project.package else {
            return Err(ModuleHostError::EvalError {
                message: format!(
                    "No package was declared in project `{}`.\n\nAdd a `package` section to the PklProject file.",
                    project.project_file_uri
                )
                .into(),
                hint: None,
            });
        };
        if let Some(existing) = self.results.get(&pkg.uri) {
            return Ok(existing.clone());
        }

        // 1. Compile exclude globs & collect/filter/sort regular project files
        let mut exclude_matchers = Vec::with_capacity(pkg.exclude.len());
        for pat in &pkg.exclude {
            exclude_matchers.push(ExcludeGlobMatcher::compile(pat)?);
        }
        let mut raw_files = self.host.list_project_files(&project.project_dir)?;
        raw_files.retain(|(rel, _)| !is_file_excluded(rel, &exclude_matchers));
        raw_files.sort_by(|(a, _), (b, _)| a.cmp(b));

        // 2. Validate imports and reads in all included `.pkl` files
        for (rel, bytes) in &raw_files {
            if rel.ends_with(".pkl") {
                let text = crate::resources::decode_utf8(bytes);
                validate_pkl_imports_and_reads(
                    &project.project_dir,
                    rel,
                    text.as_str().unwrap_or(""),
                )?;
            }
        }

        // 3. Build deterministic DEFLATE ZIP archive + compute SHA-256
        let zip_entries: Vec<(&str, &[u8])> = raw_files
            .iter()
            .map(|(rel, bytes)| (rel.as_str(), bytes.as_slice()))
            .collect();
        let zip_bytes = build_deterministic_package_zip(&zip_entries);
        let zip_sha256 = sha256_hex(&zip_bytes);

        // 4. Build metadata dependencies (recursively packaging local deps if not overridden)
        let dependencies = self.build_metadata_dependencies(project)?;

        // 5. Serialize DependencyMetadata JSON (no trailing newline) + compute SHA-256
        let metadata = DependencyMetadata {
            name: PklString::from(pkg.name.as_str()),
            package_uri: pkg.uri.clone(),
            version: pkg.version.clone(),
            package_zip_url: PklString::from(pkg.package_zip_url.as_str()),
            package_zip_sha256: PklString::from(zip_sha256.as_str()),
            dependencies,
            source_code_url_scheme: pkg.source_code_url_scheme.as_deref().map(PklString::from),
            source_code: pkg.source_code.as_deref().map(PklString::from),
            documentation: pkg.documentation.as_deref().map(PklString::from),
            license: pkg.license.as_deref().map(PklString::from),
            license_text: pkg.license_text.as_deref().map(PklString::from),
            authors: Some(
                pkg.authors
                    .iter()
                    .map(|a| PklString::from(a.as_str()))
                    .collect(),
            ),
            issue_tracker: pkg.issue_tracker.as_deref().map(PklString::from),
            description: pkg.description.as_deref().map(PklString::from),
            annotations: project.annotations.clone(),
        };
        let metadata_json = metadata
            .write_json()
            .map_err(|msg| ModuleHostError::EvalError {
                message: msg,
                hint: None,
            })?;
        let metadata_bytes = metadata_json.as_bytes();
        let metadata_sha256 = sha256_hex(metadata_bytes);

        // 6. Compute output paths & write all 4 artifacts (exact leaf filename)
        let output_dir = self.resolve_output_dir(pkg);
        let pkg_uri_str = pkg.uri.uri.as_str().unwrap_or("");
        let metadata_file_name = pkg_uri_str.rsplit('/').next().unwrap_or(&pkg.name);
        let metadata_file = output_dir.join(metadata_file_name);
        let metadata_checksum_file = output_dir.join(format!("{metadata_file_name}.sha256"));
        let zip_file = output_dir.join(format!("{metadata_file_name}.zip"));
        let zip_checksum_file = output_dir.join(format!("{metadata_file_name}.zip.sha256"));

        self.host.write_artifact(&zip_file, &zip_bytes)?;
        self.host.write_artifact(&metadata_file, metadata_bytes)?;
        self.host
            .write_artifact(&zip_checksum_file, zip_sha256.as_bytes())?;
        self.host
            .write_artifact(&metadata_checksum_file, metadata_sha256.as_bytes())?;

        // 7. Publish check unless `--skip-publish-check` (`ProjectPackager.java:174-214`)
        if !self.skip_publish_check {
            self.check_already_published(pkg, &metadata_sha256)?;
        }

        let result = PackageArtifactResult {
            zip_file,
            zip_checksum_file,
            metadata_file,
            metadata_checksum_file,
            zip_sha256,
            metadata_sha256,
            metadata_json,
            zip_bytes,
        };
        self.results.insert(pkg.uri.clone(), result.clone());
        Ok(result)
    }

    fn build_metadata_dependencies(
        &mut self,
        project: &Project,
    ) -> Result<Vec<RemoteDependency>, ModuleHostError> {
        let has_any = !project.declared_dependencies.remote_dependencies.is_empty()
            || !project.declared_dependencies.local_dependencies.is_empty();
        if !has_any {
            return Ok(Vec::new());
        }
        let deps_display = format!("{}{PKL_PROJECT_DEPS_FILENAME}", project.project_base_uri);
        let Some(deps_text) = self.host.read_project_deps_json(&project.project_dir)? else {
            return Err(ModuleHostError::EvalError {
                message: format!(
                    "Encountered an error when attempting to load `PklProject.deps.json` at `{deps_display}`.\n\nTry running `pkl project resolve` within the project directory to create a new set of dependencies."
                )
                .into(),
                hint: None,
            });
        };
        let deps = ProjectDeps::parse(&deps_text, &deps_display)?;
        let mut mgr = crate::packages::project::ProjectDependenciesManager::with_deps(
            project.declared_dependencies.clone(),
            project.project_dir.clone(),
            deps,
        )?;
        let resolved_map = mgr.get_dependencies()?.clone();
        // Force unused mut warning clean
        let _ = &mut mgr;

        // Upstream `Project.java:254,337` stores `remoteDependencies` and
        // `localProjectDependencies` in `new HashMap<>()`, and
        // `ProjectPackager.java:229-267` merges their `entrySet()` iterations
        // (remote first, then local) into `new HashMap<>(localSize + remoteSize)`
        // before `DependencyMetadata.java:528` serializes `entrySet()` directly.
        let remote_ordered = java_hashmap_order(
            project
                .declared_dependencies
                .remote_dependencies
                .iter()
                .map(|remote| (remote.name.clone(), remote))
                .collect(),
            None,
        );
        let local_ordered = java_hashmap_order(
            project
                .declared_dependencies
                .local_dependencies
                .iter()
                .map(|(name, _)| (name.clone(), name.as_str()))
                .collect(),
            None,
        );
        let total_declared = project.declared_dependencies.local_dependencies.len()
            + project.declared_dependencies.remote_dependencies.len();

        let mut merged = Vec::with_capacity(total_declared);
        for remote in remote_ordered {
            let Some(resolved) = resolved_map.get(&remote.name) else {
                continue;
            };
            let dep_entry = match resolved {
                ProjectDepEntry::Remote { uri, sha256 } => RemoteDependency {
                    name: PklString::from(remote.name.as_str()),
                    package_uri: uri.to_external_package_uri(),
                    sha256: sha256.clone(),
                },
                ProjectDepEntry::Local { uri, .. } => {
                    // Remote dep resolved to local sibling project: package it for checksum
                    let local_proj = self.find_local_project(project, uri).ok_or_else(|| {
                        ModuleHostError::EvalError {
                            message: format!(
                                "Missing local project for `{}` during packaging.",
                                uri.display_name()
                            )
                            .into(),
                            hint: None,
                        }
                    })?;
                    let packaged = self.do_package(&local_proj)?;
                    RemoteDependency {
                        name: PklString::from(remote.name.as_str()),
                        package_uri: uri.to_external_package_uri(),
                        sha256: PklString::from(packaged.metadata_sha256.as_str()),
                    }
                }
            };
            merged.push((remote.name.clone(), dep_entry));
        }
        for name in local_ordered {
            let Some(resolved) = resolved_map.get(name) else {
                continue;
            };
            let dep_entry = match resolved {
                ProjectDepEntry::Remote { uri, sha256 } => {
                    // Local dep overridden to a newer remote dependency in PklProject.deps.json
                    RemoteDependency {
                        name: PklString::from(name),
                        package_uri: uri.to_external_package_uri(),
                        sha256: sha256.clone(),
                    }
                }
                ProjectDepEntry::Local { uri, .. } => {
                    let local_proj = project
                        .local_project_dependencies
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, p)| p.clone())
                        .or_else(|| self.find_local_project(project, uri))
                        .ok_or_else(|| ModuleHostError::EvalError {
                            message: format!("Missing local project `{name}` during packaging.")
                                .into(),
                            hint: None,
                        })?;
                    let packaged = self.do_package(&local_proj)?;
                    RemoteDependency {
                        name: PklString::from(name),
                        package_uri: uri.to_external_package_uri(),
                        sha256: PklString::from(packaged.metadata_sha256.as_str()),
                    }
                }
            };
            merged.push((name.to_string(), dep_entry));
        }
        Ok(java_hashmap_order(merged, Some(total_declared)))
    }

    fn find_local_project(&self, parent: &Project, uri: &PackageUri) -> Option<Project> {
        let target = uri.to_external_package_uri();
        for p in self.projects {
            if p.package
                .as_ref()
                .is_some_and(|pkg| pkg.uri.to_external_package_uri() == target)
            {
                return Some(p.clone());
            }
        }
        for (_, lp) in &parent.local_project_dependencies {
            if lp
                .package
                .as_ref()
                .is_some_and(|pkg| pkg.uri.to_external_package_uri() == target)
            {
                return Some(lp.clone());
            }
        }
        None
    }

    fn check_already_published(
        &self,
        pkg: &PackageSpec,
        computed_checksum: &str,
    ) -> Result<(), ModuleHostError> {
        let Some(transport) = self.host.transport() else {
            return Err(ModuleHostError::EvalError {
                message: format!(
                    "Failed trying to fetch package `{}` from `{}` (no HTTP transport configured).",
                    pkg.name, pkg.package_zip_url
                )
                .into(),
                hint: None,
            });
        };
        let request_uri = get_metadata_request_uri(&pkg.uri);
        match transport.fetch_metadata_with_status(&pkg.uri, &request_uri) {
            Ok(bytes) => {
                let received_checksum = sha256_hex(&bytes);
                if received_checksum != computed_checksum {
                    return Err(ModuleHostError::EvalError {
                        message: format!(
                            "Package `{}` was already published with different contents.\n\nComputed checksum: {computed_checksum}\nPublished checksum: {received_checksum}\n\nThis likely means that you need to bump the package's version.",
                            pkg.uri.uri
                        )
                        .into(),
                        hint: None,
                    });
                }
                Ok(())
            }
            Err(PackageHttpError::BadStatusCode { status_code: 404, .. }) => Ok(()),
            Err(PackageHttpError::BadStatusCode { status_code, .. }) => {
                Err(ModuleHostError::EvalError {
                    message: format!(
                        "Failed trying to fetch package `{}` from `{}` (response code `{status_code}`).",
                        pkg.name, pkg.package_zip_url
                    )
                    .into(),
                    hint: None,
                })
            }
            Err(other) => Err(other.into_host_error()),
        }
    }
}

/// Simulates JDK 21 `java.util.HashMap` (`DEFAULT_LOAD_FACTOR = 0.75f`) bucket
/// iteration order for `String` keys.
///
/// Upstream `Project.java:254,337` stores `remoteDependencies` and
/// `localProjectDependencies` in `new HashMap<>()`, and
/// `ProjectPackager.java:229-267` merges them into
/// `new HashMap<>(localSize + remoteSize)` before `DependencyMetadata.java:528`
/// serializes `entrySet()` directly.
pub fn java_hashmap_order<T>(entries: Vec<(String, T)>, initial_capacity: Option<usize>) -> Vec<T> {
    const MAX_CAPACITY: usize = 1 << 30;
    const DEFAULT_INITIAL_CAPACITY: usize = 16;

    fn table_size_for(cap: usize) -> usize {
        if cap <= 1 {
            1
        } else if cap >= MAX_CAPACITY {
            MAX_CAPACITY
        } else {
            cap.next_power_of_two()
        }
    }

    fn spread_hash(key: &str) -> u32 {
        let mut h: i32 = 0;
        for unit in key.encode_utf16() {
            h = h.wrapping_mul(31).wrapping_add(i32::from(unit));
        }
        let bits = h as u32;
        bits ^ (bits >> 16)
    }

    fn threshold_for(cap: usize) -> usize {
        if cap >= MAX_CAPACITY {
            usize::MAX
        } else {
            // Exact match for JDK 21 `(int)((float) newCap * 0.75f)` for all powers of 2 <= 2^29.
            (cap * 3) / 4
        }
    }

    let mut cap: usize = 0;
    let mut threshold: usize = initial_capacity.map_or(0, table_size_for);
    let mut buckets: Vec<Vec<(u32, String, T)>> = Vec::new();
    let mut size: usize = 0;

    let resize = |cap: &mut usize,
                  threshold: &mut usize,
                  buckets: &mut Vec<Vec<(u32, String, T)>>| {
        let old_cap = *cap;
        let new_cap = if old_cap > 0 {
            old_cap << 1
        } else if *threshold > 0 {
            *threshold
        } else {
            DEFAULT_INITIAL_CAPACITY
        };
        *cap = new_cap;
        *threshold = threshold_for(new_cap);
        let old_buckets = std::mem::replace(buckets, (0..new_cap).map(|_| Vec::new()).collect());
        if old_cap > 0 {
            for (j, chain) in old_buckets.into_iter().enumerate() {
                for node in chain {
                    let dst = if (node.0 as usize & old_cap) == 0 {
                        j
                    } else {
                        j + old_cap
                    };
                    buckets[dst].push(node);
                }
            }
        }
    };

    for (key, value) in entries {
        if cap == 0 {
            resize(&mut cap, &mut threshold, &mut buckets);
        }
        let hash = spread_hash(&key);
        let idx = (hash as usize) & (cap - 1);
        if let Some(existing) = buckets[idx].iter_mut().find(|(_, k, _)| k == &key) {
            existing.2 = value;
        } else {
            buckets[idx].push((hash, key, value));
            size += 1;
            if size > threshold && cap < MAX_CAPACITY {
                resize(&mut cap, &mut threshold, &mut buckets);
            }
        }
    }

    let mut ordered = Vec::with_capacity(size);
    for chain in buckets {
        for (_, _, value) in chain {
            ordered.push(value);
        }
    }
    ordered
}
