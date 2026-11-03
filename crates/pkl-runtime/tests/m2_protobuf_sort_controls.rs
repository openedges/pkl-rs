//! Exact Java21 Arrays.sort controls acquired before the Stage144 commit.
//! Tests the production Apache Spark-derived sorter, including every compared pair.
//! These 42 algorithm controls are separate from Pkl renderer observations.
#[path = "../src/vm/protobuf_spark_sort.rs"]
mod protobuf_spark_sort;

use std::{cell::RefCell, collections::BTreeMap, fmt::Write, fs, path::PathBuf};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-protobuf-sort-controls")
}
fn tsv(text: &str, width: usize) -> Vec<Vec<&str>> {
    if text.is_empty() {
        return Vec::new();
    }
    let rows = text
        .strip_suffix('\n')
        .expect("terminal LF")
        .split('\n')
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert!(rows.iter().all(|row| row.len() == width));
    rows
}
fn subclass(child: &str, ancestor: &str, parents: &BTreeMap<&str, &str>) -> bool {
    let mut current = child;
    while current != "-" {
        if current == ancestor {
            return true;
        }
        current = parents[current];
    }
    false
}
fn compare(left: &str, right: &str, parents: &BTreeMap<&str, &str>) -> i32 {
    if left == "-" || right == "-" {
        return 0;
    }
    // The pinned RendererNodes comparator deliberately returns Less even for
    // the same object class. Never replace this relation with a total order.
    if subclass(left, right, parents) {
        return -1;
    }
    if subclass(right, left, parents) {
        return 1;
    }
    0
}
fn check(id: &str) {
    let root = fixture();
    let directory = root.join(id);
    let input = fs::read_to_string(directory.join("input.tsv")).unwrap();
    let items = tsv(&input, 5);
    let hierarchy = fs::read_to_string(root.join("hierarchy.tsv")).unwrap();
    let parents = tsv(&hierarchy, 2)
        .into_iter()
        .map(|r| (r[0], r[1]))
        .collect();
    let trace = RefCell::new((0usize, String::new()));
    let result = protobuf_spark_sort::order(items.len(), |left, right| {
        let comparison = compare(items[left][4], items[right][4], &parents);
        let mut state = trace.borrow_mut();
        let sequence = state.0;
        writeln!(state.1, "{sequence}\t{left}\t{right}\t{comparison}").unwrap();
        state.0 += 1;
        comparison
    });
    let (count, trace) = trace.into_inner();
    assert_eq!(
        trace.as_bytes(),
        fs::read(directory.join("comparisons.tsv")).unwrap(),
        "{id}: full compared-pair trace"
    );
    assert_eq!(
        format!("{count}\n").as_bytes(),
        fs::read(directory.join("comparisons.count.bin")).unwrap(),
        "{id}: comparison count"
    );
    let order =
        result.unwrap_or_else(|error| panic!("{id}: unexpected native sort failure: {error}"));
    let mut permutation = String::new();
    for (position, original) in order.into_iter().enumerate() {
        let item = &items[original];
        writeln!(
            permutation,
            "{position}\t{}\t{}\t{}\t{}",
            item[1], item[2], item[3], item[4]
        )
        .unwrap();
    }
    for name in [
        "permutation.tsv",
        "mutated-permutation.tsv",
        "result.stdout.bin",
    ] {
        assert_eq!(
            permutation.as_bytes(),
            fs::read(directory.join(name)).unwrap(),
            "{id}: {name}"
        );
    }
    let units = permutation
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    assert_eq!(
        units,
        fs::read(directory.join("result.stdout.code-units.bin")).unwrap(),
        "{id}: UTF16 permutation"
    );
    assert_eq!(
        b"success\n",
        fs::read(directory.join("result.status.bin"))
            .unwrap()
            .as_slice()
    );
    for name in [
        "result.stderr.bin",
        "result.stderr.code-units.bin",
        "result.error-class.bin",
        "result.error-class.code-units.bin",
        "error-message.bin",
        "error-message.code-units.bin",
    ] {
        assert_eq!(
            b"",
            fs::read(directory.join(name)).unwrap().as_slice(),
            "{id}: {name}"
        );
    }
    assert_eq!(
        b"0\n",
        fs::read(directory.join("error-message.present.bin"))
            .unwrap()
            .as_slice()
    );
}
macro_rules! control {
    ($test:ident, $id:literal) => {
        #[test]
        fn $test() {
            check($id);
        }
    };
}
control!(boundary_0, "boundary-0");
control!(boundary_1, "boundary-1");
control!(boundary_2, "boundary-2");
control!(mixed_31_original, "mixed-31-original");
control!(mixed_31_reverse, "mixed-31-reverse");
control!(mixed_31_rotate, "mixed-31-rotate");
control!(mixed_32_original, "mixed-32-original");
control!(mixed_32_reverse, "mixed-32-reverse");
control!(mixed_32_rotate, "mixed-32-rotate");
control!(mixed_33_original, "mixed-33-original");
control!(mixed_33_reverse, "mixed-33-reverse");
control!(mixed_33_rotate, "mixed-33-rotate");
control!(same_class_33, "same-class-33");
control!(primitive_zero_33, "primitive-zero-33");
control!(unrelated_33, "unrelated-33");
control!(sort_long_mixed_40_original, "sort-long-mixed-40-original");
control!(
    sort_long_mixed_40_normalized_original,
    "sort-long-mixed-40-normalized-original"
);
control!(sort_long_mixed_40_reverse, "sort-long-mixed-40-reverse");
control!(
    sort_long_mixed_40_normalized_reverse,
    "sort-long-mixed-40-normalized-reverse"
);
control!(sort_long_mixed_40_rotate, "sort-long-mixed-40-rotate");
control!(
    sort_long_mixed_40_normalized_rotate,
    "sort-long-mixed-40-normalized-rotate"
);
control!(
    sort_long_mixed_alias_64_original,
    "sort-long-mixed-alias-64-original"
);
control!(
    sort_long_mixed_alias_64_normalized_original,
    "sort-long-mixed-alias-64-normalized-original"
);
control!(
    sort_long_mixed_alias_64_reverse,
    "sort-long-mixed-alias-64-reverse"
);
control!(
    sort_long_mixed_alias_64_normalized_reverse,
    "sort-long-mixed-alias-64-normalized-reverse"
);
control!(
    sort_long_mixed_alias_64_rotate,
    "sort-long-mixed-alias-64-rotate"
);
control!(
    sort_long_mixed_alias_64_normalized_rotate,
    "sort-long-mixed-alias-64-normalized-rotate"
);
control!(run_witness_608_original, "run-witness-608-original");
control!(run_witness_608_reverse, "run-witness-608-reverse");
control!(run_witness_608_rotate, "run-witness-608-rotate");
control!(gallop_low_6, "gallop-low-6");
control!(gallop_high_6, "gallop-high-6");
control!(gallop_low_7, "gallop-low-7");
control!(gallop_high_7, "gallop-high-7");
control!(gallop_low_8, "gallop-low-8");
control!(gallop_high_8, "gallop-high-8");
control!(tail_left_single, "tail-left-single");
control!(tail_right_single, "tail-right-single");
control!(tail_low_exhaust, "tail-low-exhaust");
control!(tail_high_exhaust, "tail-high-exhaust");
control!(capacity_boundary_1541, "capacity-boundary-1541");
control!(capacity_boundary_1542, "capacity-boundary-1542");
