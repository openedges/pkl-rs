//! Parity regression tests for M3 renderers, output formats, directives, and scalar conversion hints.
//! Expected bytes and diagnostics are verified against pinned Pkl 0.32.0 (Temurin 21.0.13.1+2-LTS).

use pkl_runtime::{
    evaluation::{
        frontend::{
            DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
        },
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    java_exception::provenance::ForeignOperation,
    modules::default_completion,
    resources::ReadResult,
    value::ErrorKind,
};

fn eval_output(
    source: &str,
    uri: &str,
    inferred_name: &str,
    options: ModuleEvalOptions,
) -> Vec<u8> {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri,
            inferred_name,
        },
        &options,
    );
    assert_eq!(
        result.exit_code,
        0,
        "unexpected error: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stderr, b"");
    result.stdout
}

fn eval_output_err(source: &str, options: ModuleEvalOptions) -> String {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///m3-renderers/main.pkl",
            inferred_name: "main",
        },
        &options,
    );
    assert_ne!(
        result.exit_code,
        0,
        "expected failure, got stdout: {:?}",
        String::from_utf8_lossy(&result.stdout)
    );
    String::from_utf8(result.stderr).expect("UTF-8 diagnostic")
}

fn eval_expr_err(source: &str, expr: &str) -> pkl_runtime::value::RuntimeError {
    let mut session = SourceSession::compile(
        source,
        "file:///m3-renderers/main.pkl",
        "main",
        &[expr.to_owned()],
        SourceSessionOptions::default(),
    )
    .unwrap();
    session.start_path(0, 0, None).unwrap();
    let error = loop {
        match session.poll(0, 100) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            HostPoll::Module { token, request } => {
                session
                    .complete_module(&token, default_completion(&request).unwrap(), 0)
                    .unwrap();
            }
            HostPoll::Read { token, .. } => {
                session
                    .complete_read(&token, Ok(ReadResult::Missing), 0)
                    .unwrap();
            }
            HostPoll::Ready(Err(error)) => break error,
            HostPoll::Ready(Ok(value)) => panic!("expected error, got {value:?}"),
            other => panic!("unexpected poll event: {other:?}"),
        }
    };
    session.close();
    session.collect(&[]);
    assert_eq!(session.heap_stats().slots, 0);
    error
}

fn eval_expr_string(source: &str, expr: &str) -> String {
    let mut session = SourceSession::compile(
        source,
        "file:///m3-renderers/main.pkl",
        "main",
        &[expr.to_owned()],
        SourceSessionOptions::default(),
    )
    .unwrap();
    session.start_path(0, 0, None).unwrap();
    let value = loop {
        match session.poll(0, 100) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            HostPoll::Module { token, request } => {
                session
                    .complete_module(&token, default_completion(&request).unwrap(), 0)
                    .unwrap();
            }
            HostPoll::Read { token, .. } => {
                session
                    .complete_read(&token, Ok(ReadResult::Missing), 0)
                    .unwrap();
            }
            HostPoll::Ready(Ok(value)) => break value,
            HostPoll::Ready(Err(error)) => panic!("unexpected error: {error:?}"),
            other => panic!("unexpected poll event: {other:?}"),
        }
    };
    session.close();
    session.collect(&[]);
    assert_eq!(session.heap_stats().slots, 0);
    match value {
        pkl_runtime::value::Value::String(s) => s.to_java_utf8(),
        other => panic!("expected String, got {other:?}"),
    }
}

fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn module_output_renderer_supports_all_nine_output_formats() {
    let source = "name = \"Pigeon\"\nage = 42\n";
    let uri = "file:///tmp/probe_formats.pkl";
    let inferred = "probe_formats";

    let text_cases: [(&str, &str); 8] = [
        ("pcf", "name = \"Pigeon\"\nage = 42\n"),
        ("json", "{\n  \"name\": \"Pigeon\",\n  \"age\": 42\n}\n"),
        ("jsonnet", "{\n  name: 'Pigeon',\n  age: 42,\n}\n"),
        (
            "plist",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>name</key>\n  <string>Pigeon</string>\n  <key>age</key>\n  <integer>42</integer>\n</dict>\n</plist>\n",
        ),
        ("properties", "name = Pigeon\nage = 42\n"),
        ("textproto", "name: \"Pigeon\"\nage: 42\n"),
        (
            "xml",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>\n  <name>Pigeon</name>\n  <age>42</age>\n</root>\n",
        ),
        ("yaml", "name: Pigeon\nage: 42\n"),
    ];
    for (format, expected) in text_cases {
        let options = ModuleEvalOptions {
            properties: vec![("pkl.outputFormat".into(), format.into())],
            ..Default::default()
        };
        assert_eq!(
            eval_output(source, uri, inferred, options),
            expected.as_bytes(),
            "format={format}"
        );
    }

    // Exact msgpack bytes captured from pinned Java 0.32.0 `pkl eval -f pkl-binary`.
    let expected_binary = decode_hex(
        "9401ad70726f62655f666f726d617473bd66696c653a2f2f2f746d702f70726f62655f666f726d6174732e706b6c929310a46e616d65a6506967656f6e9310a36167652a",
    );
    let binary_options = ModuleEvalOptions {
        properties: vec![("pkl.outputFormat".into(), "pkl-binary".into())],
        ..Default::default()
    };
    assert_eq!(
        eval_output(source, uri, inferred, binary_options),
        expected_binary
    );

    // Unknown output format throws exact base.pkl message (not Unsupported).
    let bogus = eval_output_err(
        source,
        ModuleEvalOptions {
            properties: vec![("pkl.outputFormat".into(), "bogus".into())],
            diagnostic_profile: DiagnosticProfile::CliBundledSettings,
            ..Default::default()
        },
    );
    assert!(
        bogus.contains("Unknown output format: `bogus`. Supported formats are `json`, `jsonnet`, `pcf`, `plist`, `properties`, `textproto`, `xml`, `yaml`, `pkl-binary`."),
        "unexpected error:\n{bogus}"
    );
    assert!(
        bogus.contains("at pkl.base#Module.output.renderer"),
        "missing renderer frame:\n{bogus}"
    );
}

#[test]
fn value_and_bytes_renderer_abstract_methods_report_abstract_invocation() {
    let val_err = eval_expr_err(
        "class MyR extends ValueRenderer {}\nr = new MyR {}.renderValue(1)\n",
        "r",
    );
    assert_eq!(val_err.kind, ErrorKind::Type);
    assert_eq!(
        val_err.message,
        "Cannot invoke abstract method `pkl.base#ValueRenderer.renderValue`."
    );

    let val_doc_err = eval_expr_err(
        "class MyR extends ValueRenderer {}\nr = new MyR {}.renderDocument(1)\n",
        "r",
    );
    assert_eq!(val_doc_err.kind, ErrorKind::Type);
    assert_eq!(
        val_doc_err.message,
        "Cannot invoke abstract method `pkl.base#ValueRenderer.renderDocument`."
    );

    let bytes_doc_err = eval_expr_err(
        "class MyB extends BytesRenderer {}\nr = new MyB {}.renderDocument(1)\n",
        "r",
    );
    assert_eq!(bytes_doc_err.kind, ErrorKind::Type);
    assert_eq!(
        bytes_doc_err.message,
        "Cannot invoke abstract method `pkl.base#BytesRenderer.renderDocument`."
    );

    let bytes_val_err = eval_expr_err(
        "class MyB extends BytesRenderer {}\nr = new MyB {}.renderValue(1)\n",
        "r",
    );
    assert_eq!(bytes_val_err.kind, ErrorKind::Type);
    assert_eq!(
        bytes_val_err.message,
        "Cannot invoke abstract method `pkl.base#BytesRenderer.renderValue`."
    );
}

