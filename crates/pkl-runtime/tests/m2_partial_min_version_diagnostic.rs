//! Aggregate diagnostic acquisition; original fail-fast oracle tests remain unchanged.
use pkl_runtime::{
    evaluation::{EvaluationOptions, scalar_expression_text_with_options},
    source::SourceText,
    value::PklString,
};
use std::{fs, io::Write, panic, path::PathBuf};

fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}

fn encoded_units(text: &PklString) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

#[test]
fn aggregate_all_eighty_primary_raw_channels_and_fail_on_any_difference() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-min-version-partial-oracle");
    let output = std::env::var_os("STAGE122_DIAGNOSTIC_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            std::env::temp_dir().join(format!(
                "pkl-stage122-min-version-{}-{timestamp}",
                std::process::id()
            ))
        });
    // Both explicit and default paths must be fresh; retain captures on success or failure.
    fs::create_dir(&output).unwrap();
    eprintln!("aggregate raw diagnostic captures: {}", output.display());
    let programs = fs::read_to_string(fixtures.join("ordinary-programs.tsv")).unwrap();
    assert_eq!(programs.lines().count(), 80);
    let mut roster = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("attempted.tsv"))
        .unwrap();
    let mut attempted = 0;
    let mut mismatched = 0;
    for id in programs.lines() {
        assert!(
            id.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        );
        let expected = fixtures.join("ordinary").join(id);
        let actual = output.join(id);
        fs::create_dir(&actual).unwrap();
        let source = SourceText::from_utf16(units(
            &fs::read(expected.join("source.code-units.bin")).unwrap(),
        ));
        let source_matches = source.original().to_java_utf8().as_bytes()
            == fs::read(expected.join("source.pkl")).unwrap();
        fs::write(
            actual.join("source.code-units.bin"),
            encoded_units(source.original()),
        )
        .unwrap();
        fs::write(
            actual.join("source.pkl"),
            source.original().to_java_utf8().as_bytes(),
        )
        .unwrap();
        let uri = fs::read_to_string(expected.join("uri.txt")).unwrap();
        let name = fs::read_to_string(expected.join("inferred-name.txt")).unwrap();
        fs::write(actual.join("uri.txt"), uri.as_bytes()).unwrap();
        fs::write(actual.join("inferred-name.txt"), name.as_bytes()).unwrap();
        let result = panic::catch_unwind(|| {
            scalar_expression_text_with_options(
                source,
                uri,
                name,
                "result",
                EvaluationOptions::default(),
            )
        });
        let (status, payload) = match result {
            Ok(Ok(value)) => (b"success\n".as_slice(), Some(("string", value))),
            Ok(Err(error)) => (b"error\n".as_slice(), Some(("error", error))),
            Err(error) => {
                let message = error
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| error.downcast_ref::<&str>().copied())
                    .unwrap_or("non-string panic");
                fs::write(actual.join("panic.bin"), message.as_bytes()).unwrap();
                (b"panic\n".as_slice(), None)
            }
        };
        fs::write(actual.join("status.bin"), status).unwrap();
        let expected_status = fs::read(expected.join("status.bin")).unwrap();
        let expected_channel = match expected_status.as_slice() {
            b"success\n" => "string",
            b"error\n" => "error",
            other => panic!("ordinary fixture has nonordinary status: {other:?}"),
        };
        let status_matches = status == expected_status;
        let (utf16_matches, utf8_matches) = if let Some((channel, text)) = payload {
            let utf16 = encoded_units(&text);
            let utf8 = text.to_java_utf8();
            fs::write(actual.join(format!("{channel}.code-units.bin")), &utf16).unwrap();
            fs::write(actual.join(format!("{channel}.bin")), utf8.as_bytes()).unwrap();
            (
                utf16
                    == fs::read(expected.join(format!("{expected_channel}.code-units.bin")))
                        .unwrap(),
                utf8.as_bytes()
                    == fs::read(expected.join(format!("{expected_channel}.bin"))).unwrap(),
            )
        } else {
            (false, false)
        };
        let matches = source_matches && status_matches && utf16_matches && utf8_matches;
        attempted += 1;
        mismatched += usize::from(!matches);
        let checks = format!(
            "source\t{source_matches}\nstatus\t{status_matches}\nutf16\t{utf16_matches}\njava-utf8\t{utf8_matches}\nmatch\t{matches}\n"
        );
        fs::write(actual.join("checks.tsv"), checks).unwrap();
        writeln!(roster, "{id}\t{matches}").unwrap();
        roster.flush().unwrap();
    }
    fs::write(
        output.join("completed.tsv"),
        format!(
            "attempted\t{attempted}\nmatched\t{}\nmismatched\t{mismatched}\n",
            attempted - mismatched
        ),
    )
    .unwrap();
    assert_eq!(attempted, 80);
    assert_eq!(
        mismatched,
        0,
        "aggregate raw comparison failed; complete observations retained at {}",
        output.display()
    );
}
