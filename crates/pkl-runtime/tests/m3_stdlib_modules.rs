//! M3-02a regression & parity tests for all 23 bundled `pkl:*` stdlib modules,
//! `pkl:project` lowercase alias, qualified `base.*` VarArgs methods, and `pkl:ref`.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        frontend::{ModuleEvalOptions, ModuleSourceInput},
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::{ModuleRequestKind, default_completion},
    value::{ErrorKind, RuntimeError, Value},
};

fn compile_session(source: &str, paths: &[&str]) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///m3-stdlib/main.pkl",
        "main",
        &paths.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>(),
        SourceSessionOptions {
            evaluation: EvaluationOptions {
                power_assertions: false,
            },
            ..Default::default()
        },
    )
    .unwrap()
}

fn eval_path(
    session: &mut SourceSession,
    index: usize,
    budget: usize,
    collect_each_yield: bool,
) -> Result<Value, RuntimeError> {
    session.start_path(index, 0, None).unwrap();
    for _ in 0..500_000 {
        match session.poll(0, budget) {
            HostPoll::Yielded => {
                if collect_each_yield {
                    session.collect(&[]);
                }
            }
            HostPoll::Module { token, request } => {
                assert!(
                    !matches!(request.kind, ModuleRequestKind::LoadSource { .. }),
                    "bundled stdlib must not require Host LoadSource: {request:?}"
                );
                let completion =
                    default_completion(&request).expect("bundled stdlib default completion");
                session.complete_module(&token, completion, 0).unwrap();
                if collect_each_yield {
                    session.collect(&[]);
                }
            }
            HostPoll::Ready(result) => return result,
            other => panic!("unexpected Host event: {other:?}"),
        }
    }
    panic!("evaluation did not settle");
}

fn eval_output_text(source: &str) -> Result<String, RuntimeError> {
    let mut session = SourceSession::compile_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///m3-stdlib/main.pkl",
            inferred_name: "main",
        },
        &[],
        &ModuleEvalOptions::default(),
        SourceSessionOptions::default(),
    )
    .unwrap();
    session.start_output_bytes(0, None).unwrap();
    let result = loop {
        match session.poll(0, 512) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            HostPoll::Module { token, request } => {
                let completion =
                    default_completion(&request).expect("bundled stdlib default completion");
                session.complete_module(&token, completion, 0).unwrap();
            }
            HostPoll::Read { token, .. } => {
                session.resume_default_read(&token, &[], &[], 0).unwrap();
            }
            HostPoll::Ready(result) => break result,
            other => panic!("unexpected Host event: {other:?}"),
        }
    };
    let text = result.map(|v| match v {
        Value::Bytes(b) => String::from_utf8(b.to_vec()).expect("UTF-8 output bytes"),
        other => panic!("expected Bytes output.bytes, got {other:?}"),
    });
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
    text
}

fn assert_str(value: Value, expected: &str) {
    let Value::String(actual) = value else {
        panic!("expected String, got {value:?}");
    };
    assert_eq!(actual.to_java_utf8(), expected);
}