#[test]
fn module_output_files_amendment_uses_mapping_default_and_file_output_entries() {
    let source = r#"
output {
  files {
    ["a.json"] {
      value = new Dynamic { k = 1 }
      renderer = new JsonRenderer { indent = "" }
    }
  }
}
text = output.files["a.json"].text
"#;
    assert_eq!(eval_expr_string(source, "text"), "{\"k\":1}\n");
}

#[test]
fn implicit_listing_join_works_in_constraints_and_body() {
    let source = r#"
x: Listing<String>(join(",") == "a,b") = new { "a"; "b" }
y: List<String>(join("-") == "p-q") = List("p", "q")
z: Set<String>(join(":") == "m:n") = Set("m", "n")
res = "\(x.join(","))|\(y.join("-"))|\(z.join(":"))"
"#;
    assert_eq!(eval_expr_string(source, "res"), "a,b|p-q|m:n");
}

#[test]
fn pcf_render_directive_matches_pinned_java_output() {
    let source = r#"
class Person { name: String }
p = new Person { name = "Pigeon" }
propCase = new PcfRenderDirective {
  before = "/* before */ "
  value = p
  after = " /* after */"
}
listCase = List(new PcfRenderDirective { before = "A"; value = p; after = "B" })
listingCase = new Listing {
  new PcfRenderDirective { before = "A"; value = p; after = "B" }
  new RenderDirective { text = "RAW" }
}
scalarCase = new PcfRenderDirective { before = "("; value = 42; after = ")" }
nullBeforeAfter = new PcfRenderDirective { value = "hi" }
listingStr = new Listing {
  new PcfRenderDirective {
    before = "/*s*/"
    value = "line1\nline2"
    after = "/*/s*/"
  }
}
mapCase = Map(
  new PcfRenderDirective { before = "K"; value = "a"; after = "E" },
  new PcfRenderDirective { before = "V"; value = 1; after = "W" }
)
nestedCase = new PcfRenderDirective {
  before = "["
  value = new PcfRenderDirective {
    before = "("
    value = 7
    after = ")"
  }
  after = "]"
}
docCase = new PcfRenderer {}.renderDocument(new PcfRenderDirective { before = "// header\n"; value = p; after = "\n// footer" })
valCase = new PcfRenderer {}.renderValue(new PcfRenderDirective { before = "/*v*/"; value = p; after = "/*/v*/" })
convCase = (new PcfRenderer {
  converters {
    [Person] = (v) -> new PcfRenderDirective { before = "new Person "; value = v; after = " // end" }
  }
}).renderDocument(new Dynamic { person = p; items = new Listing { p } })
jsonCase = new JsonRenderer {}.renderValue(new PcfRenderDirective { before = "A"; value = 1; after = "B" })
"#;
    let expected = concat!(
        "p {\n  name = \"Pigeon\"\n}\n",
        "propCase /* before */ {\n  name = \"Pigeon\"\n} /* after */\n",
        "listCase = List(new A{\n  name = \"Pigeon\"\n}B)\n",
        "listingCase {\n  new A{\n    name = \"Pigeon\"\n  }B\n  RAW\n}\n",
        "scalarCase (42)\n",
        "nullBeforeAfter \"hi\"\n",
        "listingStr {\n  new /*s*/\"\"\"\n    line1\n    line2\n    \"\"\"/*/s*/\n}\n",
        "mapCase = Map(new K\"a\"E, new V1W)\n",
        "nestedCase [(7)]\n",
        "docCase = \"\"\"\n  // header\n  {\n    name = \"Pigeon\"\n  }\n  // footer\n  \n  \"\"\"\n",
        "valCase = \"\"\"\n  /*v*/{\n    name = \"Pigeon\"\n  }/*/v*/\n  \"\"\"\n",
        "convCase = \"\"\"\n  person new Person {\n    name = \"Pigeon\"\n  } // end\n  items {\n    new new Person {\n      name = \"Pigeon\"\n    } // end\n  }\n  \n  \"\"\"\n",
        "jsonCase = \"\"\"\n  {\n    \"before\": \"A\",\n    \"value\": 1,\n    \"after\": \"B\"\n  }\n  \"\"\"\n",
    );
    assert_eq!(
        eval_output(
            source,
            "file:///pcf-directive.pkl",
            "pcf_directive",
            Default::default()
        ),
        expected.as_bytes()
    );

    // Undefined PcfRenderDirective.value fills in the rendering path hint `a.b.value`.
    let err = eval_output_err(
        "a {\n  b = new PcfRenderDirective {\n    before = \"X\"\n  }\n}\n",
        Default::default(),
    );
    assert!(
        err.contains("Tried to read property `value` but its value is undefined."),
        "{err}"
    );
    assert!(
        err.contains("when rendering path `a.b.value` of module"),
        "{err}"
    );

    // Cyclic PcfRenderDirective terminates cleanly as Cycle.
    let cycle = eval_expr_err(
        "d = new PcfRenderDirective { value = d }\nr = new PcfRenderer {}.renderValue(d)\n",
        "r",
    );
    assert_eq!(cycle.kind, ErrorKind::Cycle);
}

