//! `pkl run` subcommand (`RunCommand.kt` + `CliCommandRunner.kt`) for executing `pkl:Command`
//! CLI tools with Clikt-compatible help/usage wrapping, option/argument parsing, custom `convert`
//! / `transformAll` error formatting, and `output.bytes` / `output.files` execution.

use crate::env::{
    build_effective_settings, build_native_host, discover_optional_project, effective_working_dir,
    is_uri_like, load_user_settings, normalize_lexical, normalize_source_module, resolve_use_color,
};
use crate::help::HELP_RUN;
use crate::options::{CliError, SubcommandKind, parse_options};
use crate::test_cmd::{pkl_quote_str, strip_trailing_repl_frames};
use pkl_host_native::{NativeHost, file_uri_to_path};
use pkl_runtime::evaluation::command::{
    ArgumentSpec, CommandCollectionKind, CommandMapKind, CommandOptionSpec, CommandSpec,
    CommandValueKind, InvokedCommandLevel, ParsedOptionValue, decode_command_spec,
    extract_source_command_metadata, parse_single_value,
};
use pkl_runtime::evaluation::frontend::{DiagnosticProfile, check_path_spec};
use pkl_runtime::evaluation::session::SourceSession;
use pkl_runtime::modules::ModuleSourceSpec;
use pkl_runtime::packages::mvs::relativize_paths;
use pkl_runtime::pkl_binary_decode::{DecodedValue, decode_lossless};
use pkl_runtime::value::PklString;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const CLIKT_DEFAULT_WIDTH: usize = 78;

const RESERVED_RUN_FLAGS: &[&str] = &[
    "help",
    "allowed-modules",
    "allowed-resources",
    "root-dir",
    "cache-dir",
    "working-dir",
    "property",
    "color",
    "no-cache",
    "format",
    "env-var",
    "module-path",
    "settings",
    "timeout",
    "ca-certificates",
    "http-proxy",
    "http-no-proxy",
    "http-rewrite",
    "http-header",
    "external-module-reader",
    "external-resource-reader",
    "trace-mode",
    "test-port",
    "project-dir",
    "omit-project-settings",
    "no-project",
];

const RESERVED_RUN_SHORTS: &[&str] = &["h", "w", "p", "f", "e", "t"];

pub fn cmd_run(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(SubcommandKind::Run, args)?;

    if positionals.is_empty() {
        if opts.show_help {
            crate::write_help(HELP_RUN);
            return Ok(0);
        }
        return Err(CliError::usage(
            "Usage: pkl [<options>] <command> [<args>]...",
            "missing argument <module>",
        ));
    }

    let raw_module = &positionals[0];
    let mut cmd_args: Vec<String> = positionals[1..].to_vec();
    if opts.show_help {
        cmd_args.push("--help".to_string());
    }

    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let project = discover_optional_project(&opts, &working_dir, &host_env)?;
    let (effective, effective_headers) = build_effective_settings(
        &opts,
        &working_dir,
        project.as_ref(),
        &user_settings,
        &host_env,
    );

    let resolved_module = normalize_source_module(raw_module, &working_dir)?;
    let display_root_name = format_display_module_arg(raw_module);

    let use_color = resolve_use_color(effective.color);
    let host = build_native_host(
        &opts,
        &working_dir,
        project.as_ref(),
        &effective,
        effective_headers.as_ref(),
        &[],
        use_color,
    )?;
    let mut session = host.new_public_session(DiagnosticProfile::CliBundledSettings);

    let outcome = run_command_inner(
        &host,
        &mut session,
        &resolved_module.uri,
        &display_root_name,
        &cmd_args,
        &working_dir,
    );

    session.close();
    host.close();
    outcome
}

fn format_display_module_arg(raw: &str) -> String {
    if raw == "-" {
        return "repl:text".to_string();
    }
    if is_uri_like(raw) {
        return raw.to_string();
    }
    raw.replace('\\', "/")
}

