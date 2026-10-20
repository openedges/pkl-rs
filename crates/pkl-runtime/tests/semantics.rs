//! Golden primitive outputs extracted from the pinned Java run, with provenance
//! beside the fixture. Full catalog mismatches remain in the experiment report.
fn unhex(input: &str) -> String {
    let bytes = (0..input.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&input[i..i + 2], 16).unwrap())
        .collect();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn java_recorded_numeric_and_regex_regressions() {
    let fixture = include_str!("../../../tests/fixtures/m1/semantics/regressions.tsv");
    let mut count = 0;
    for row in fixture.lines() {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 4);
        let input = unhex(fields[2]);
        let expected = unhex(fields[3]);
        let actual = match fields[1] {
            "numeric" => pkl_runtime::numeric::probe(&input),
            "regex" => pkl_runtime::regex::probe(&input),
            _ => panic!("unknown family"),
        }
        .unwrap();
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{}", fields[0]);
        count += 1;
    }
    assert_eq!(count, 56);
}
