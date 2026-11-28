use super::*;
use std::fmt::Write;
fn hex(units: &[u16]) -> String {
    units
        .iter()
        .map(|x| format!("{x:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}
fn observed(out: &mut String, label: &str, found: Option<MatchRanges>, input: &[u16]) {
    match found {
        None => write!(out, " {label}=false").unwrap(),
        Some(m) => write!(
            out,
            " {label}=true [{},{}:{}]",
            m.start,
            m.end,
            hex(&input[m.start..m.end])
        )
        .unwrap(),
    }
}
#[test]
fn pinned_jdk_slice_and_lookbehind_raw_output() {
    let text = [0xd83d, 0xde00, 0x78];
    let encode = |s: &str| s.encode_utf16().collect::<Vec<_>>();
    let rows = [
        ("raw-low-slice-Uu", vec![0xde00, 0x78], "Uu"),
        ("escaped-low-slice-Uu", encode("\\uDE00x"), "Uu"),
        ("raw-low-alone-Uu", vec![0xde00], "Uu"),
        ("escaped-low-alone-Uu", encode("\\uDE00"), "Uu"),
        ("class-low-Uu", encode("[\\uDE00]"), "Uu"),
        ("raw-low-slice-no-flags", vec![0xde00, 0x78], ""),
        ("escaped-low-slice-no-flags", encode("\\uDE00x"), ""),
        ("behind-dot-Uu", encode("(?<=.)x"), "Uu"),
        ("behind-raw-pair-Uu", encode("(?<=😀)x"), "Uu"),
        ("behind-escaped-pair-Uu", encode("(?<=\\x{1F600})x"), "Uu"),
        ("behind-low-Uu", encode("(?<=\\uDE00)x"), "Uu"),
    ];
    let mut out = String::new();
    for (name, pattern, flags) in rows {
        let re = Utf16Regex::with_flags(&pattern, flags).unwrap();
        write!(
            out,
            "{name} raw={} input={} flags={} root={}",
            hex(&pattern),
            hex(&text),
            if flags.is_empty() { 0 } else { 320 },
            if re.start_s { "StartS" } else { "Start" }
        )
        .unwrap();
        observed(
            &mut out,
            "find",
            re.find(&text, Limits::default()).unwrap(),
            &text,
        );
        for pos in 0..=2 {
            observed(
                &mut out,
                &("find".to_string() + &pos.to_string()),
                re.matcher(&text, Limits::default()).find_from(pos).unwrap(),
                &text,
            );
        }
        out.push('\n');
    }
    assert_eq!(
        out.as_bytes(),
        include_bytes!("../../tests/fixtures/utf16-authority/jdk-slice-behind.stdout.bin")
    );
}

#[test]
fn round5_looking_at_region_does_not_cross_split_surrogate_end() {
    let text = [0xd83d, 0xde00]; // U+1F600 😀 split at region end = 1
    let pat: Vec<u16> = ".".encode_utf16().collect();
    let re = Utf16Regex::with_flags(&pat, "").unwrap();
    let mut matcher = re.matcher(&text, Limits::default());
    let matched = matcher
        .looking_at_region(0, 1)
        .expect("looking_at_region must succeed")
        .expect("single high surrogate within [0, 1) must match `.`");
    assert_eq!(matched.start, 0);
    assert_eq!(
        matched.end, 1,
        "looking_at_region(0, 1) must not consume low surrogate past text_end=1"
    );
}