/// Builds a synthetic Pkl module with real module-level `import "pkl:reflect"` /
/// `import "pkl:Command"` declarations and top-level `local function`s (avoiding both
/// `is <let-var>.Type` errors and deep expression scope nesting) that reflects a
/// `pkl:Command` module into the `Mapping` schema consumed by `decode_command_spec`.
fn build_command_spec_reflect_module(module_uri: &str) -> String {
    let quoted_uri = pkl_quote_str(module_uri);
    format!(
        r#"import "pkl:reflect" as reflect
import "pkl:Command" as cmdMod

local __target = import({quoted_uri})
local __cmdClass = reflect.Class(cmdMod.getClass())
local __flagClass = reflect.Class(cmdMod.Flag)
local __boolFlagClass = reflect.Class(cmdMod.BooleanFlag)
local __countedFlagClass = reflect.Class(cmdMod.CountedFlag)
local __argClass = reflect.Class(cmdMod.Argument)

local function __encodeType(ty: Any): Mapping =
  if (ty is reflect.NullableType) new Mapping {{
    ["kind"] = "nullable"
    ["member"] = __encodeType(ty.member)
  }}
  else if (ty is reflect.UnionType) new Mapping {{
    ["kind"] = "union"
    ["members"] = ty.members.map((m) -> __encodeType(m))
  }}
  else if (ty is reflect.StringLiteralType) new Mapping {{
    ["kind"] = "stringLiteral"
    ["value"] = ty.value
  }}
  else if (ty is reflect.DeclaredType) (
    let (ref = ty.referent)
    let (refModUri = ref.enclosingDeclaration.uri)
    if (ref is reflect.TypeAlias) new Mapping {{
      ["kind"] = "typeAlias"
      ["name"] = ref.name
      ["moduleUri"] = refModUri
      ["referent"] = __encodeType(ref.referent)
      ["typeArguments"] = ty.typeArguments.map((a) -> __encodeType(a))
    }} else new Mapping {{
      ["kind"] = "class"
      ["name"] = ref.name
      ["qualifiedName"] = if (refModUri == "pkl:base") ref.name else "\(refModUri)#\(ref.name)"
      ["moduleUri"] = refModUri
      ["isAbstract"] = ref.modifiers.contains("abstract")
      ["typeArguments"] = ty.typeArguments.map((a) -> __encodeType(a))
    }}
  )
  else if (ty is reflect.ModuleType) new Mapping {{ ["kind"] = "module" }}
  else if (ty is reflect.UnknownType) new Mapping {{ ["kind"] = "unknown" }}
  else if (ty is reflect.NothingType) new Mapping {{ ["kind"] = "nothing" }}
  else new Mapping {{ ["kind"] = "other" }}

local function __encodeAnnotation(a: Any): Mapping =
  let (ac = reflect.Class(a.getClass()))
  if (ac == __flagClass) new Mapping {{
    ["kind"] = "Flag"
    ["shortName"] = a.shortName
    ["hide"] = a.hide
    ["metavar"] = a.metavar
    ["multiple"] = a.multiple
    ["hasConvert"] = a.convert != null
    ["hasTransformAll"] = a.transformAll != null
    ["completionCandidates"] =
      if (a.completionCandidates == null) null
      else if (a.completionCandidates == "paths") "paths"
      else a.completionCandidates.toList()
  }}
  else if (ac == __boolFlagClass) new Mapping {{
    ["kind"] = "BooleanFlag"
    ["shortName"] = a.shortName
    ["hide"] = a.hide
  }}
  else if (ac == __countedFlagClass) new Mapping {{
    ["kind"] = "CountedFlag"
    ["shortName"] = a.shortName
    ["hide"] = a.hide
  }}
  else if (ac == __argClass) new Mapping {{
    ["kind"] = "Argument"
    ["multiple"] = a.multiple
    ["hasConvert"] = a.convert != null
    ["hasTransformAll"] = a.transformAll != null
    ["completionCandidates"] =
      if (a.completionCandidates == null) null
      else if (a.completionCandidates == "paths") "paths"
      else a.completionCandidates.toList()
  }}
  else new Mapping {{ ["kind"] = "Other" }}

local function __collectProps(cls: Any, seen: Set): List =
  if (cls == null) List()
  else (
    let (own = cls.properties.fold(List(), (acc, k, prop) ->
      if (
        !seen.contains(k) &&
        !prop.modifiers.contains("external") &&
        !prop.modifiers.contains("abstract") &&
        !prop.modifiers.contains("fixed") &&
        !prop.modifiers.contains("const")
      ) acc + List(new Mapping {{
        ["name"] = prop.name
        ["docComment"] = prop.docComment
        ["type"] = __encodeType(prop.type)
        ["annotations"] = prop.allAnnotations.map((a) -> __encodeAnnotation(a))
      }})
      else acc
    ))
    own + __collectProps(cls.superclass, seen + cls.properties.keys)
  )

local function __inspectCmd(cmd: Any, path: List): Mapping =
  let (rc = reflect.Class(cmd.getClass()))
  if (!rc.isSubclassOf(__cmdClass)) new Mapping {{ ["isCommand"] = false; ["actualClass"] = rc.name }}
  else (
    let (info = cmd.command)
    let (optType = rc.allProperties["options"].type)
    let (optClassRef =
      if (optType is reflect.DeclaredType && optType.referent is reflect.Class) optType.referent
      else null
    )
    let (props =
      if (optClassRef == null || optClassRef.name == "Typed" && optClassRef.enclosingDeclaration.uri == "pkl:base") List()
      else __collectProps(optClassRef, Set())
    )
    let (subs = info.subcommands.toList().mapIndexed((idx, sub) -> __inspectCmd(sub, path + List(idx))))
    new Mapping {{
      ["isCommand"] = true
      ["moduleUri"] = rc.enclosingDeclaration.uri
      ["path"] = path
      ["name"] = info.name
      ["description"] = info.description
      ["hide"] = info.hide
      ["noOp"] = info.noOp
      ["optionsTypeKind"] = if (optClassRef != null) "class" else "other"
      ["optionsClassName"] = if (optClassRef != null) optClassRef.name else ""
      ["optionsClassModuleUri"] = if (optClassRef != null) optClassRef.enclosingDeclaration.uri else ""
      ["optionsClassAbstract"] = if (optClassRef != null) optClassRef.modifiers.contains("abstract") else false
      ["properties"] = props
      ["subcommands"] = subs
    }}
  )

spec = __inspectCmd(__target, List())
"#
    )
}

fn run_command_inner(
    host: &NativeHost,
    session: &mut SourceSession,
    module_uri: &str,
    display_root_name: &str,
    cmd_args: &[String],
    working_dir: &Path,
) -> Result<i32, CliError> {
    let root_spec_src = ModuleSourceSpec {
        uri: PklString::from(module_uri),
        contents: None,
    };

    let reflect_module = build_command_spec_reflect_module(module_uri);
    let reflect_src = ModuleSourceSpec {
        uri: PklString::from("repl:text"),
        contents: Some(reflect_module.as_str().into()),
    };
    let spec_bytes = host
        .evaluate_expression_pkl_binary_on(session, reflect_src, "spec")
        .map_err(|e| CliError::err(strip_trailing_repl_frames(&e)))?;
    let spec_decoded = decode_lossless(&spec_bytes)
        .map_err(|e| CliError::err(format!("Failed to decode CommandSpec: {e:?}")))?;

    let source_text = if module_uri.starts_with("file:") {
        file_uri_to_path(&PklString::from(module_uri))
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
    } else {
        None
    };
    let source_meta = source_text
        .as_deref()
        .map(extract_source_command_metadata)
        .unwrap_or_default();

    let reserved_flags: BTreeSet<String> = RESERVED_RUN_FLAGS.iter().map(|s| (*s).into()).collect();
    let reserved_shorts: BTreeSet<String> =
        RESERVED_RUN_SHORTS.iter().map(|s| (*s).into()).collect();

    let root_spec = decode_command_spec(
        &spec_decoded,
        &source_meta,
        &reserved_flags,
        &reserved_shorts,
    )
    .map_err(CliError::err)?;

    let mut path_names = vec![display_root_name.to_string()];
    let mut chain = Vec::new();
    match parse_clikt_level(
        &root_spec,
        &mut path_names,
        cmd_args,
        true,
        &mut chain,
        host,
        session,
        &root_spec_src,
    ) {
        RunParseOutcome::PrintHelp { text, exit_code } => {
            let level = pkl_runtime::diagnostic::mordant::detect_ansi_level();
            let colored = pkl_runtime::diagnostic::mordant::colorize_clikt_help(&text, level);
            if exit_code == 0 {
                let mut stdout = io::stdout().lock();
                let _ = stdout.write_all(colored.as_bytes());
                let _ = stdout.flush();
                Ok(0)
            } else {
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(colored.as_bytes());
                let _ = stderr.flush();
                Ok(exit_code)
            }
        }
        RunParseOutcome::UsageError { text } => {
            let level = pkl_runtime::diagnostic::mordant::detect_ansi_level();
            let colored =
                pkl_runtime::diagnostic::mordant::colorize_clikt_usage_error(&text, level);
            let mut stderr = io::stderr().lock();
            let _ = stderr.write_all(colored.as_bytes());
            let _ = stderr.flush();
            Ok(1)
        }
        RunParseOutcome::ShellCompletion { shell } => {
            let script = generate_command_completion(&root_spec, display_root_name, &shell);
            let mut stdout = io::stdout().lock();
            let _ = stdout.write_all(script.as_bytes());
            let _ = stdout.flush();
            Ok(0)
        }
        RunParseOutcome::Execute(levels) => {
            let synth_pkl = synthesize_local_command_eval_module(module_uri, &levels);
            let synth_spec = ModuleSourceSpec {
                uri: PklString::from("repl:text"),
                contents: Some(synth_pkl.as_str().into()),
            };

            let output_bytes = host
                .evaluate_output_bytes_on(session, synth_spec.clone())
                .map_err(CliError::err)?;
            if !output_bytes.is_empty() {
                let mut stdout = io::stdout().lock();
                let _ = stdout.write_all(&output_bytes);
                let _ = stdout.flush();
            }

            // Evaluate `output.files`
            const FILES_EXPR: &str = "let (f = output.files) if (f == null) Map() else f.toMap().mapValues((_, v) -> v.bytes)";
            let files_bytes = host
                .evaluate_expression_pkl_binary_on(session, synth_spec, FILES_EXPR)
                .map_err(CliError::err)?;
            let decoded_files = decode_lossless(&files_bytes)
                .map_err(|e| CliError::err(format!("Failed to decode output.files: {e:?}")))?;
            write_command_output_files(&decoded_files, working_dir)?;
            Ok(0)
        }
    }
}