#[test]
fn render_document_scalar_conversion_hints_match_java_boxing_semantics() {
    // Uncached Float (NaN) and uncached Int (1000) re-box on identity lambda -> hint emitted.
    let nan_id = eval_expr_err(
        "x = new JsonRenderer { converters { [Float] = (v) -> v } }.renderDocument(NaN)\n",
        "x",
    );
    assert_eq!(nan_id.kind, ErrorKind::Type);
    assert_eq!(
        nan_id.hint,
        Some("This value was converted during rendering. Previous: NaN. After: NaN.".into())
    );

    let int_1000 = eval_expr_err(
        "x = new PcfRenderer { converters { [Int] = (v) -> v } }.renderDocument(1000)\n",
        "x",
    );
    assert_eq!(int_1000.kind, ErrorKind::Type);
    assert_eq!(
        int_1000.hint,
        Some("This value was converted during rendering. Previous: 1000. After: 1000.".into())
    );

    // Cached Int (5 in -128..=127) retains LongCache identity -> no conversion hint.
    let int_5 = eval_expr_err(
        "x = new PcfRenderer { converters { [Int] = (v) -> v + 0 } }.renderDocument(5)\n",
        "x",
    );
    assert_eq!(int_5.kind, ErrorKind::Type);
    assert_eq!(int_5.hint, None);

    // Implicit VmNull.withDefault (`n: Foo?`) preserves identity on `(v) -> v` but differs from `null`.
    let null_same = eval_expr_err(
        "class Foo { a: Int = 1 }\nn: Foo?\nx = new PcfRenderer { converters { [Null] = (v) -> v } }.renderDocument(n)\n",
        "x",
    );
    assert_eq!(null_same.kind, ErrorKind::Type);
    assert_eq!(null_same.hint, None);

    let null_diff = eval_expr_err(
        "class Foo { a: Int = 1 }\nn: Foo?\nx = new PcfRenderer { converters { [Null] = (v) -> null } }.renderDocument(n)\n",
        "x",
    );
    assert_eq!(null_diff.kind, ErrorKind::Type);
    assert_eq!(
        null_diff.hint,
        Some("This value was converted during rendering. Previous: null. After: null.".into())
    );
}

#[test]
fn xml_and_pklbinary_foreign_map_entry_use_unified_foreign_projection() {
    for (module, renderer) in [
        ("pkl:xml", "xml.Renderer"),
        ("pkl:pklbinary", "pklbinary.Renderer"),
    ] {
        let source = format!(
            "import \"{module}\"\ne = List(Map(\"a\", 1)).flatten().first\nx = new {renderer} {{ converters {{ [Int] = (_) -> e }} }}.renderValue(1)\n"
        );
        let err = eval_expr_err(&source, "x");
        assert_eq!(err.kind, ErrorKind::Unsupported);
        let prov = err
            .foreign_exception
            .as_deref()
            .expect("foreign KeyValueHolder provenance");
        assert_eq!(
            prov.operation,
            ForeignOperation::KeyValueHolderTypeVisitor { is_set: false }
        );
        assert_eq!(
            prov.message,
            "Unknown VM value type: java.util.KeyValueHolder"
        );
    }
}

