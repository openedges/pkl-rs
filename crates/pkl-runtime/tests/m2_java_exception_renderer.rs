//! Whole-byte comparisons with the pinned Java Throwable and VmExceptionRenderer.
//! Captured ordinary diagnostics and runtime profile strings are explicit inputs
//! to this renderer component; VM stack capture is a separate required contract.
use pkl_runtime::{
    java_exception::{
        InvalidThrowableId, JavaStackTraceElement, JavaThrowable, JavaThrowableGraph,
        JavaThrowableId, flatten_caught, url_encode,
    },
    value::PklString,
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-java-exceptions")
}
fn field(value: &str) -> Option<PklString> {
    if value == "-" {
        return None;
    }
    let hex = value.strip_prefix('u').expect("UTF-16 field");
    assert_eq!(hex.len() % 4, 0);
    Some(PklString::from_utf16(
        (0..hex.len())
            .step_by(4)
            .map(|i| u16::from_str_radix(&hex[i..i + 4], 16).unwrap())
            .collect::<Vec<_>>(),
    ))
}
fn id(value: &str) -> JavaThrowableId {
    JavaThrowableId(value.parse().unwrap())
}
fn graph(path: &Path) -> JavaThrowableGraph {
    let text = fs::read_to_string(path).unwrap();
    let mut graph = JavaThrowableGraph::default();
    for line in text.lines() {
        let f: Vec<_> = line.split('\t').collect();
        match f[0] {
            "throwable" => {
                assert_eq!(f.len(), 7);
                assert_eq!(id(f[1]).0, graph.throwables.len());
                let mut item = JavaThrowable::new(field(f[2]).unwrap(), field(f[3]));
                item.localized_message = field(f[4]);
                item.to_string_override = field(f[5]);
                item.cause = (f[6] != "-").then(|| id(f[6]));
                graph.throwables.push(item);
            }
            "frame" => {
                assert_eq!(f.len(), 11);
                graph.throwables[id(f[1]).0]
                    .stack_trace
                    .push(JavaStackTraceElement {
                        class_loader_name: field(f[2]),
                        module_name: field(f[3]),
                        module_version: field(f[4]),
                        declaring_class: field(f[5]).unwrap(),
                        method_name: field(f[6]).unwrap(),
                        file_name: field(f[7]),
                        line_number: f[8].parse().unwrap(),
                        omit_class_loader_name: f[9] == "1",
                        omit_module_version: f[10] == "1",
                    });
            }
            "suppressed" => {
                assert_eq!(f.len(), 3);
                graph.throwables[id(f[1]).0].suppressed.push(id(f[2]));
            }
            other => panic!("unknown row {other}"),
        }
    }
    graph.validate().unwrap();
    graph
}
fn input(dir: &Path, name: &str) -> PklString {
    let bytes = fs::read(dir.join(format!("{name}.code-units.bin"))).unwrap();
    let (pairs, rest) = bytes.as_chunks::<2>();
    assert!(rest.is_empty());
    PklString::from_utf16(
        pairs
            .iter()
            .map(|x| u16::from_be_bytes(*x))
            .collect::<Vec<_>>(),
    )
}
fn compare(actual: &PklString, dir: &Path, name: &str) {
    let units: Vec<_> = actual.encode_utf16().flat_map(u16::to_be_bytes).collect();
    assert_eq!(
        units,
        fs::read(dir.join(format!("{name}.code-units.bin"))).unwrap(),
        "{} {name} UTF-16",
        dir.display()
    );
    assert_eq!(
        actual.to_java_utf8().as_bytes(),
        fs::read(dir.join(format!("{name}.bin"))).unwrap(),
        "{} {name} UTF-8",
        dir.display()
    );
}
fn matrix(profile: &str) {
    let root = root();
    let roster = fs::read_to_string(root.join("input/cases.txt")).unwrap();
    let ids: Vec<_> = roster.lines().collect();
    assert_eq!(ids.len(), 54);
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
    for case in ids {
        let graph = graph(&root.join("input").join(format!("{case}.tsv")));
        let expected = root.join(profile).join(case);
        let separator = input(&expected, "line-separator");
        assert_eq!(separator, if profile == "lf" { "\n" } else { "\r\n" });
        let release = input(&expected, "release");
        for prefix in ["", "colored-"] {
            let ordinary = input(&expected, &format!("{prefix}ordinary"));
            let rendered = graph
                .render_pkl_bug(JavaThrowableId(0), &ordinary, &release, &separator)
                .unwrap();
            compare(&rendered, &expected, &format!("{prefix}bug"));
            if prefix.is_empty() {
                compare(&flatten_caught(&rendered), &expected, "caught");
            }
        }
        let mut expected_stems: BTreeSet<String> = [
            "ordinary",
            "bug",
            "caught",
            "colored-ordinary",
            "colored-bug",
            "release",
            "line-separator",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        for (i, item) in graph.throwables.iter().enumerate() {
            let description = graph.to_string(JavaThrowableId(i)).unwrap();
            compare(&description, &expected, &format!("description-{i}"));
            compare(&url_encode(&description), &expected, &format!("url-{i}"));
            compare(
                &graph
                    .print_stack_trace(JavaThrowableId(i), &separator)
                    .unwrap(),
                &expected,
                &format!("trace-{i}"),
            );
            expected_stems.extend([
                format!("description-{i}"),
                format!("url-{i}"),
                format!("trace-{i}"),
            ]);
            for (j, frame) in item.stack_trace.iter().enumerate() {
                compare(&frame.render(), &expected, &format!("frame-{i}-{j}"));
                expected_stems.insert(format!("frame-{i}-{j}"));
            }
        }
        let expected_files: BTreeSet<_> = expected_stems
            .into_iter()
            .flat_map(|stem| [format!("{stem}.bin"), format!("{stem}.code-units.bin")])
            .collect();
        let actual_files: BTreeSet<_> = fs::read_dir(&expected)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                assert!(e.file_type().unwrap().is_file());
                e.file_name().into_string().unwrap()
            })
            .collect();
        assert_eq!(
            actual_files,
            expected_files,
            "{} complete field namespace",
            expected.display()
        );
    }
}
#[test]
fn complete_java_exception_matrix_lf() {
    matrix("lf");
}
#[test]
fn complete_java_exception_matrix_crlf() {
    matrix("crlf");
}
#[test]
fn invalid_references_fail_without_rendering_partial_diagnostics() {
    let missing = JavaThrowableId(1);
    let mut item = JavaThrowable::new("example.Error", None);
    item.cause = Some(missing);
    let graph = JavaThrowableGraph {
        throwables: vec![item],
    };
    assert_eq!(graph.validate(), Err(InvalidThrowableId(missing)));
    assert_eq!(
        graph.print_stack_trace(JavaThrowableId(0), &"\n".into()),
        Err(InvalidThrowableId(missing))
    );
    assert_eq!(
        graph.render_pkl_bug(
            JavaThrowableId(0),
            &"diagnostic".into(),
            &"release".into(),
            &"\n".into()
        ),
        Err(InvalidThrowableId(missing))
    );
}