fn synthesize_local_command_eval_module(
    root_module_uri: &str,
    levels: &[InvokedCommandLevel],
) -> String {
    assert!(!levels.is_empty(), "at least one invoked command level");
    let target = levels.last().unwrap();

    let mut out = String::new();
    if target.spec.subcommand_path.is_empty() {
        out.push_str(&format!("amends {}\n\n", pkl_quote_str(root_module_uri)));
    } else {
        out.push_str("import \"pkl:Command\" as __pklCmd\n");
        out.push_str(&format!(
            "local __rootMod = import({})\n",
            pkl_quote_str(root_module_uri)
        ));
    }

    let has_any_custom = levels.iter().any(|l| {
        l.values
            .values()
            .any(|v| matches!(v, ParsedOptionValue::RawCustom { .. }))
    });
    if has_any_custom {
        out.push_str("import \"pkl:reflect\" as __pklReflect\n");
        out.push_str("import \"pkl:Command\" as __pklCmdMod\n");
    }

    if target.spec.subcommand_path.is_empty() {
        out.push_str("options {\n");
        write_local_level_options(&mut out, target, "module", "  ");
        out.push_str("}\n");
    } else {
        let mut parent_expr = String::new();
        for (idx, level) in levels.iter().enumerate() {
            let mut cmd_ref = String::from("__rootMod");
            for sidx in &level.spec.subcommand_path {
                cmd_ref.push_str(&format!(".command.subcommands[{sidx}]"));
            }
            let mut body = String::new();
            body.push_str("options {\n");
            write_local_level_options(&mut body, level, &cmd_ref, "    ");
            body.push_str("  }\n");
            if idx > 0 {
                body.push_str(&format!("  parent = {parent_expr}\n"));
            }
            parent_expr = format!("({cmd_ref}) {{\n  {body}}}");
        }
        out.push_str(&format!("local __executed = {parent_expr}\n"));
        out.push_str("output = __executed.output\n");
    }

    out
}

fn write_local_level_options(
    out: &mut String,
    level: &InvokedCommandLevel,
    cmd_ref: &str,
    indent: &str,
) {
    for (name, val) in &level.values {
        let ident = pkl_ident(name);
        let expr = render_local_parsed_value(val, cmd_ref, name);
        out.push_str(&format!("{indent}{ident} = {expr}\n"));
    }
}