#[test]
fn pure_member_render_fast_path_indexes_captured_locals_by_de_bruijn_depth() {
    let source = r#"
const function makeMapping(path: String, message: String, severity: String): Mapping =
  new Mapping {
    ["path"] = path
    ["message"] = message
    ["severity"] = severity
  }

const function makeDynamic(a: String, b: String, c: String): Dynamic =
  new Dynamic {
    first = a
    second = b
    third = c
  }

const function makeListing(a: String, b: String, c: String): Listing =
  new Listing { a; b; c }

const function makeGeneratorMapping(prefix: String, suffix: String, keys: List<String>): Mapping =
  new Mapping {
    for (k in keys) {
      [k] = "\(prefix)-\(k)-\(suffix)"
    }
  }

mapping = makeMapping("plain", "hello", "error")
dynObj = makeDynamic("one", "two", "three")
listing = makeListing("x", "y", "z")
generated = makeGeneratorMapping("pre", "suf", List("k1", "k2"))
"#;

    let json_bytes = eval_output(
        &format!("{source}\noutput {{ renderer = new JsonRenderer {{}} }}\n"),
        "file:///m3-renderers/captured-locals.pkl",
        "captured-locals",
        ModuleEvalOptions::default(),
    );
    let json = String::from_utf8(json_bytes).expect("UTF-8 JSON");
    assert_eq!(
        json,
        "{\n  \"mapping\": {\n    \"path\": \"plain\",\n    \"message\": \"hello\",\n    \"severity\": \"error\"\n  },\n  \"dynObj\": {\n    \"first\": \"one\",\n    \"second\": \"two\",\n    \"third\": \"three\"\n  },\n  \"listing\": [\n    \"x\",\n    \"y\",\n    \"z\"\n  ],\n  \"generated\": {\n    \"k1\": \"pre-k1-suf\",\n    \"k2\": \"pre-k2-suf\"\n  }\n}\n"
    );

    let yaml_bytes = eval_output(
        &format!("{source}\noutput {{ renderer = new YamlRenderer {{}} }}\n"),
        "file:///m3-renderers/captured-locals.pkl",
        "captured-locals",
        ModuleEvalOptions::default(),
    );
    let yaml = String::from_utf8(yaml_bytes).expect("UTF-8 YAML");
    assert_eq!(
        yaml,
        "mapping:\n  path: plain\n  message: hello\n  severity: error\ndynObj:\n  first: one\n  second: two\n  third: three\nlisting:\n- x\n- 'y'\n- z\ngenerated:\n  k1: pre-k1-suf\n  k2: pre-k2-suf\n"
    );
}

