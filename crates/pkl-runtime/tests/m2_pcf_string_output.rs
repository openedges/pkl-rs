//! Whole output bytes acquired from the pinned Java evaluator, including UTF-16 Host values.
use pkl_runtime::{
    evaluation::{
        frontend::{DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput},
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    value::{PklString, Value},
};

struct Fixture<'a>(&'a [u8]);

impl<'a> Fixture<'a> {
    fn take(&mut self, length: usize) -> &'a [u8] {
        let (value, remaining) = self.0.split_at(length);
        self.0 = remaining;
        value
    }

    fn count(&mut self) -> usize {
        u32::from_be_bytes(self.take(4).try_into().unwrap()) as usize
    }

    fn bytes(&mut self) -> &'a [u8] {
        let length = self.count();
        self.take(length)
    }

    fn text(&mut self) -> &'a str {
        std::str::from_utf8(self.bytes()).unwrap()
    }

    fn units(&mut self) -> Vec<u16> {
        let length = self.count();
        self.take(length * 2)
            .as_chunks::<2>()
            .0
            .iter()
            .map(|unit| u16::from_be_bytes(*unit))
            .collect()
    }
}

#[test]
fn actual_pcf_output_matches_java_for_host_utf16_strings() {
    let mut fixture = Fixture(include_bytes!(
        "../../../tests/fixtures/m2-pcf-string-output/cases.bin"
    ));
    assert_eq!(fixture.take(8), b"PKLPCF1\0");
    let count = fixture.count();
    assert_eq!(count, 192, "all fixed Java output cases must be retained");
    for _ in 0..count {
        let id = fixture.text();
        let source = fixture.text();
        let value = fixture.units();
        let expected = fixture.bytes();
        let mut session = SourceSession::compile_output(
            ModuleSourceInput {
                text: source,
                uri: "file:///stage40-pcf-string-output.pkl",
                inferred_name: "stage40-pcf-string-output",
            },
            &[],
            &ModuleEvalOptions {
                diagnostic_profile: DiagnosticProfile::ApiIdentity,
                ..Default::default()
            },
            SourceSessionOptions::default(),
        )
        .unwrap_or_else(|error| panic!("{id}: module preparation failed: {error:?}"));
        session.start_output_bytes(0, None).unwrap();
        let mut ticks = 0;
        let mut value_reads = 0;
        let bytes = loop {
            ticks += 1;
            assert!(ticks <= 100_000, "{id}: output failed to settle");
            let event = session.poll(0, 7);
            let roots = match &event {
                HostPoll::Ready(Ok(value)) => vec![value.clone()],
                _ => Vec::new(),
            };
            session.collect(&roots);
            match event {
                HostPoll::Yielded => {}
                HostPoll::Read { token, uri } if uri == "life:value" => {
                    value_reads += 1;
                    session
                        .resume(
                            &token,
                            Ok(Value::String(PklString::from_utf16(value.clone()))),
                            0,
                        )
                        .unwrap();
                }
                HostPoll::Read { token, uri }
                    if uri.starts_with("prop:") || uri.starts_with("env:") =>
                {
                    session.resume_default_read(&token, &[], &[], 0).unwrap();
                }
                HostPoll::Ready(Ok(value)) => {
                    break SourceSession::result_bytes(value).unwrap();
                }
                HostPoll::Ready(Err(error)) => {
                    let failure = session.output_failure(error);
                    panic!("{id}: {}", String::from_utf8_lossy(&failure.stderr));
                }
                other => panic!("{id}: unexpected output event: {other:?}"),
            }
        };
        assert_eq!(
            value_reads, 1,
            "{id}: actual Host input must be evaluated once"
        );
        assert_eq!(bytes.as_ref(), expected, "{id}: complete Java output bytes");
        session.close();
        session.collect(&[]);
        assert_eq!(session.heap_stats().slots, 0, "{id}: closed session roots");
        assert_eq!(
            bytes.as_ref(),
            expected,
            "{id}: output survives evaluator close"
        );
    }
    assert!(fixture.0.is_empty(), "unconsumed Java fixture records");
}