fn render_local_parsed_value(val: &ParsedOptionValue, cmd_ref: &str, prop_name: &str) -> String {
    match val {
        ParsedOptionValue::String(s) => pkl_quote_str(s),
        ParsedOptionValue::Int(i) => i.to_string(),
        ParsedOptionValue::Float(f) => pkl_runtime::numeric::double_to_string(*f),
        ParsedOptionValue::Bool(b) => b.to_string(),
        ParsedOptionValue::Pair(a, b) => format!(
            "Pair({}, {})",
            render_local_parsed_value(a, cmd_ref, prop_name),
            render_local_parsed_value(b, cmd_ref, prop_name)
        ),
        ParsedOptionValue::List(items) => {
            let elems: Vec<String> = items
                .iter()
                .map(|x| render_local_parsed_value(x, cmd_ref, prop_name))
                .collect();
            format!("List({})", elems.join(", "))
        }
        ParsedOptionValue::Set(items) => {
            let elems: Vec<String> = items
                .iter()
                .map(|x| render_local_parsed_value(x, cmd_ref, prop_name))
                .collect();
            format!("Set({})", elems.join(", "))
        }
        ParsedOptionValue::Listing(items) => {
            let elems: Vec<String> = items
                .iter()
                .map(|x| render_local_parsed_value(x, cmd_ref, prop_name))
                .collect();
            format!("new Listing {{ {} }}", elems.join("; "))
        }
        ParsedOptionValue::Map(entries) => {
            let mut args = Vec::with_capacity(entries.len() * 2);
            for (k, v) in entries {
                args.push(render_local_parsed_value(k, cmd_ref, prop_name));
                args.push(render_local_parsed_value(v, cmd_ref, prop_name));
            }
            format!("Map({})", args.join(", "))
        }
        ParsedOptionValue::Mapping(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    format!(
                        "[{}] = {}",
                        render_local_parsed_value(k, cmd_ref, prop_name),
                        render_local_parsed_value(v, cmd_ref, prop_name)
                    )
                })
                .collect();
            format!("new Mapping {{ {} }}", parts.join("; "))
        }
        ParsedOptionValue::RawCustom {
            raw_values,
            has_convert,
            has_transform_all,
            multiple,
            value_kind,
        } => {
            let raw_list: Vec<String> = raw_values.iter().map(|s| pkl_quote_str(s)).collect();
            let raw_list_expr = format!("List({})", raw_list.join(", "));
            let prop_q = pkl_quote_str(prop_name);
            let converted_expr = if *has_convert {
                format!(
                    "let (__a = __pklReflect.Class({cmd_ref}.getClass()).allProperties[\"options\"].type.referent.allProperties[{prop_q}].allAnnotations.find((a) -> a is __pklCmdMod.Flag || a is __pklCmdMod.Argument)) {raw_list_expr}.map((__raw) -> __a.convert.apply(__raw))"
                )
            } else {
                let parsed: Vec<String> = raw_values
                    .iter()
                    .map(|r| match parse_single_value(r, value_kind) {
                        Ok(pv) => render_local_parsed_value(&pv, cmd_ref, prop_name),
                        Err(_) => pkl_quote_str(r),
                    })
                    .collect();
                format!("List({})", parsed.join(", "))
            };
            if *has_transform_all {
                format!(
                    "let (__a = __pklReflect.Class({cmd_ref}.getClass()).allProperties[\"options\"].type.referent.allProperties[{prop_q}].allAnnotations.find((a) -> a is __pklCmdMod.Flag || a is __pklCmdMod.Argument)) __a.transformAll.apply({converted_expr})"
                )
            } else if *multiple {
                match value_kind {
                    CommandValueKind::Collection {
                        kind: CommandCollectionKind::Listing,
                        ..
                    } => format!("({converted_expr}).toListing()"),
                    CommandValueKind::Collection {
                        kind: CommandCollectionKind::Set,
                        ..
                    } => format!("({converted_expr}).toSet()"),
                    CommandValueKind::MapOrMapping {
                        kind: CommandMapKind::Mapping,
                        ..
                    } => format!(
                        "new Mapping {{ for (__p in {converted_expr}) [__p.first] = __p.second }}"
                    ),
                    CommandValueKind::MapOrMapping {
                        kind: CommandMapKind::Map,
                        ..
                    } => {
                        format!("({converted_expr}).toMap((__p) -> __p.first, (__p) -> __p.second)")
                    }
                    _ => converted_expr,
                }
            } else {
                format!("({converted_expr}).last")
            }
        }
    }
}

fn pkl_ident(name: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "abstract",
        "amends",
        "as",
        "class",
        "const",
        "else",
        "extends",
        "external",
        "false",
        "fixed",
        "for",
        "function",
        "hidden",
        "if",
        "import",
        "in",
        "is",
        "let",
        "local",
        "module",
        "new",
        "nothing",
        "null",
        "open",
        "out",
        "outer",
        "read",
        "super",
        "this",
        "throw",
        "trace",
        "true",
        "typealias",
        "unknown",
        "when",
    ];
    let valid_bare = !name.is_empty()
        && !KEYWORDS.contains(&name)
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if valid_bare {
        name.to_string()
    } else {
        format!("`{name}`")
    }
}

fn write_command_output_files(decoded: &DecodedValue, working_dir: &Path) -> Result<(), CliError> {
    let entries = match decoded {
        DecodedValue::Map(e) | DecodedValue::Mapping(e) => e,
        _ => return Ok(()),
    };
    if entries.is_empty() {
        return Ok(());
    }
    if working_dir.exists() && !working_dir.is_dir() {
        return Err(CliError::err(format!(
            "Output path `{}` exists and is not a directory.",
            working_dir.display()
        )));
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| working_dir.to_path_buf());
    let mut written_files: HashMap<PathBuf, String> = HashMap::new();
    let mut stderr = io::stderr().lock();
    #[cfg(windows)]
    const LINE_SEP: &str = "\r\n";
    #[cfg(not(windows))]
    const LINE_SEP: &str = "\n";

    for (k, v) in entries {
        let path_spec_str = match k {
            DecodedValue::String(s) => s.to_java_utf8(),
            other => other.to_java_key_string().to_java_utf8(),
        };
        let pkl_path = PklString::from(path_spec_str.as_str());
        if let Err(msg) = check_path_spec(&pkl_path, cfg!(windows)) {
            return Err(CliError::err(msg.to_java_utf8()));
        }
        let resolved_path = normalize_lexical(&working_dir.join(&path_spec_str));
        let real_path = if resolved_path.exists() {
            std::fs::canonicalize(&resolved_path).unwrap_or_else(|_| resolved_path.clone())
        } else {
            resolved_path.clone()
        };
        if let Some(prev) = written_files.get(&real_path) {
            return Err(CliError::err(format!(
                "Output file conflict: `output.files` entries `\"{prev}\"` and `\"{path_spec_str}\"` resolve to the same file path `{}`.",
                real_path.display()
            )));
        }
        if real_path.is_dir() {
            return Err(CliError::err(format!(
                "Output file conflict: `output.files` entry `\"{path_spec_str}\"` resolves to file path `{}`, which is a directory.",
                real_path.display()
            )));
        }
        written_files.insert(real_path.clone(), path_spec_str.clone());
        if let Some(parent) = real_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::err(format!("Failed to create parent dir: {e}")))?;
        }
        let bytes = match v {
            DecodedValue::Bytes(b) => b.as_slice(),
            DecodedValue::String(s) => s.as_str().unwrap_or("").as_bytes(),
            _ => continue,
        };
        std::fs::write(&real_path, bytes).map_err(|e| {
            CliError::err(format!("Failed to write `{}`: {e}", real_path.display()))
        })?;
        let display_path = if Path::new(&path_spec_str).is_absolute() {
            path_spec_str
        } else {
            relativize_paths(&resolved_path, &cwd)
        };
        let _ = stderr.write_all(format!("{display_path}{LINE_SEP}").as_bytes());
        let _ = stderr.flush();
    }
    Ok(())
}

// ============================================================================
// Clikt-Exact Command Parser, Help Formatter & Custom Convert Validator
// ============================================================================

enum RunParseOutcome {
    Execute(Vec<InvokedCommandLevel>),
    PrintHelp { text: String, exit_code: i32 },
    ShellCompletion { shell: String },
    UsageError { text: String },
}