#[test]
fn plain_typed_fast_render_preserves_xml_wrappers_and_empty_properties_prefix() {
    let expected = concat!(
        "json = \"\"\"\n",
        "  {\n",
        "    \"c\": {\n",
        "      \"text\": \"hello\",\n",
        "      \"isBlockFormat\": true\n",
        "    },\n",
        "    \"d\": {\n",
        "      \"text\": \"raw\"\n",
        "    },\n",
        "    \"i\": {\n",
        "      \"value\": {\n",
        "        \"a\": 1\n",
        "      }\n",
        "    },\n",
        "    \"item\": {\n",
        "      \"name\": \"widget\",\n",
        "      \"count\": 3\n",
        "    }\n",
        "  }\n",
        "  \n",
        "  \"\"\"\n",
        "pcf = \"\"\"\n",
        "  name = \"widget\"\n",
        "  count = 3\n",
        "  \n",
        "  \"\"\"\n",
        "yaml = \"\"\"\n",
        "  name: widget\n",
        "  count: 3\n",
        "  \n",
        "  \"\"\"\n",
        "xmlOut = \"\"\"\n",
        "  <?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "  <root>\n",
        "    <c>\n",
        "      <!--hello-->\n",
        "    </c>\n",
        "    <d><![CDATA[raw]]></d>\n",
        "    <a>1</a>\n",
        "    <item>\n",
        "      <name>widget</name>\n",
        "      <count>3</count>\n",
        "    </item>\n",
        "  </root>\n",
        "  \n",
        "  \"\"\"\n",
        "props = \"\"\"\n",
        "  .name = widget\n",
        "  .count = 3\n",
        "  \n",
        "  \"\"\"\n",
    );

    let body = r#"
class Item {
  name: String
  count: Int
  hidden secret: String = "shh"
}
local box = new Dynamic {
  c = xml.Comment("hello")
  d = xml.CData("raw")
  i = xml.Inline(new Dynamic { a = 1 })
  item = new Item { name = "widget"; count = 3 }
}
json = new JsonRenderer {}.renderDocument(box)
pcf = new PcfRenderer {}.renderDocument(new Item { name = "widget"; count = 3 })
yaml = new YamlRenderer {}.renderDocument(new Item { name = "widget"; count = 3 })
xmlOut = new xml.Renderer {}.renderDocument(box)
props = new PropertiesRenderer {}.renderDocument(new Mapping { [""] = new Item { name = "widget"; count = 3 } })
"#;

    for (tag, header) in [
        ("without-jsonnet", "import \"pkl:xml\"\n"),
        (
            "with-jsonnet",
            "import \"pkl:xml\"\nimport \"pkl:jsonnet\"\n",
        ),
    ] {
        let source = format!("{header}{body}");
        let out = eval_output(
            &source,
            &format!("file:///m3-renderers/plain-typed-{tag}.pkl"),
            "plain_typed",
            ModuleEvalOptions::default(),
        );
        assert_eq!(
            String::from_utf8(out).expect("UTF-8 PCF"),
            expected,
            "mismatch for {tag}"
        );
    }
}

#[test]
fn deep_nested_pair_renders_without_unbounded_recursion_in_pcf_and_pkl_binary() {
    let depth = 3000usize;
    let source =
        format!("p = IntSeq(1, {depth}).toList().fold(Pair(0, 0), (acc, i) -> Pair(i, acc))\n");

    // Expected PCF matches pinned Apple Pkl 0.32.0 (`34,908` bytes for depth=3000).
    let mut expected_pcf = String::from("p = ");
    for i in (1..=depth).rev() {
        expected_pcf.push_str(&format!("Pair({i}, "));
    }
    expected_pcf.push_str("Pair(0, 0)");
    for _ in 0..depth {
        expected_pcf.push(')');
    }
    expected_pcf.push('\n');
    assert_eq!(expected_pcf.len(), 34_908);

    let pcf_out = eval_output(
        &source,
        "file:///m3-renderers/deep-pair.pkl",
        "deep_pair",
        ModuleEvalOptions::default(),
    );
    assert_eq!(String::from_utf8(pcf_out).expect("UTF-8 PCF"), expected_pcf);

    // Also exercise `try_fast_encode_binary_value` bailout to iterative `RenderRun::tasks`.
    let bin_out = eval_output(
        &source,
        "repl:text",
        "text",
        ModuleEvalOptions {
            properties: vec![("pkl.outputFormat".into(), "pkl-binary".into())],
            ..ModuleEvalOptions::default()
        },
    );
    // Matches pinned Apple Pkl 0.32.0 (`pkl-macos-aarch64 eval -f pkl-binary -`: 14,644 bytes).
    assert_eq!(bin_out.len(), 14_644);
}

