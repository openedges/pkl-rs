//! Rust differential probe matching `tools/java/M5ApiOracleProbe.java` across all 10 scenarios.

use pkl::schema::Modifier;
use pkl::{
    AnalyzerBuilder, EvaluatorBuilder, HttpClient, Logger, ModuleKey, ModuleKeyFactories,
    ModuleSource, PathElement, PklError, PklErrorKind, Project, ResolvedModuleKey, Resource,
    ResourceReader, ResourceValue, SecurityManager, SecurityManagers, StackFrame,
    StackFrameTransformers, TraceMode,
};
use pkl_host_native::{file_uri_to_path, path_to_file_uri};
use pkl_runtime::value::PklString;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn emit_pkl_error(rendered: &str) {
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(rendered.as_bytes());
    if !rendered.ends_with('\n') {
        let _ = err.write_all(b"\n");
    }
    let _ = err.flush();
}

fn abs_normalize(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut out = PathBuf::new();
    for comp in abs.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn base_deterministic_builder() -> EvaluatorBuilder {
    EvaluatorBuilder::preconfigured()
        .with_module_cache_dir(None)
        .with_external_properties(Vec::new())
        .with_environment_variables(Vec::new())
}

fn run_eval_text(args: &[String]) -> Result<(), PklError> {
    if args.len() < 2 {
        return Err(PklError::invalid_argument("eval-text requires <file>"));
    }
    let file = abs_normalize(Path::new(&args[1]));
    let mut format: Option<String> = None;
    let mut color = false;
    let mut trace_mode = TraceMode::Compact;
    let mut props: Vec<(String, String)> = Vec::new();
    let mut envs: Vec<(String, String)> = Vec::new();
    for a in &args[2..] {
        if let Some(v) = a.strip_prefix("--format=") {
            format = Some(v.to_string());
        } else if let Some(kv) = a.strip_prefix("--prop=") {
            let Some((k, v)) = kv.split_once('=') else {
                return Err(PklError::invalid_argument(format!("invalid --prop: {a}")));
            };
            props.push((k.to_string(), v.to_string()));
        } else if let Some(kv) = a.strip_prefix("--env=") {
            let Some((k, v)) = kv.split_once('=') else {
                return Err(PklError::invalid_argument(format!("invalid --env: {a}")));
            };
            envs.push((k.to_string(), v.to_string()));
        } else if a == "--color=always" {
            color = true;
        } else if a == "--color=never" {
            color = false;
        } else if a == "--trace-mode=pretty" {
            trace_mode = TraceMode::Pretty;
        } else if a == "--trace-mode=compact" {
            trace_mode = TraceMode::Compact;
        } else {
            return Err(PklError::invalid_argument(format!("unknown arg: {a}")));
        }
    }
    let mut builder = base_deterministic_builder()
        .with_color(color)
        .with_trace_mode(trace_mode)
        .with_external_properties(props)
        .with_environment_variables(envs);
    if let Some(fmt) = format {
        builder = builder.with_output_format(fmt);
    }
    let mut evaluator = builder.build()?;
    let out = evaluator.evaluate_output_text(&ModuleSource::path(&file))?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(out.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn run_eval_source_text(args: &[String]) -> Result<(), PklError> {
    if args.len() < 2 {
        return Err(PklError::invalid_argument(
            "eval-source-text requires <text-file>",
        ));
    }
    let text = std::fs::read_to_string(&args[1])?;
    let mut expr: Option<String> = None;
    for a in &args[2..] {
        if let Some(v) = a.strip_prefix("--expr=") {
            expr = Some(v.to_string());
        } else {
            return Err(PklError::invalid_argument(format!("unknown arg: {a}")));
        }
    }
    let mut evaluator = base_deterministic_builder().build()?;
    let src = ModuleSource::text(text);
    let mut stdout = std::io::stdout().lock();
    match expr {
        None => {
            let out = evaluator.evaluate_output_text(&src)?;
            stdout.write_all(out.as_bytes())?;
        }
        Some(e) => {
            let out = evaluator.evaluate_expression_string(&src, &e)?;
            stdout.write_all(out.as_bytes())?;
            stdout.write_all(b"\n")?;
        }
    }
    stdout.flush()?;
    Ok(())
}

fn run_eval_output_bytes(args: &[String]) -> Result<(), PklError> {
    if args.len() < 2 {
        return Err(PklError::invalid_argument(
            "eval-output-bytes requires <file>",
        ));
    }
    let file = abs_normalize(Path::new(&args[1]));
    let mut evaluator = base_deterministic_builder().build()?;
    let bytes = evaluator.evaluate_output_bytes(&ModuleSource::path(&file))?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&bytes)?;
    stdout.flush()?;
    Ok(())
}

fn run_eval_output_files(args: &[String]) -> Result<(), PklError> {
    if args.len() < 3 {
        return Err(PklError::invalid_argument(
            "eval-output-files requires <file> <out-dir>",
        ));
    }
    let file = abs_normalize(Path::new(&args[1]));
    let out_dir = abs_normalize(Path::new(&args[2]));
    std::fs::create_dir_all(&out_dir)?;
    let mut evaluator = base_deterministic_builder().build()?;
    let files = evaluator.evaluate_output_files(&ModuleSource::path(&file))?;
    let mut sb = String::new();
    for (rel, file_output) in &files {
        let dest = abs_normalize(&out_dir.join(rel));
        if !dest.starts_with(&out_dir) {
            return Err(PklError::invalid_argument(format!(
                "output path escapes out-dir: {rel}"
            )));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, file_output.bytes())?;
        sb.push_str(rel);
        sb.push('\n');
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(sb.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn run_eval_expr_pkl_binary(args: &[String]) -> Result<(), PklError> {
    if args.len() < 3 {
        return Err(PklError::invalid_argument(
            "eval-expr-pkl-binary requires <file> <expr>",
        ));
    }
    let file = abs_normalize(Path::new(&args[1]));
    let expr = &args[2];
    let mut evaluator = base_deterministic_builder().build()?;
    let bytes = evaluator.evaluate_expression_pkl_binary(&ModuleSource::path(&file), expr)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&bytes)?;
    stdout.flush()?;
    Ok(())
}

fn uri_path(uri: &str) -> &str {
    let after_scheme = uri.split_once(':').map_or(uri, |(_, rest)| rest);
    let no_query = after_scheme
        .split(['?', '#'])
        .next()
        .unwrap_or(after_scheme);
    if let Some(rest) = no_query.strip_prefix("//") {
        rest.find('/').map_or("", |i| &rest[i..])
    } else {
        no_query
    }
}

struct MemModuleKey {
    uri: String,
}

impl MemModuleKey {
    fn lookup(path: &str) -> Option<&'static str> {
        match path {
            "/a.pkl" => Some(
                "greeting = \"hello from memmod\"\nvalue = trace(42)\n@Deprecated { message = \"use value\" }\nfunction oldFn() = 7\n",
            ),
            "/dir/b.pkl" => Some("nested = true\n"),
            "/boom.pkl" => Some("fail = throw(\"memmod-boom\")\n"),
            _ => None,
        }
    }
}

impl ModuleKey for MemModuleKey {
    fn uri(&self) -> &str {
        &self.uri
    }

    fn has_hierarchical_uris(&self) -> bool {
        true
    }

    fn is_globbable(&self) -> bool {
        true
    }

    fn is_local(&self) -> bool {
        false
    }

    fn is_cached(&self) -> bool {
        false
    }

    fn resolve(
        &self,
        security_manager: &dyn SecurityManager,
    ) -> Result<ResolvedModuleKey, PklError> {
        security_manager.check_resolve_module(&self.uri)?;
        let path = uri_path(&self.uri);
        let Some(content) = Self::lookup(path) else {
            return Err(PklError::io(format!("memmod not found: {}", self.uri)));
        };
        Ok(ResolvedModuleKey::virtual_module(self.uri.clone(), content))
    }

    fn has_element(
        &self,
        security_manager: &dyn SecurityManager,
        element_uri: &str,
    ) -> Result<bool, PklError> {
        security_manager.check_resolve_module(element_uri)?;
        let p = uri_path(element_uri);
        Ok(Self::lookup(p).is_some() || p == "/" || p == "/dir")
    }

    fn list_elements(
        &self,
        security_manager: &dyn SecurityManager,
        base_uri: &str,
    ) -> Result<Vec<PathElement>, PklError> {
        security_manager.check_resolve_module(base_uri)?;
        let p = uri_path(base_uri);
        if p == "/" || p.is_empty() {
            Ok(vec![
                PathElement::new("a.pkl", false),
                PathElement::new("boom.pkl", false),
                PathElement::new("dir", true),
            ])
        } else if p == "/dir" || p == "/dir/" {
            Ok(vec![PathElement::new("b.pkl", false)])
        } else {
            Ok(Vec::new())
        }
    }
}

struct MemResourceReader;

impl MemResourceReader {
    fn lookup(path: &str) -> Option<&'static str> {
        match path {
            "/secret.txt" => Some("top-secret-42"),
            "/items/one.txt" => Some("1"),
            "/items/two.txt" => Some("2"),
            _ => None,
        }
    }
}

impl ResourceReader for MemResourceReader {
    fn uri_scheme(&self) -> &str {
        "memres"
    }

    fn has_hierarchical_uris(&self) -> bool {
        true
    }

    fn is_globbable(&self) -> bool {
        true
    }

    fn read(&self, uri: &str) -> Result<Option<ResourceValue>, PklError> {
        let p = uri_path(uri);
        Ok(Self::lookup(p)
            .map(|content| ResourceValue::Resource(Resource::from_text(uri, content))))
    }

    fn has_element(
        &self,
        security_manager: &dyn SecurityManager,
        element_uri: &str,
    ) -> Result<bool, PklError> {
        security_manager.check_resolve_resource(element_uri)?;
        let p = uri_path(element_uri);
        Ok(Self::lookup(p).is_some() || p == "/" || p == "/items")
    }

    fn list_elements(
        &self,
        security_manager: &dyn SecurityManager,
        base_uri: &str,
    ) -> Result<Vec<PathElement>, PklError> {
        security_manager.check_resolve_resource(base_uri)?;
        let p = uri_path(base_uri);
        if p == "/items" || p == "/items/" {
            Ok(vec![
                PathElement::new("one.txt", false),
                PathElement::new("two.txt", false),
            ])
        } else if p == "/" || p.is_empty() {
            Ok(vec![
                PathElement::new("items", true),
                PathElement::new("secret.txt", false),
            ])
        } else {
            Ok(Vec::new())
        }
    }
}

struct RecordingLogger {
    buf: Arc<Mutex<String>>,
}

impl Logger for RecordingLogger {
    fn trace(&self, message: &str, frame: &StackFrame) {
        let mut g = self.buf.lock().unwrap_or_else(|e| e.into_inner());
        g.push_str(&format!(
            "TRACE|{}|{}|{}\n",
            frame.module_uri(),
            frame.start_line(),
            message
        ));
    }

    fn warn(&self, message: &str, frame: &StackFrame) {
        let mut g = self.buf.lock().unwrap_or_else(|e| e.into_inner());
        g.push_str(&format!(
            "WARN|{}|{}|{}\n",
            frame.module_uri(),
            frame.start_line(),
            message
        ));
    }
}

fn run_eval_custom_spi(args: &[String]) -> Result<(), PklError> {
    if args.len() < 2 {
        return Err(PklError::invalid_argument(
            "eval-custom-spi requires <file>",
        ));
    }
    let file = abs_normalize(Path::new(&args[1]));
    let mut allowed_modules = SecurityManagers::default_allowed_modules();
    allowed_modules.push("^memmod:".to_string());
    let mut allowed_resources = SecurityManagers::default_allowed_resources();
    allowed_resources.push("^memres:".to_string());

    let logs = Arc::new(Mutex::new(String::new()));
    let logger = Arc::new(RecordingLogger {
        buf: Arc::clone(&logs),
    });

    let transformer = StackFrameTransformers::chain(
        StackFrameTransformers::default_transformer(),
        StackFrameTransformers::from_fn(|frame| {
            if frame.module_uri().starts_with("memmod:") {
                frame.with_module_uri(format!("{}?transformed=1", frame.module_uri()))
            } else {
                frame
            }
        }),
    );

    let mem_factory = ModuleKeyFactories::from_fn(|uri| {
        uri.strip_prefix("memmod:").map(|_| {
            Box::new(MemModuleKey {
                uri: uri.to_string(),
            }) as Box<dyn ModuleKey>
        })
    });

    let mut evaluator = base_deterministic_builder()
        .with_allowed_modules(allowed_modules)
        .with_allowed_resources(allowed_resources)
        .with_logger(logger)
        .with_stack_frame_transformer(transformer)
        .add_module_key_factory(mem_factory)
        .add_resource_reader(Arc::new(MemResourceReader))
        .build()?;

    let out = evaluator.evaluate_output_text(&ModuleSource::path(&file))?;
    let log_str = logs.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let combined = format!("---OUTPUT---\n{out}---LOGS---\n{log_str}");
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(combined.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

struct StrictCustomSecurity;

impl SecurityManager for StrictCustomSecurity {
    fn check_resolve_module(&self, uri: &str) -> Result<(), PklError> {
        let scheme = uri.split_once(':').map_or("", |(s, _)| s);
        if scheme != "pkl" && scheme != "file" {
            return Err(PklError::security(format!(
                "Custom security denied module: {uri}"
            )));
        }
        Ok(())
    }

    fn check_import_module(
        &self,
        _importing_module: &str,
        imported_module: &str,
    ) -> Result<(), PklError> {
        self.check_resolve_module(imported_module)
    }

    fn check_read_resource(&self, resource_uri: &str) -> Result<(), PklError> {
        if resource_uri == "prop:pkl.outputFormat" {
            return Ok(());
        }
        Err(PklError::security(format!(
            "Custom security denied resource: {resource_uri}"
        )))
    }
}

fn run_eval_custom_security(args: &[String]) -> Result<(), PklError> {
    if args.len() < 2 {
        return Err(PklError::invalid_argument(
            "eval-custom-security requires <file>",
        ));
    }
    let file = abs_normalize(Path::new(&args[1]));
    let mut evaluator = EvaluatorBuilder::unconfigured()
        .with_stack_frame_transformer(StackFrameTransformers::default_transformer())
        .with_security_manager(Arc::new(StrictCustomSecurity))
        .with_module_cache_dir(None)
        .build()?;
    let out = evaluator.evaluate_output_text(&ModuleSource::path(&file))?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(out.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn run_eval_schema(args: &[String]) -> Result<(), PklError> {
    if args.len() < 2 {
        return Err(PklError::invalid_argument("eval-schema requires <file>"));
    }
    let file = abs_normalize(Path::new(&args[1]));
    let mut evaluator = base_deterministic_builder().build()?;
    let schema = evaluator.evaluate_schema(&ModuleSource::path(&file))?;

    let mut sb = String::new();
    sb.push_str(&format!("module={}\n", schema.module_name));
    sb.push_str(&format!("isAmend={}\n", schema.is_amend));
    sb.push_str(&format!("isExtend={}\n", schema.is_extend()));
    sb.push_str(&format!(
        "supermodule={}\n",
        schema
            .supermodule
            .as_deref()
            .map_or("none", |s| s.module_name.as_str())
    ));
    sb.push_str(&format!(
        "docComment={}\n",
        schema
            .doc_comment
            .as_deref()
            .map(|d| d.replace('\n', "\\n"))
            .unwrap_or_else(|| "none".to_string())
    ));

    let mut mod_props: Vec<String> = schema
        .module_class()
        .properties
        .iter()
        .map(|(k, _)| k.clone())
        .collect();
    mod_props.sort();
    sb.push_str(&format!("properties={}\n", mod_props.join(",")));

    let mut mod_methods: Vec<String> = schema
        .module_class()
        .methods
        .iter()
        .map(|(k, _)| k.clone())
        .collect();
    mod_methods.sort();
    sb.push_str(&format!("methods={}\n", mod_methods.join(",")));

    let mut class_names: Vec<String> = schema.classes.iter().map(|(k, _)| k.clone()).collect();
    class_names.sort();
    for cname in &class_names {
        let cls = schema.class_by_name(cname).expect("class exists");
        let mut cprops: Vec<String> = cls.properties.iter().map(|(k, _)| k.clone()).collect();
        cprops.sort();
        let mut cmeths: Vec<String> = cls.methods.iter().map(|(k, _)| k.clone()).collect();
        cmeths.sort();
        let is_abstract = cls.modifiers.contains(&Modifier::Abstract);
        let is_open = cls.modifiers.contains(&Modifier::Open);
        sb.push_str(&format!(
            "class={cname}|abstract={is_abstract}|open={is_open}|props={}|methods={}\n",
            cprops.join(","),
            cmeths.join(",")
        ));
    }

    let mut aliases: Vec<String> = schema.type_aliases.iter().map(|(k, _)| k.clone()).collect();
    aliases.sort();
    sb.push_str(&format!("typeAliases={}\n", aliases.join(",")));

    let mut imports: Vec<String> = schema.imports.iter().map(|(k, _)| k.clone()).collect();
    imports.sort();
    sb.push_str(&format!("imports={}\n", imports.join(",")));

    let mut stdout = std::io::stdout().lock();
    stdout.write_all(sb.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn normalize_graph_uri(uri: &str, project_dir: &Path) -> String {
    if uri.starts_with("file:")
        && let Ok(p) = file_uri_to_path(&PklString::from(uri))
    {
        let norm = abs_normalize(&p);
        let proj_canon =
            std::fs::canonicalize(project_dir).unwrap_or_else(|_| project_dir.to_path_buf());
        let p_canon = std::fs::canonicalize(&norm).unwrap_or(norm);
        if let Ok(rel) = p_canon.strip_prefix(&proj_canon) {
            return format!("project:/{}", rel.to_string_lossy().replace('\\', "/"));
        }
    }
    uri.to_string()
}

fn run_project_and_analyzer(args: &[String]) -> Result<(), PklError> {
    if args.len() < 3 {
        return Err(PklError::invalid_argument(
            "project-and-analyzer requires <project-dir> <entry-file>",
        ));
    }
    let project_dir = abs_normalize(Path::new(&args[1]));
    let entry_file = abs_normalize(Path::new(&args[2]));
    let project = Project::load_from_path(project_dir.join("PklProject"))?;

    let mut evaluator = base_deterministic_builder()
        .apply_from_project(&project)?
        .with_module_cache_dir(None)
        .build()?;
    let eval_out = evaluator.evaluate_output_text(&ModuleSource::path(&entry_file))?;

    let http_client = HttpClient::builder().build()?;
    let analyzer = AnalyzerBuilder::preconfigured()
        .with_stack_frame_transformer(StackFrameTransformers::default_transformer())
        .with_project(&project)
        .with_module_cache_dir(None)
        .with_http_client(http_client)
        .with_trace_mode(TraceMode::Compact)
        .build()?;
    let entry_uri = path_to_file_uri(&entry_file).to_java_utf8();
    let graph = analyzer.import_graph([entry_uri])?;

    let mut norm_imports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (from_uri, targets) in &graph.imports {
        let from = normalize_graph_uri(from_uri, &project_dir);
        let entry = norm_imports.entry(from).or_default();
        for imp in targets {
            entry.insert(normalize_graph_uri(&imp.uri, &project_dir));
        }
    }

    let mut sb = String::new();
    sb.push_str("---PROJECT-EVAL---\n");
    sb.push_str(&eval_out);
    sb.push_str("---IMPORT-GRAPH---\n");
    for (from, targets) in &norm_imports {
        let joined: Vec<&str> = targets.iter().map(String::as_str).collect();
        sb.push_str(&format!("{from} -> {}\n", joined.join(",")));
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(sb.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn run_lifecycle(args: &[String]) -> Result<(), PklError> {
    if args.len() < 3 {
        return Err(PklError::invalid_argument(
            "lifecycle-reuse-timeout-close requires <fast-file> <slow-file>",
        ));
    }
    let fast_file = abs_normalize(Path::new(&args[1]));
    let slow_file = abs_normalize(Path::new(&args[2]));
    let mut sb = String::new();

    // 1. Module reuse & idempotent close + post-close rejection
    let mut eval1 = base_deterministic_builder().build()?;
    let first = eval1.evaluate_output_text(&ModuleSource::path(&fast_file))?;
    let second = eval1.evaluate_output_text(&ModuleSource::path(&fast_file))?;
    sb.push_str(&format!("reuse_equal={}\n", first == second));
    sb.push_str(&format!("reuse_output={}\n", first.replace('\n', "\\n")));
    eval1.close();
    eval1.close();
    let closed_rejected = eval1
        .evaluate_output_text(&ModuleSource::path(&fast_file))
        .is_err();
    sb.push_str(&format!("closed_rejected={closed_rejected}\n"));

    // 2. Timeout enforcement (1s)
    let mut timeout_triggered = false;
    let mut timeout_header = String::new();
    {
        let mut eval2 = base_deterministic_builder()
            .with_timeout(Duration::from_secs(1))
            .build()?;
        if let Err(err) = eval2.evaluate_output_text(&ModuleSource::path(&slow_file)) {
            let msg = err.message();
            timeout_triggered = msg.contains("timed out");
            let mut lines = msg.splitn(3, '\n');
            let l1 = lines.next().unwrap_or("");
            timeout_header = match lines.next() {
                Some(l2) => format!("{l1}\n{l2}"),
                None => l1.to_string(),
            };
        }
    }
    sb.push_str(&format!("timeout_triggered={timeout_triggered}\n"));
    sb.push_str(&format!(
        "timeout_header={}\n",
        timeout_header.replace('\n', "\\n")
    ));

    let mut stdout = std::io::stdout().lock();
    stdout.write_all(sb.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: pkl_m5_api_probe <scenario> [args...]");
        return ExitCode::from(2);
    }
    let res = match args[0].as_str() {
        "eval-text" => run_eval_text(&args),
        "eval-source-text" => run_eval_source_text(&args),
        "eval-output-bytes" => run_eval_output_bytes(&args),
        "eval-output-files" => run_eval_output_files(&args),
        "eval-expr-pkl-binary" => run_eval_expr_pkl_binary(&args),
        "eval-custom-spi" => run_eval_custom_spi(&args),
        "eval-custom-security" => run_eval_custom_security(&args),
        "eval-schema" => run_eval_schema(&args),
        "project-and-analyzer" => run_project_and_analyzer(&args),
        "lifecycle-reuse-timeout-close" => run_lifecycle(&args),
        other => {
            eprintln!("Unknown scenario: {other}");
            return ExitCode::from(2);
        }
    };
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => match err.kind() {
            PklErrorKind::Evaluation
            | PklErrorKind::Security
            | PklErrorKind::Timeout
            | PklErrorKind::Closed
            | PklErrorKind::Cancelled => {
                emit_pkl_error(err.message());
                ExitCode::from(1)
            }
            _ => {
                eprintln!("{:?}: {}", err.kind(), err.message());
                ExitCode::from(2)
            }
        },
    }
}