#[allow(clippy::too_many_arguments)]
fn parse_clikt_level(
    spec: &CommandSpec,
    path_names: &mut Vec<String>,
    args: &[String],
    is_root: bool,
    chain: &mut Vec<InvokedCommandLevel>,
    host: &NativeHost,
    session: &mut SourceSession,
    root_src: &ModuleSourceSpec,
) -> RunParseOutcome {
    let full_cmd = path_names.join(" ");
    let mut raw_flag_values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut bool_flag_values: BTreeMap<String, bool> = BTreeMap::new();
    let mut counted_flag_values: BTreeMap<String, i64> = BTreeMap::new();
    let mut positional: Vec<String> = Vec::new();
    let mut subcmd_invoked: Option<(usize, usize)> = None;
    let mut shell_completion_start: Option<usize> = None;

    let mut i = 0usize;
    let mut after_double_dash = false;
    while i < args.len() {
        let arg = &args[i];
        if !after_double_dash && arg == "--" {
            after_double_dash = true;
            i += 1;
            continue;
        }
        if !after_double_dash && (arg == "-h" || arg == "--help") {
            return RunParseOutcome::PrintHelp {
                text: render_clikt_command_help(spec, &full_cmd, is_root),
                exit_code: 0,
            };
        }
        if !after_double_dash && arg.starts_with("--") {
            let body = &arg[2..];
            let (opt_name, attached_val) = match body.split_once('=') {
                Some((k, v)) => (k, Some(v)),
                None => (body, None),
            };
            if let Some(neg) = opt_name.strip_prefix("no-")
                && spec
                    .options
                    .iter()
                    .any(|o| matches!(o, CommandOptionSpec::BooleanFlag(b) if b.name == neg))
            {
                if attached_val.is_some() {
                    return usage_err(
                        spec,
                        &full_cmd,
                        is_root,
                        &format!("option --{opt_name} does not take a value"),
                    );
                }
                bool_flag_values.insert(neg.to_string(), false);
                i += 1;
                continue;
            }
            if let Some(opt) = spec.options.iter().find(|o| match o {
                CommandOptionSpec::Flag(f) => f.name == opt_name,
                CommandOptionSpec::BooleanFlag(f) => f.name == opt_name,
                CommandOptionSpec::CountedFlag(f) => f.name == opt_name,
                CommandOptionSpec::Argument(_) => false,
            }) {
                match opt {
                    CommandOptionSpec::Flag(f) => {
                        let val = if let Some(v) = attached_val {
                            i += 1;
                            v.to_string()
                        } else if i + 1 < args.len() {
                            let v = args[i + 1].clone();
                            i += 2;
                            v
                        } else {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("option --{} requires a value", f.name),
                            );
                        };
                        raw_flag_values.entry(f.name.clone()).or_default().push(val);
                    }
                    CommandOptionSpec::BooleanFlag(f) => {
                        if attached_val.is_some() {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("option --{} does not take a value", f.name),
                            );
                        }
                        bool_flag_values.insert(f.name.clone(), true);
                        i += 1;
                    }
                    CommandOptionSpec::CountedFlag(f) => {
                        if attached_val.is_some() {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("option --{} does not take a value", f.name),
                            );
                        }
                        *counted_flag_values.entry(f.name.clone()).or_insert(0) += 1;
                        i += 1;
                    }
                    CommandOptionSpec::Argument(_) => unreachable!(),
                }
                continue;
            }
            return usage_err(
                spec,
                &full_cmd,
                is_root,
                &format!("no such option --{opt_name}"),
            );
        }
        if !after_double_dash && arg.starts_with('-') && arg.len() > 1 {
            let chars: Vec<char> = arg[1..].chars().collect();
            let mut ci = 0;
            while ci < chars.len() {
                let c = chars[ci].to_string();
                if c == "h" {
                    return RunParseOutcome::PrintHelp {
                        text: render_clikt_command_help(spec, &full_cmd, is_root),
                        exit_code: 0,
                    };
                }
                let Some(opt) = spec.options.iter().find(|o| match o {
                    CommandOptionSpec::Flag(f) => f.short_name.as_deref() == Some(c.as_str()),
                    CommandOptionSpec::BooleanFlag(f) => {
                        f.short_name.as_deref() == Some(c.as_str())
                    }
                    CommandOptionSpec::CountedFlag(f) => {
                        f.short_name.as_deref() == Some(c.as_str())
                    }
                    CommandOptionSpec::Argument(_) => false,
                }) else {
                    return usage_err(spec, &full_cmd, is_root, &format!("no such option -{c}"));
                };
                match opt {
                    CommandOptionSpec::Flag(f) => {
                        let rest: String = chars[ci + 1..].iter().collect();
                        let val = if !rest.is_empty() {
                            i += 1;
                            rest
                        } else if i + 1 < args.len() {
                            let v = args[i + 1].clone();
                            i += 2;
                            v
                        } else {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("option -{c} requires a value"),
                            );
                        };
                        raw_flag_values.entry(f.name.clone()).or_default().push(val);
                        break;
                    }
                    CommandOptionSpec::BooleanFlag(f) => {
                        bool_flag_values.insert(f.name.clone(), true);
                        ci += 1;
                        if ci == chars.len() {
                            i += 1;
                        }
                    }
                    CommandOptionSpec::CountedFlag(f) => {
                        *counted_flag_values.entry(f.name.clone()).or_insert(0) += 1;
                        ci += 1;
                        if ci == chars.len() {
                            i += 1;
                        }
                    }
                    CommandOptionSpec::Argument(_) => unreachable!(),
                }
            }
            continue;
        }

        let arg_specs: Vec<&ArgumentSpec> = spec
            .options
            .iter()
            .filter_map(|o| match o {
                CommandOptionSpec::Argument(a) => Some(a),
                _ => None,
            })
            .collect();
        if positional.len() >= arg_specs.iter().filter(|a| !a.repeated).count() {
            if is_root && arg == "shell-completion" {
                shell_completion_start = Some(i + 1);
                break;
            }
            if let Some(sub_idx) = spec.subcommands.iter().position(|s| s.name == *arg) {
                subcmd_invoked = Some((sub_idx, i + 1));
                break;
            }
        }
        positional.push(arg.clone());
        i += 1;
    }

    if let Some(sc_start) = shell_completion_start {
        let sc_args = &args[sc_start..];
        let sc_usage = format_wrapped_usage(
            &format!("{full_cmd} shell-completion"),
            &["[<options>]", "<shell>"],
        );
        if sc_args.iter().any(|a| a == "-h" || a == "--help") {
            return RunParseOutcome::PrintHelp {
                text: format!(
                    "{sc_usage}\n\n  Generate a completion script for the given shell\n\nOptions:\n  -h, --help  Show this message and exit\n"
                ),
                exit_code: 0,
            };
        }
        let Some(shell) = sc_args.first() else {
            return RunParseOutcome::UsageError {
                text: format!("{sc_usage}\n\nError: missing argument <shell>\n"),
            };
        };
        if !matches!(shell.as_str(), "bash" | "zsh" | "fish") {
            return RunParseOutcome::UsageError {
                text: format!(
                    "{sc_usage}\n\nError: invalid value for <shell>: invalid choice: {shell}. (choose from bash, zsh, fish)\n"
                ),
            };
        }
        return RunParseOutcome::ShellCompletion {
            shell: shell.clone(),
        };
    }

    // Bind options and eagerly validate custom `convert` / `transformAll` matching Clikt.
    let mut values: BTreeMap<String, ParsedOptionValue> = BTreeMap::new();
    for opt in &spec.options {
        match opt {
            CommandOptionSpec::Flag(f) => {
                let raws = raw_flag_values.remove(&f.name).unwrap_or_default();
                if raws.is_empty() {
                    if f.required {
                        return usage_err(
                            spec,
                            &full_cmd,
                            is_root,
                            &format!("missing option --{}", f.name),
                        );
                    }
                    continue;
                }
                if f.has_convert || f.has_transform_all {
                    if let Err(err_msg) = validate_custom_option(
                        host,
                        session,
                        root_src,
                        &spec.subcommand_path,
                        &f.name,
                        &raws,
                        f.has_convert,
                        f.has_transform_all,
                        &f.value_kind,
                    ) {
                        return usage_err_bad_convert(
                            spec,
                            &full_cmd,
                            is_root,
                            &format!("--{}", f.name),
                            &err_msg,
                        );
                    }
                    values.insert(
                        f.name.clone(),
                        ParsedOptionValue::RawCustom {
                            raw_values: raws,
                            has_convert: f.has_convert,
                            has_transform_all: f.has_transform_all,
                            multiple: f.multiple,
                            value_kind: f.value_kind.clone(),
                        },
                    );
                } else {
                    match parse_all_values(&raws, &f.value_kind, f.multiple) {
                        Ok(parsed) => {
                            values.insert(f.name.clone(), parsed);
                        }
                        Err(msg) => {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("invalid value for --{}: {msg}", f.name),
                            );
                        }
                    }
                }
            }
            CommandOptionSpec::BooleanFlag(f) => {
                if let Some(b) = bool_flag_values.remove(&f.name) {
                    values.insert(f.name.clone(), ParsedOptionValue::Bool(b));
                } else if let Some(def) = f.default_value {
                    values.insert(f.name.clone(), ParsedOptionValue::Bool(def));
                }
            }
            CommandOptionSpec::CountedFlag(f) => {
                let count = counted_flag_values.remove(&f.name).unwrap_or(0);
                values.insert(f.name.clone(), ParsedOptionValue::Int(count));
            }
            CommandOptionSpec::Argument(_) => {}
        }
    }

    // Bind positional arguments
    let arg_specs: Vec<&ArgumentSpec> = spec
        .options
        .iter()
        .filter_map(|o| match o {
            CommandOptionSpec::Argument(a) => Some(a),
            _ => None,
        })
        .collect();
    let mut pos_idx = 0usize;
    for (a_idx, a) in arg_specs.iter().enumerate() {
        if a.repeated {
            let remaining_after = arg_specs.len() - a_idx - 1;
            let take_end = positional.len().saturating_sub(remaining_after);
            let slice: Vec<String> = if pos_idx <= take_end {
                positional[pos_idx..take_end].to_vec()
            } else {
                Vec::new()
            };
            pos_idx = take_end;
            if slice.is_empty() {
                continue;
            }
            if a.has_convert || a.has_transform_all {
                if let Err(err_msg) = validate_custom_option(
                    host,
                    session,
                    root_src,
                    &spec.subcommand_path,
                    &a.name,
                    &slice,
                    a.has_convert,
                    a.has_transform_all,
                    &a.value_kind,
                ) {
                    return usage_err_bad_convert(
                        spec,
                        &full_cmd,
                        is_root,
                        &format!("<{}>", a.name),
                        &err_msg,
                    );
                }
                values.insert(
                    a.name.clone(),
                    ParsedOptionValue::RawCustom {
                        raw_values: slice,
                        has_convert: a.has_convert,
                        has_transform_all: a.has_transform_all,
                        multiple: true,
                        value_kind: a.value_kind.clone(),
                    },
                );
            } else {
                match parse_all_values(&slice, &a.value_kind, true) {
                    Ok(parsed) => {
                        values.insert(a.name.clone(), parsed);
                    }
                    Err(msg) => {
                        return usage_err(
                            spec,
                            &full_cmd,
                            is_root,
                            &format!("invalid value for <{}>: {msg}", a.name),
                        );
                    }
                }
            }
        } else {
            let Some(raw) = positional.get(pos_idx) else {
                return usage_err(
                    spec,
                    &full_cmd,
                    is_root,
                    &format!("missing argument <{}>", a.name),
                );
            };
            pos_idx += 1;
            if a.has_convert || a.has_transform_all {
                let single = vec![raw.clone()];
                if let Err(err_msg) = validate_custom_option(
                    host,
                    session,
                    root_src,
                    &spec.subcommand_path,
                    &a.name,
                    &single,
                    a.has_convert,
                    a.has_transform_all,
                    &a.value_kind,
                ) {
                    return usage_err_bad_convert(
                        spec,
                        &full_cmd,
                        is_root,
                        &format!("<{}>", a.name),
                        &err_msg,
                    );
                }
                values.insert(
                    a.name.clone(),
                    ParsedOptionValue::RawCustom {
                        raw_values: single,
                        has_convert: a.has_convert,
                        has_transform_all: a.has_transform_all,
                        multiple: false,
                        value_kind: a.value_kind.clone(),
                    },
                );
            } else {
                match parse_single_value(raw, &a.value_kind) {
                    Ok(parsed) => {
                        values.insert(a.name.clone(), parsed);
                    }
                    Err(msg) => {
                        return usage_err(
                            spec,
                            &full_cmd,
                            is_root,
                            &format!("invalid value for <{}>: {msg}", a.name),
                        );
                    }
                }
            }
        }
    }

    if pos_idx < positional.len() {
        let extra = &positional[pos_idx..];
        let msg = if extra.len() == 1 {
            format!("got unexpected extra argument ({})", extra[0])
        } else {
            format!("got unexpected extra arguments ({})", extra.join(" "))
        };
        return usage_err(spec, &full_cmd, is_root, &msg);
    }

    chain.push(InvokedCommandLevel {
        spec: spec.clone(),
        values,
    });

    if let Some((sub_idx, sub_start)) = subcmd_invoked {
        let sub_spec = &spec.subcommands[sub_idx];
        path_names.push(sub_spec.name.clone());
        return parse_clikt_level(
            sub_spec,
            path_names,
            &args[sub_start..],
            false,
            chain,
            host,
            session,
            root_src,
        );
    }

    if !spec.subcommands.is_empty() && spec.no_op {
        return RunParseOutcome::PrintHelp {
            text: render_clikt_command_help(spec, &full_cmd, is_root),
            exit_code: 1,
        };
    }

    RunParseOutcome::Execute(std::mem::take(chain))
}