#[test]
fn user_module_pkl_base_classes_do_not_stop_forcing_or_hidden_checks() {
    // Oracle 0.32.0 permits user `module pkl.base` defining `open class Object`/`Typed`/`Any`.
    let render_src = concat!(
        "module pkl.base\n",
        "open class Object { hidden secret = 42; visible = 99 }\n",
        "class Sub extends Object { secret = 77; x = 1 }\n",
        "res = new Sub {}\n",
        "res2 = new Sub { x = 2 }\n",
    );
    let pcf_out = eval_output(
        render_src,
        "file:///m3-renderers/user-pkl-base-render.pkl",
        "pkl.base",
        ModuleEvalOptions::default(),
    );
    assert_eq!(
        String::from_utf8(pcf_out).expect("UTF-8 PCF"),
        "res {\n  visible = 99\n  x = 1\n}\nres2 {\n  visible = 99\n  x = 2\n}\n"
    );
    let json_out = eval_output(
        render_src,
        "file:///m3-renderers/user-pkl-base-render.pkl",
        "pkl.base",
        ModuleEvalOptions {
            properties: vec![("pkl.outputFormat".into(), "json".into())],
            ..ModuleEvalOptions::default()
        },
    );
    assert_eq!(
        String::from_utf8(json_out).expect("UTF-8 JSON"),
        "{\n  \"res\": {\n    \"visible\": 99,\n    \"x\": 1\n  },\n  \"res2\": {\n    \"visible\": 99,\n    \"x\": 2\n  }\n}\n"
    );

    let eq_src = concat!(
        "module pkl.base\n",
        "open class Object { hidden h: Int; visible = h + 1 }\n",
        "class Sub extends Object { x = 1 }\n",
        "local a = new Sub { h = 1 }\n",
        "local b = new Sub { h = 2 }\n",
        "eq = a == b\n",
    );
    let eq_out = eval_output(
        eq_src,
        "file:///m3-renderers/user-pkl-base-eq.pkl",
        "pkl.base",
        ModuleEvalOptions::default(),
    );
    assert_eq!(
        String::from_utf8(eq_out).expect("UTF-8 PCF"),
        "eq = false\n"
    );

    let hidden_src = concat!(
        "module pkl.base\n",
        "open class Object { hidden secret = 42 }\n",
        "class Sub extends Object { secret = throw(\"must not force hidden\"); x = 1 }\n",
        "res = new Sub {}\n",
    );
    let hidden_out = eval_output(
        hidden_src,
        "file:///m3-renderers/user-pkl-base-hidden.pkl",
        "pkl.base",
        ModuleEvalOptions::default(),
    );
    assert_eq!(
        String::from_utf8(hidden_out).expect("UTF-8 PCF"),
        "res {\n  x = 1\n}\n"
    );
}

