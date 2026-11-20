//! Regression tests for `pkl:ref` module dispatch, `Reference` builtin properties,
//! and path converter wildcard parity against pinned Pkl 0.32.0.

use pkl_runtime::{
    evaluation::session::{SourceSession, SourceSessionOptions},
    host::HostPoll,
    modules::default_completion,
    resources::ReadResult,
    value::{ErrorKind, Value},
};

fn eval_expr_string(source: &str, expr: &str) -> String {
    let mut session = SourceSession::compile(
        source,
        "file:///m3-ref-conv/main.pkl",
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
        Value::String(s) => s.to_java_utf8(),
        other => panic!("expected String, got {other:?}"),
    }
}

fn eval_expr_err(source: &str, expr: &str) -> pkl_runtime::value::RuntimeError {
    let mut session = SourceSession::compile(
        source,
        "file:///m3-ref-conv/main.pkl",
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

#[test]
fn pkl_ref_module_inherits_module_typed_any_members_and_reports_missing_members() {
    let source = r#"
import "pkl:ref" as r
import "pkl:test"

res = new Listing {
  "toString=" + r.toString()
  "getClass=" + r.getClass().toString()
  "toMap=" + r.toMap().toString()
  "hasPropOutput=" + r.hasProperty("output").toString()
  "hasPropBogus=" + r.hasProperty("bogus").toString()
  "getPropOrNull=" + (r.getPropertyOrNull("bogus") ?? "NULL").toString()
  "outputText=" + r.output.text
}.join("\n")
missingProp = r.bogus
missingMethod = r.bogus()
"#;
    let expected = concat!(
        "toString=new ModuleClass {}\n",
        "getClass=pkl.ref\n",
        "toMap=Map()\n",
        "hasPropOutput=true\n",
        "hasPropBogus=false\n",
        "getPropOrNull=NULL\n",
        "outputText=",
    );
    assert_eq!(eval_expr_string(source, "res"), expected);

    let prop_err = eval_expr_err(source, "missingProp");
    assert_eq!(prop_err.kind, ErrorKind::MissingMember);
    assert!(
        prop_err.message.contains("Cannot find property `bogus`"),
        "{prop_err:?}"
    );

    let method_err = eval_expr_err(source, "missingMethod");
    assert_eq!(method_err.kind, ErrorKind::MissingMember);
    assert!(
        method_err.message.contains("Cannot find method `bogus`"),
        "{method_err:?}"
    );
}

#[test]
fn reference_supports_all_base_pkl_class_properties_matching_java_032() {
    let source = r#"
import "pkl:ref"
import "pkl:test"

class MyDomain extends ref.Domain {
  function renderReference(r) = "ok"
}
class Holder {
  s: String
  i: Int
  f: Float
  n: Number
  l: List<Int>
  st: Set<Int>
  mp: Map<String, Int>
  c: Collection<Int>
  li: Listing<Int>
  ma: Mapping<String, Int>
  p: Pair<String, Int>
  du: Duration
  ds: DataSize
  re: Regex
  sq: IntSeq
  b: Bytes
  cl: Class<String>
  res: Resource
  fo: FileOutput
  mo: ModuleOutput
  rd: RenderDirective
  prd: PcfRenderDirective
  acc: ref.Access
}
d = new MyDomain {}
h = ref.Reference(d, Holder, "h")

function typeOfRef(f: () -> Any): String =
  let (err = test.catchOrNull(() -> f.apply().__probe_type__))
    err ?? "NOERR"

results = new Listing {
  "s.length=" + typeOfRef(() -> h.s.length)
  "s.md5=" + typeOfRef(() -> h.s.md5)
  "s.chars=" + typeOfRef(() -> h.s.chars)
  "s.codePoints=" + typeOfRef(() -> h.s.codePoints)
  "i.sign=" + typeOfRef(() -> h.i.sign)
  "i.ns=" + typeOfRef(() -> h.i.ns)
  "i.b=" + typeOfRef(() -> h.i.b)
  "i.inv=" + typeOfRef(() -> h.i.inv)
  "i.isEven=" + typeOfRef(() -> h.i.isEven)
  "f.sign=" + typeOfRef(() -> h.f.sign)
  "f.ns=" + typeOfRef(() -> h.f.ns)
  "f.isNaN=" + typeOfRef(() -> h.f.isNaN)
  "n.sign=" + typeOfRef(() -> h.n.sign)
  "n.ns=" + typeOfRef(() -> h.n.ns)
  "n.isFinite=" + typeOfRef(() -> h.n.isFinite)
  "l.length=" + typeOfRef(() -> h.l.length)
  "l.first=" + typeOfRef(() -> h.l.first)
  "l.firstOrNull=" + typeOfRef(() -> h.l.firstOrNull)
  "l.rest=" + typeOfRef(() -> h.l.rest)
  "l.distinct=" + typeOfRef(() -> h.l.distinct)
  "l.isNotEmpty=" + typeOfRef(() -> h.l.isNotEmpty)
  "st.first=" + typeOfRef(() -> h.st.first)
  "st.rest=" + typeOfRef(() -> h.st.rest)
  "st.minOrNull=" + typeOfRef(() -> h.st.minOrNull)
  "mp.keys=" + typeOfRef(() -> h.mp.keys)
  "mp.values=" + typeOfRef(() -> h.mp.values)
  "mp.entries=" + typeOfRef(() -> h.mp.entries)
  "mp.length=" + typeOfRef(() -> h.mp.length)
  "c.length=" + typeOfRef(() -> h.c.length)
  "c.first=" + typeOfRef(() -> h.c.first)
  "c.rest=" + typeOfRef(() -> h.c.rest)
  "c.firstOrNull=" + typeOfRef(() -> h.c.firstOrNull)
  "p.first=" + typeOfRef(() -> h.p.first)
  "p.second=" + typeOfRef(() -> h.p.second)
  "p.key=" + typeOfRef(() -> h.p.key)
  "p.value=" + typeOfRef(() -> h.p.value)
  "du.value=" + typeOfRef(() -> h.du.value)
  "du.unit=" + typeOfRef(() -> h.du.unit)
  "du.isoString=" + typeOfRef(() -> h.du.isoString)
  "du.isPositive=" + typeOfRef(() -> h.du.isPositive)
  "ds.value=" + typeOfRef(() -> h.ds.value)
  "ds.unit=" + typeOfRef(() -> h.ds.unit)
  "ds.isBinaryUnit=" + typeOfRef(() -> h.ds.isBinaryUnit)
  "re.pattern=" + typeOfRef(() -> h.re.pattern)
  "re.groupCount=" + typeOfRef(() -> h.re.groupCount)
  "sq.start=" + typeOfRef(() -> h.sq.start)
  "sq.end=" + typeOfRef(() -> h.sq.end)
  "sq.step=" + typeOfRef(() -> h.sq.step)
  "b.length=" + typeOfRef(() -> h.b.length)
  "b.size=" + typeOfRef(() -> h.b.size)
  "b.base64=" + typeOfRef(() -> h.b.base64)
  "b.sha256Int=" + typeOfRef(() -> h.b.sha256Int)
  "cl.simpleName=" + typeOfRef(() -> h.cl.simpleName)
  "res.uri=" + typeOfRef(() -> h.res.uri)
  "res.text=" + typeOfRef(() -> h.res.text)
  "res.bytes=" + typeOfRef(() -> h.res.bytes)
  "res.sha256Int=" + typeOfRef(() -> h.res.sha256Int)
  "fo.value=" + typeOfRef(() -> h.fo.value)
  "fo.renderer=" + typeOfRef(() -> h.fo.renderer)
  "fo.text=" + typeOfRef(() -> h.fo.text)
  "fo.bytes=" + typeOfRef(() -> h.fo.bytes)
  "mo.files=" + typeOfRef(() -> h.mo.files)
  "rd.text=" + typeOfRef(() -> h.rd.text)
  "rd.bytes=" + typeOfRef(() -> h.rd.bytes)
  "prd.before=" + typeOfRef(() -> h.prd.before)
  "prd.value=" + typeOfRef(() -> h.prd.value)
  "prd.after=" + typeOfRef(() -> h.prd.after)
  "acc.property=" + typeOfRef(() -> h.acc.property)
  "acc.key=" + typeOfRef(() -> h.acc.key)
}.join("\n")

liLen = h.li.length
liDef = h.li.default
maKeys = h.ma.keys
maDef = h.ma.default
rawLiDef = ref.Reference(d, Listing, "li").default
rawMaDef = ref.Reference(d, Mapping, "ma").default
"#;

    let actual = eval_expr_string(source, "results");
    let expected_suffixes = [
        ("s.length", "Int"),
        ("s.md5", "String"),
        ("s.chars", "List<String>"),
        ("s.codePoints", "List<Int>"),
        ("i.sign", "Int"),
        ("i.ns", "Duration"),
        ("i.b", "DataSize"),
        ("i.inv", "Int"),
        ("i.isEven", "Boolean"),
        ("f.sign", "Float"),
        ("f.ns", "Duration"),
        ("f.isNaN", "Boolean"),
        ("n.sign", "Number"),
        ("n.ns", "Duration"),
        ("n.isFinite", "Boolean"),
        ("l.length", "Int"),
        ("l.first", "Element"),
        ("l.firstOrNull", "Element | Null"),
        ("l.rest", "List<Element>"),
        ("l.distinct", "List<Element>"),
        ("l.isNotEmpty", "Boolean"),
        ("st.first", "Element"),
        ("st.rest", "Set<Element>"),
        ("st.minOrNull", "Element"),
        ("mp.keys", "Set<Key>"),
        ("mp.values", "List<Value>"),
        ("mp.entries", "List<Pair<Key, Value>>"),
        ("mp.length", "Int"),
        ("c.length", "Int"),
        ("c.first", "Element"),
        ("c.rest", "Collection<Element>"),
        ("c.firstOrNull", "Element | Null"),
        ("p.first", "First"),
        ("p.second", "Second"),
        ("p.key", "First"),
        ("p.value", "Second"),
        ("du.value", "Number"),
        (
            "du.unit",
            "\"d\" | \"h\" | \"min\" | \"ms\" | \"ns\" | \"s\" | \"us\"",
        ),
        ("du.isoString", "String"),
        ("du.isPositive", "Boolean"),
        ("ds.value", "Number"),
        (
            "ds.unit",
            "\"b\" | \"gb\" | \"gib\" | \"kb\" | \"kib\" | \"mb\" | \"mib\" | \"pb\" | \"pib\" | \"tb\" | \"tib\"",
        ),
        ("ds.isBinaryUnit", "Boolean"),
        ("re.pattern", "String"),
        ("re.groupCount", "Int"),
        ("sq.start", "Int"),
        ("sq.end", "Int"),
        ("sq.step", "Int"),
        ("b.length", "Int"),
        ("b.size", "DataSize"),
        ("b.base64", "String"),
        ("b.sha256Int", "Int"),
        ("cl.simpleName", "String"),
        ("res.uri", "String"),
        ("res.text", "String"),
        ("res.bytes", "Bytes"),
        ("res.sha256Int", "Int"),
        ("fo.value", "Any"),
        ("fo.renderer", "BaseValueRenderer"),
        ("fo.text", "String"),
        ("fo.bytes", "Bytes"),
        ("mo.files", "Mapping<String, FileOutput> | Null"),
        ("rd.text", "String"),
        ("rd.bytes", "Bytes"),
        ("prd.before", "Null | String"),
        ("prd.value", "Any"),
        ("prd.after", "Null | String"),
        ("acc.property", "Null | String"),
        ("acc.key", "Any"),
    ];
    let lines: Vec<&str> = actual.lines().collect();
    assert_eq!(lines.len(), expected_suffixes.len());
    for (line, (label, ty)) in lines.iter().zip(expected_suffixes) {
        let expected = format!(
            "{label}=Cannot find property `__probe_type__` in object of type `pkl.ref#Reference<main#MyDomain, {ty}>`."
        );
        assert_eq!(*line, expected);
    }

    // Listing and Mapping reject all property references; `default` produces DEFAULT_MEMBER hint.
    let li_len = eval_expr_err(source, "liLen");
    assert_eq!(li_len.kind, ErrorKind::MissingMember);
    assert_eq!(
        li_len.hint,
        Some("Cannot find property `length` in type `Listing<Int>`.".into())
    );
    let li_def = eval_expr_err(source, "liDef");
    assert_eq!(li_def.kind, ErrorKind::MissingMember);
    assert_eq!(
        li_def.hint,
        Some("Cannot reference property `default` in class `Listing`.".into())
    );
    let ma_keys = eval_expr_err(source, "maKeys");
    assert_eq!(ma_keys.kind, ErrorKind::MissingMember);
    assert_eq!(
        ma_keys.hint,
        Some("Cannot find property `keys` in type `Mapping<String, Int>`.".into())
    );
    let ma_def = eval_expr_err(source, "maDef");
    assert_eq!(ma_def.kind, ErrorKind::MissingMember);
    assert_eq!(
        ma_def.hint,
        Some("Cannot reference property `default` in class `Mapping`.".into())
    );
    let raw_li_def = eval_expr_err(source, "rawLiDef");
    assert_eq!(
        raw_li_def.hint,
        Some("Cannot reference property `default` in class `Listing`.".into())
    );
    let raw_ma_def = eval_expr_err(source, "rawMaDef");
    assert_eq!(
        raw_ma_def.hint,
        Some("Cannot reference property `default` in class `Mapping`.".into())
    );
}

#[test]
fn wildcard_entry_converter_matches_yaml_non_string_map_keys() {
    let source = r#"
import "pkl:yaml"
p = new yaml.Parser {
  converters {
    ["[*]"] = (v) -> "CONVERTED:\(v)"
  }
}
res = p.parse("1: hello\na: world").toString()
"#;
    assert_eq!(
        eval_expr_string(source, "res"),
        "new Dynamic { [1] = \"CONVERTED:hello\"; a = \"world\" }"
    );
}