#[allow(clippy::too_many_arguments)]
fn validate_custom_option(
    host: &NativeHost,
    session: &mut SourceSession,
    root_src: &ModuleSourceSpec,
    subcommand_path: &[usize],
    prop_name: &str,
    raw_values: &[String],
    has_convert: bool,
    has_transform_all: bool,
    value_kind: &CommandValueKind,
) -> Result<(), String> {
    let mut cmd_ref = String::from("module");
    for idx in subcommand_path {
        cmd_ref.push_str(&format!(".command.subcommands[{idx}]"));
    }
    let prop_q = pkl_quote_str(prop_name);

    if has_convert {
        for raw in raw_values {
            let raw_q = pkl_quote_str(raw);
            let expr = format!(
                "let (__r = import(\"pkl:reflect\")) let (__c = import(\"pkl:Command\")) let (__fc = __r.Class(__c.Flag)) let (__ac = __r.Class(__c.Argument)) let (__a = __r.Class({cmd_ref}.getClass()).allProperties[\"options\"].type.referent.allProperties[{prop_q}].allAnnotations.find((a) -> let (__cls = __r.Class(a.getClass())) __cls == __fc || __cls == __ac)) let (_ = __a.convert.apply({raw_q})) true"
            );
            if let Err(err) =
                host.evaluate_expression_pkl_binary_on(session, root_src.clone(), &expr)
            {
                return Err(strip_trailing_repl_frames(&err));
            }
        }
    } else {
        for raw in raw_values {
            parse_single_value(raw, value_kind)?;
        }
    }

    if has_transform_all {
        let list_expr = if has_convert {
            let raws: Vec<String> = raw_values.iter().map(|s| pkl_quote_str(s)).collect();
            format!(
                "List({}).map((__v) -> __a.convert.apply(__v))",
                raws.join(", ")
            )
        } else {
            let raws: Vec<String> = raw_values.iter().map(|s| pkl_quote_str(s)).collect();
            format!("List({})", raws.join(", "))
        };
        let expr = format!(
            "let (__r = import(\"pkl:reflect\")) let (__c = import(\"pkl:Command\")) let (__fc = __r.Class(__c.Flag)) let (__ac = __r.Class(__c.Argument)) let (__a = __r.Class({cmd_ref}.getClass()).allProperties[\"options\"].type.referent.allProperties[{prop_q}].allAnnotations.find((a) -> let (__cls = __r.Class(a.getClass())) __cls == __fc || __cls == __ac)) let (_ = __a.transformAll.apply({list_expr})) true"
        );
        if let Err(err) = host.evaluate_expression_pkl_binary_on(session, root_src.clone(), &expr) {
            return Err(strip_trailing_repl_frames(&err));
        }
    }

    Ok(())
}