#[test]
fn plain_object_sync_force_handles_precached_object_key_rehash() {
    // Both 4 scalar properties (previously infinite loop) and 5 scalar properties
    // (previously `economic_map.rs:156` panic) with a pre-cached object entry key.
    for (tag, extra, extra_line, extra_json) in [
        ("four", "", "", ""),
        ("five", "e = 5; ", "    e = 5\n", "  \"e\": 5,\n"),
    ] {
        let pcf_src = format!(
            "local kobj = new Dynamic {{ k = 1 }}\n\
             local o = new Dynamic {{ a = 1; b = 2; c = 3; d = 4; {extra}[kobj] = 10 }}\n\
             res = if (o[kobj] == 10) new PcfRenderer {{}}.renderValue(o) else \"nope\"\n\
             custom = if (o[kobj] == 10) new PcfRenderer {{ indent = \"    \" }}.renderValue(o) else \"nope\"\n"
        );
        let pcf_out = eval_output(
            &pcf_src,
            &format!("file:///m3-renderers/rehash-{tag}.pkl"),
            "rehash",
            ModuleEvalOptions::default(),
        );
        let expected_pcf = format!(
            "res = \"\"\"\n  {{\n    a = 1\n    b = 2\n    c = 3\n    d = 4\n{extra_line}    [new {{\n      k = 1\n    }}] = 10\n  }}\n  \"\"\"\n\
             custom = \"\"\"\n  {{\n      a = 1\n      b = 2\n      c = 3\n      d = 4\n{custom_extra}      [new {{\n          k = 1\n      }}] = 10\n  }}\n  \"\"\"\n",
            custom_extra = if extra.is_empty() {
                ""
            } else {
                "      e = 5\n"
            }
        );
        assert_eq!(
            String::from_utf8(pcf_out).expect("UTF-8 PCF"),
            expected_pcf,
            "PCF mismatch for {tag}"
        );

        // Nested inside a Typed object (exercises `try_force_plain_typed_sync` -> `try_force_plain_object_sync`)
        // and Mapping container.
        let nested_src = format!(
            "class Box {{ inner: Dynamic }}\n\
             local kobj = new Dynamic {{ k = 1 }}\n\
             local o = new Dynamic {{ a = 1; b = 2; c = 3; d = 4; {extra}[kobj] = 10 }}\n\
             local m = new Mapping {{ [\"a\"] = 1; [\"b\"] = 2; [\"c\"] = 3; [\"d\"] = 4; {map_extra}[kobj] = 10 }}\n\
             boxed = if (o[kobj] == 10) new Box {{ inner = o }} else throw(\"bad\")\n\
             mapped = if (m[kobj] == 10) new PcfRenderer {{}}.renderValue(m) else \"nope\"\n",
            map_extra = if extra.is_empty() {
                ""
            } else {
                "[\"e\"] = 5; "
            }
        );
        let nested_out = eval_output(
            &nested_src,
            &format!("file:///m3-renderers/rehash-nested-{tag}.pkl"),
            "rehash_nested",
            ModuleEvalOptions::default(),
        );
        let nested_text = String::from_utf8(nested_out).expect("UTF-8 PCF");
        assert!(
            nested_text.contains("boxed {\n  inner {\n    a = 1\n"),
            "{nested_text}"
        );
        assert!(nested_text.contains("mapped = \"\"\""), "{nested_text}");
        let _ = extra_json;
    }
}

#[test]
fn plain_typed_sync_force_preserves_superclass_property_when_subclass_declares_hidden() {
    let annot_src = concat!(
        "import \"pkl:reflect\"\n",
        "open class Base extends Annotation { x = 1; y = 2 }\n",
        "class Sub extends Base { hidden x = 99 }\n",
        "@Sub { y = 3 }\n",
        "hidden dummy = 0\n",
        "res = reflect.Module(module).moduleClass.properties[\"dummy\"].annotations[0]\n",
    );
    let annot_pcf = eval_output(
        annot_src,
        "file:///m3-renderers/hidden-sub-annot.pkl",
        "hidden_sub_annot",
        ModuleEvalOptions::default(),
    );
    assert_eq!(
        String::from_utf8(annot_pcf).expect("UTF-8 PCF"),
        "res {\n  x = 1\n  y = 3\n}\n"
    );
    let annot_json = eval_output(
        annot_src,
        "file:///m3-renderers/hidden-sub-annot.pkl",
        "hidden_sub_annot",
        ModuleEvalOptions {
            properties: vec![("pkl.outputFormat".into(), "json".into())],
            ..ModuleEvalOptions::default()
        },
    );
    assert_eq!(
        String::from_utf8(annot_json).expect("UTF-8 JSON"),
        "{\n  \"res\": {\n    \"x\": 1,\n    \"y\": 3\n  }\n}\n"
    );

    // Three-level case: amending a subclass with a hidden override still renders Base.x.
    let three_level_src = concat!(
        "open class Base { x = 1; y = 2 }\n",
        "class Sub extends Base { hidden x = 99 }\n",
        "res = new Sub { x = throw(\"must not force hidden\"); y = 4 }\n",
    );
    let three_out = eval_output(
        three_level_src,
        "file:///m3-renderers/hidden-sub-3level.pkl",
        "hidden_sub_3level",
        ModuleEvalOptions::default(),
    );
    assert_eq!(
        String::from_utf8(three_out).expect("UTF-8 PCF"),
        "res {\n  x = 1\n  y = 4\n}\n"
    );
}