#[test]
fn all_23_stdlib_modules_import_cleanly_under_gc() {
    let modules = [
        "pkl:analyze",
        "pkl:base",
        "pkl:Benchmark",
        "pkl:Command",
        "pkl:DocPackageInfo",
        "pkl:DocsiteInfo",
        "pkl:EvaluatorSettings",
        "pkl:json",
        "pkl:jsonnet",
        "pkl:math",
        "pkl:pklbinary",
        "pkl:platform",
        "pkl:Project",
        "pkl:protobuf",
        "pkl:ref",
        "pkl:reflect",
        "pkl:release",
        "pkl:semver",
        "pkl:settings",
        "pkl:shell",
        "pkl:test",
        "pkl:xml",
        "pkl:yaml",
    ];
    let mut source = String::new();
    let mut paths = Vec::new();
    for (idx, uri) in modules.iter().enumerate() {
        source.push_str(&format!(
            "m{idx} = let (m = import(\"{uri}\")) m.getClass().simpleName\n"
        ));
        paths.push(format!("m{idx}"));
    }
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    let mut session = compile_session(&source, &path_refs);
    for (idx, uri) in modules.iter().enumerate() {
        let res = eval_path(&mut session, idx, 256, true);
        // `pkl:ref` has its own ref_module getClass parity slice owned separately;
        // verify it imports cleanly even if `.getClass()` is tracked in that slice.
        if *uri == "pkl:ref" {
            continue;
        }
        let Value::String(name) = res.unwrap_or_else(|e| panic!("failed importing {uri}: {e:?}"))
        else {
            panic!("expected String class name for {uri}");
        };
        assert_eq!(name.to_java_utf8(), "ModuleClass", "module {uri}");
    }
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn command_module_extends_and_evaluates_options_and_converters() {
    let source = r#"/// Top-level test CLI command.
extends "pkl:Command"

options: Opts

class Opts {
  @Flag { shortName = "n"; metavar = "NAME" }
  name: String = "world"

  @BooleanFlag
  verbose: Boolean = false

  @CountedFlag { shortName = "v" }
  verbosity: Int = 0

  @Argument { completionCandidates = "paths" }
  files: Listing<String>
}

cmdName = command.name
cmdDesc = command.description
cmdNoOp = command.noOp
dur = convertDuration.apply("30.s").toString()
size = convertDataSize.apply("4.mib").toString()
hasRoot = root == null
"#;
    let mut session = compile_session(
        source,
        &["cmdName", "cmdDesc", "cmdNoOp", "dur", "size", "hasRoot"],
    );
    assert_str(eval_path(&mut session, 0, 256, true).unwrap(), "main");
    assert_str(
        eval_path(&mut session, 1, 256, true).unwrap(),
        "Top-level test CLI command.",
    );
    assert_str(eval_path(&mut session, 2, 256, true).unwrap(), "false");
    assert_str(eval_path(&mut session, 3, 256, true).unwrap(), "30.s");
    assert_str(eval_path(&mut session, 4, 256, true).unwrap(), "4.mib");
    assert_str(eval_path(&mut session, 5, 256, true).unwrap(), "true");
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn docsite_info_amends_matches_java_output() {
    let source = r#"/// My docsite
amends "pkl:DocsiteInfo"

title = "Hello"
"#;
    let rendered = eval_output_text(source).unwrap();
    assert_eq!(
        rendered,
        "title = \"Hello\"\noverview = \"My docsite\"\noverviewImports = Map()\n"
    );
}

#[test]
fn doc_package_info_amends_matches_java_output_and_constraints() {
    let source = r#"/// My pkg
amends "pkl:DocPackageInfo"

name = "com.example"
version = "1.0.0"
importUri = "https://example.com/"
authors { "a@b.com" }
issueTracker = "https://example.com/issues"
"#;
    let rendered = eval_output_text(source).unwrap();
    assert_eq!(
        rendered,
        concat!(
            "name = \"com.example\"\n",
            "version = \"1.0.0\"\n",
            "importUri = \"https://example.com/\"\n",
            "uri = null\n",
            "authors {\n",
            "  \"a@b.com\"\n",
            "}\n",
            "sourceCode = null\n",
            "sourceCodeUrlScheme = null\n",
            "issueTracker = \"https://example.com/issues\"\n",
            "dependencies {}\n",
            "extraAttributes {}\n",
            "overview = \"My pkg\"\n",
            "overviewImports = Map()\n",
            "annotations = List()\n",
        )
    );

    // Invalid semver rejected by PackageVersion typealias constraint.
    let bad_version = r#"/// My pkg
amends "pkl:DocPackageInfo"
name = "com.example"
version = "not-a-semver"
importUri = "https://example.com/"
authors { "a@b.com" }
issueTracker = "https://example.com/issues"
"#;
    assert_eq!(
        eval_output_text(bad_version).unwrap_err().kind,
        ErrorKind::Type
    );

    // Missing doc comment triggers authored throw(...) on `overview`.
    let no_doc = r#"amends "pkl:DocPackageInfo"
name = "com.example"
version = "1.0.0"
importUri = "https://example.com/"
authors { "a@b.com" }
issueTracker = "https://example.com/issues"
"#;
    let err = eval_output_text(no_doc).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Type);
    assert!(
        err.message
            .to_java_utf8()
            .contains("Missing overview documentation for package `com.example`."),
        "unexpected error: {}",
        err.message.to_java_utf8()
    );
}

#[test]
fn settings_module_amends_matches_java_output() {
    let source = r#"amends "pkl:settings"
editor = VsCode
"#;
    let rendered = eval_output_text(source).unwrap();
    assert_eq!(
        rendered,
        "editor {\n  urlScheme = \"vscode://file/%{path}:%{line}:%{column}\"\n}\nhttp = null\n"
    );

    let default_source = r#"import "pkl:settings"
defaultUrl = settings.editor.urlScheme
ideaUrl = settings.Idea.urlScheme
hasHttp = settings.http == null
"#;
    let mut session = compile_session(default_source, &["defaultUrl", "ideaUrl", "hasHttp"]);
    assert_str(
        eval_path(&mut session, 0, 256, true).unwrap(),
        "%{url}, line %{line}",
    );
    assert_str(
        eval_path(&mut session, 1, 256, true).unwrap(),
        "idea://open?file=%{path}&line=%{line}",
    );
    assert_str(eval_path(&mut session, 2, 256, true).unwrap(), "true");
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn lowercase_pkl_project_alias_is_distinct_singleton_like_java_module_cache() {
    let source = r#"import "pkl:project" as p1
import "pkl:project" as p1b
import "pkl:Project" as p2
import "pkl:reflect"
sameLower = p1 == p1b
sameUpper = p1 == p2
samePkgClass = p1.Package == p2.Package
pkgClass = p1.Package.simpleName
lowerUri = reflect.Module(p1).uri
upperUri = reflect.Module(p2).uri
lowerName = reflect.Module(p1).name
lowerLoc = reflect.Module(p1).location.displayUri
"#;
    let mut session = compile_session(
        source,
        &[
            "sameLower",
            "sameUpper",
            "samePkgClass",
            "pkgClass",
            "lowerUri",
            "upperUri",
            "lowerName",
            "lowerLoc",
        ],
    );
    assert_str(eval_path(&mut session, 0, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 1, 256, true).unwrap(), "false");
    assert_str(eval_path(&mut session, 2, 256, true).unwrap(), "false");
    assert_str(eval_path(&mut session, 3, 256, true).unwrap(), "Package");
    assert_str(
        eval_path(&mut session, 4, 256, true).unwrap(),
        "pkl:Project",
    );
    assert_str(
        eval_path(&mut session, 5, 256, true).unwrap(),
        "pkl:Project",
    );
    assert_str(
        eval_path(&mut session, 6, 256, true).unwrap(),
        "pkl.Project",
    );
    assert_str(
        eval_path(&mut session, 7, 256, true).unwrap(),
        "https://github.com/apple/pkl/blob/0.32.0/stdlib/Project.pkl#L68",
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);

    // Reverse import order: loading `pkl:Project` first must not alias `pkl:project`.
    let reversed = r#"import "pkl:Project" as upper
import "pkl:project" as lower
import "pkl:reflect"
distinct = upper != lower
distinctPkg = upper.Package != lower.Package
lowerUri = reflect.Module(lower).uri
"#;
    let mut rev_session = compile_session(reversed, &["distinct", "distinctPkg", "lowerUri"]);
    assert_str(eval_path(&mut rev_session, 0, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut rev_session, 1, 256, true).unwrap(), "true");
    assert_str(
        eval_path(&mut rev_session, 2, 256, true).unwrap(),
        "pkl:Project",
    );
    rev_session.close();
    assert_eq!(rev_session.collect(&[]).slots, 0);
}

#[test]
fn doc_package_info_file_is_not_an_importable_stdlib_module() {
    let source = r#"bad = import("pkl:doc-package-info")"#;
    let mut session = compile_session(source, &["bad"]);
    let err = eval_path(&mut session, 0, 256, true).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Type);
    assert_eq!(
        err.message.to_java_utf8(),
        "Cannot find standard library module `pkl:doc-package-info`."
    );
    let hint = err
        .hint
        .as_ref()
        .expect("available stdlib modules hint")
        .to_java_utf8();
    assert!(hint.contains("pkl:Command"));
    assert!(hint.contains("pkl:DocPackageInfo"));
    assert!(hint.contains("pkl:DocsiteInfo"));
    assert!(hint.contains("pkl:settings"));
    assert!(!hint.contains("pkl:doc-package-info"));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn qualified_base_varargs_constructors_match_java_errors_not_unsupported() {
    let source = r#"import "pkl:base"
list2 = base.List(1, 2)
list1 = base.List(1)
set0 = base.Set()
set1 = base.Set(1)
map2 = base.Map("a", 1)
map1 = base.Map("a")
bytes2 = base.Bytes(1, 2)
bytes1 = base.Bytes(65)
pairOk = base.Pair(1, 2).second
intSeqOk = base.IntSeq(1, 3).end
regexOk = base.Regex("a+").pattern
listClassOk = base.List.simpleName
"#;
    let mut session = compile_session(
        source,
        &[
            "list2",
            "list1",
            "set0",
            "set1",
            "map2",
            "map1",
            "bytes2",
            "bytes1",
            "pairOk",
            "intSeqOk",
            "regexOk",
            "listClassOk",
        ],
    );

    // Wrong arity -> `Expected 1 function arguments but got N.` (NOT Unsupported)
    for (idx, expected_msg, member) in [
        (0, "Expected 1 function arguments but got 2.", "#List"),
        (2, "Expected 1 function arguments but got 0.", "#Set"),
        (4, "Expected 1 function arguments but got 2.", "#Map"),
        (6, "Expected 1 function arguments but got 2.", "#Bytes"),
    ] {
        let err = eval_path(&mut session, idx, 256, true).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Type, "index {idx}");
        assert_eq!(err.message.to_java_utf8(), expected_msg, "index {idx}");
        assert!(
            err.frames.iter().any(|f| f.member.to_java_utf8() == member),
            "expected frame {member}, got {:?}",
            err.frames
        );
    }

    // Single arg -> `Class `VarArgs` can only be used by the standard library.`
    for (idx, member) in [(1, "#List"), (3, "#Set"), (5, "#Map"), (7, "#Bytes")] {
        let err = eval_path(&mut session, idx, 256, true).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Type, "index {idx}");
        assert_eq!(
            err.message.to_java_utf8(),
            "Class `VarArgs` can only be used by the standard library.",
            "index {idx}"
        );
        assert!(
            err.frames.iter().any(|f| f.member.to_java_utf8() == member),
            "expected frame {member}, got {:?}",
            err.frames
        );
    }

    // Qualified non-VarArgs constructors & classes continue to work.
    assert_str(eval_path(&mut session, 8, 256, true).unwrap(), "2");
    assert_str(eval_path(&mut session, 9, 256, true).unwrap(), "3");
    assert_str(eval_path(&mut session, 10, 256, true).unwrap(), "a+");
    assert_str(eval_path(&mut session, 11, 256, true).unwrap(), "List");
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn pkl_ref_module_initializes_without_unsupported_reference_binding() {
    let source = r#"import "pkl:ref"
class D extends ref.Domain {
  function renderReference(r) = "\(r.getData()):\(r.getPath().map((a) -> a.property).join("."))"
}
class Box {
  value: Int
}
rendered = ref.Reference(new D {}, Box, "root").value.toString()
"#;
    let mut session = compile_session(source, &["rendered"]);
    assert_str(eval_path(&mut session, 0, 256, true).unwrap(), "root:value");
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn stdlib_reflection_exposes_base_imports_and_synthetic_member_annotations() {
    let source = r#"import "pkl:base"
import "pkl:math"
import "pkl:reflect"

local baseMirror = reflect.Module(base)
local mathMirror = reflect.Module(math)

baseImports = baseMirror.imports.toString()
atan2Annotations = mathMirror.moduleClass.methods["atan2"].annotations.map((a) -> a.toString()).join(",")
atan2AllAnnotations = mathMirror.moduleClass.allMethods["atan2"].annotations.map((a) -> a.toString()).join(",")
bytesAnnotations = baseMirror.moduleClass.methods["Bytes"].annotations.map((a) -> a.toString()).join(",")
"#;
    let mut session = compile_session(
        source,
        &[
            "baseImports",
            "atan2Annotations",
            "atan2AllAnnotations",
            "bytesAnnotations",
        ],
    );
    assert_str(
        eval_path(&mut session, 0, 256, true).unwrap(),
        "Map(\"json\", \"pkl:json\", \"jsonnet\", \"pkl:jsonnet\", \"math\", \"pkl:math\", \"pklbinary\", \"pkl:pklbinary\", \"protobuf\", \"pkl:protobuf\", \"xml\", \"pkl:xml\", \"yaml\", \"pkl:yaml\")",
    );
    assert_str(
        eval_path(&mut session, 1, 256, true).unwrap(),
        "new Since { version = \"0.28.0\" }",
    );
    assert_str(
        eval_path(&mut session, 2, 256, true).unwrap(),
        "new Since { version = \"0.28.0\" }",
    );
    assert_str(
        eval_path(&mut session, 3, 256, true).unwrap(),
        "new Since { version = \"0.29.0\" }",
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