fn parse_all_values(
    raws: &[String],
    kind: &CommandValueKind,
    multiple: bool,
) -> Result<ParsedOptionValue, String> {
    let mut parsed_items = Vec::with_capacity(raws.len());
    for raw in raws {
        parsed_items.push(parse_single_value(raw, kind)?);
    }
    if !multiple {
        return Ok(parsed_items.pop().unwrap());
    }
    match kind {
        CommandValueKind::Collection {
            kind: CommandCollectionKind::Listing,
            ..
        } => Ok(ParsedOptionValue::Listing(parsed_items)),
        CommandValueKind::Collection {
            kind: CommandCollectionKind::List,
            ..
        } => Ok(ParsedOptionValue::List(parsed_items)),
        CommandValueKind::Collection {
            kind: CommandCollectionKind::Set,
            ..
        } => Ok(ParsedOptionValue::Set(parsed_items)),
        CommandValueKind::MapOrMapping {
            kind: CommandMapKind::Mapping,
            ..
        } => {
            let pairs = parsed_items
                .into_iter()
                .map(|p| match p {
                    ParsedOptionValue::Pair(k, v) => (*k, *v),
                    other => (ParsedOptionValue::String(String::new()), other),
                })
                .collect();
            Ok(ParsedOptionValue::Mapping(pairs))
        }
        CommandValueKind::MapOrMapping {
            kind: CommandMapKind::Map,
            ..
        } => {
            let pairs = parsed_items
                .into_iter()
                .map(|p| match p {
                    ParsedOptionValue::Pair(k, v) => (*k, *v),
                    other => (ParsedOptionValue::String(String::new()), other),
                })
                .collect();
            Ok(ParsedOptionValue::Map(pairs))
        }
        _ => Ok(parsed_items.pop().unwrap()),
    }
}

fn format_clikt_usage(spec: &CommandSpec, full_cmd: &str, is_root: bool) -> String {
    let mut parts: Vec<String> = vec!["[<options>]".to_string()];
    for opt in &spec.options {
        if let CommandOptionSpec::Argument(a) = opt {
            if a.repeated {
                parts.push(format!("[<{}>]...", a.name));
            } else {
                parts.push(format!("<{}>", a.name));
            }
        }
    }
    if is_root || !spec.subcommands.is_empty() {
        parts.push("<command>".to_string());
        parts.push("[<args>]...".to_string());
    }
    let part_refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    format_wrapped_usage(full_cmd, &part_refs)
}

fn format_wrapped_usage(full_cmd: &str, parts: &[&str]) -> String {
    let mut out = format!("Usage: {full_cmd}");
    let mut line_len = out.chars().count();
    for part in parts {
        let part_len = part.chars().count();
        if line_len + 1 + part_len > CLIKT_DEFAULT_WIDTH {
            out.push_str("\n    ");
            out.push_str(part);
            line_len = 4 + part_len;
        } else {
            out.push(' ');
            out.push_str(part);
            line_len += 1 + part_len;
        }
    }
    out
}

fn usage_err(spec: &CommandSpec, full_cmd: &str, is_root: bool, msg: &str) -> RunParseOutcome {
    let usage = format_clikt_usage(spec, full_cmd, is_root);
    RunParseOutcome::UsageError {
        text: format!("{usage}\n\nError: {msg}\n"),
    }
}

fn usage_err_bad_convert(
    spec: &CommandSpec,
    full_cmd: &str,
    is_root: bool,
    opt_display: &str,
    pkl_err: &str,
) -> RunParseOutcome {
    let usage = format_clikt_usage(spec, full_cmd, is_root);
    let mut body = pkl_err.to_string();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    RunParseOutcome::UsageError {
        text: format!("{usage}\n\nError: invalid value for {opt_display}: \n{body}\n"),
    }
}

fn format_clikt_metavar(raw: &str) -> String {
    let inner = raw
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(raw);
    format!("<{inner}>")
}

fn render_clikt_command_help(spec: &CommandSpec, full_cmd: &str, is_root: bool) -> String {
    let mut out = format_clikt_usage(spec, full_cmd, is_root);
    out.push('\n');
    if let Some(ref desc) = spec.description {
        let trimmed = desc.trim();
        if !trimmed.is_empty() {
            out.push('\n');
            for line in trimmed.lines() {
                if line.is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&format!("  {line}\n"));
                }
            }
        }
    }

    let mut opt_rows: Vec<(String, String)> = Vec::new();
    for opt in &spec.options {
        match opt {
            CommandOptionSpec::Flag(f) if !f.hidden => {
                let mv = format_clikt_metavar(&f.metavar);
                let names = match &f.short_name {
                    Some(s) => format!("-{s}, --{}={mv}", f.name),
                    None => format!("--{}={mv}", f.name),
                };
                let help = f.help_text.clone().unwrap_or_default().trim().to_string();
                opt_rows.push((names, help));
            }
            CommandOptionSpec::BooleanFlag(f) if !f.hidden => {
                let names = match &f.short_name {
                    Some(s) => format!("-{s}, --{} / --no-{}", f.name, f.name),
                    None => format!("--{} / --no-{}", f.name, f.name),
                };
                opt_rows.push((
                    names,
                    f.help_text.clone().unwrap_or_default().trim().to_string(),
                ));
            }
            CommandOptionSpec::CountedFlag(f) if !f.hidden => {
                let names = match &f.short_name {
                    Some(s) => format!("-{s}, --{}", f.name),
                    None => format!("--{}", f.name),
                };
                opt_rows.push((
                    names,
                    f.help_text.clone().unwrap_or_default().trim().to_string(),
                ));
            }
            _ => {}
        }
    }
    opt_rows.push(("-h, --help".into(), "Show this message and exit".into()));

    out.push_str("\nOptions:\n");
    let max_opt = opt_rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    for (left, right) in &opt_rows {
        if right.is_empty() {
            out.push_str(&format!("  {left}\n"));
        } else {
            let pad = " ".repeat(max_opt - left.len() + 2);
            out.push_str(&format!("  {left}{pad}{right}\n"));
        }
    }

    let arg_rows: Vec<(String, String)> = spec
        .options
        .iter()
        .filter_map(|o| match o {
            CommandOptionSpec::Argument(a) => Some((
                format!("<{}>", a.name),
                a.help_text.clone().unwrap_or_default().trim().to_string(),
            )),
            _ => None,
        })
        .collect();
    if !arg_rows.is_empty() {
        out.push_str("\nArguments:\n");
        let max_arg = arg_rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
        for (left, right) in &arg_rows {
            if right.is_empty() {
                out.push_str(&format!("  {left}\n"));
            } else {
                let pad = " ".repeat(max_arg - left.len() + 2);
                out.push_str(&format!("  {left}{pad}{right}\n"));
            }
        }
    }

    let mut cmd_rows: Vec<(String, String)> = spec
        .subcommands
        .iter()
        .filter(|s| !s.hidden)
        .map(|s| {
            (
                s.name.clone(),
                s.description.clone().unwrap_or_default().trim().to_string(),
            )
        })
        .collect();
    if is_root {
        cmd_rows.push((
            "shell-completion".into(),
            "Generate a completion script for the given shell".into(),
        ));
    }
    if !cmd_rows.is_empty() {
        out.push_str("\nCommands:\n");
        let max_cmd = cmd_rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
        for (left, right) in &cmd_rows {
            if right.is_empty() {
                out.push_str(&format!("  {left}\n"));
            } else {
                let pad = " ".repeat(max_cmd - left.len() + 2);
                out.push_str(&format!("  {left}{pad}{right}\n"));
            }
        }
    }
    out
}

fn generate_command_completion(spec: &CommandSpec, cmd_name: &str, shell: &str) -> String {
    let mut subcmds: Vec<&str> = spec
        .subcommands
        .iter()
        .filter(|s| !s.hidden)
        .map(|s| s.name.as_str())
        .collect();
    subcmds.push("shell-completion");
    match shell {
        "fish" => {
            let mut out = format!("# Command completion for {cmd_name}\n# Generated by Clikt\n\n");
            out.push_str(&format!("set -l subcommands '{}'\n", subcmds.join(" ")));
            out
        }
        "zsh" => format!(
            "#!/usr/bin/env zsh\n# Command completion for {cmd_name}\n# Generated by Clikt\n"
        ),
        _ => format!(
            "#!/usr/bin/env bash\n# Command completion for {cmd_name}\n# Generated by Clikt\n"
        ),
    }
}
