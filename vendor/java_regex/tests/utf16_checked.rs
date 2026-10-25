//! Raw JDK controls are source-bound in the draft's authority.json. Budget
//! assertions below are backend safety contracts, not Java compatibility rows.
use java_regex::utf16::{Limits, MatchError, Utf16Regex};
use std::fmt::Write;
fn hex(input: &[u16]) -> String {
    input
        .iter()
        .map(|x| format!("{x:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}
fn find(out: &mut String, name: &str, pattern: &[u16], input: &[u16]) {
    write!(out, "{name} pattern={} input={}", hex(pattern), hex(input)).unwrap();
    let re = Utf16Regex::with_flags(pattern, "Uu").unwrap();
    let mut matcher = re.matcher(input, Limits::default());
    while let Some(m) = matcher.find().unwrap() {
        write!(
            out,
            " [{},{}:{}]",
            m.start,
            m.end,
            hex(&input[m.start..m.end])
        )
        .unwrap();
    }
    out.push('\n');
}
fn find_one(out: &mut String, name: &str, pattern: &[u16], input: &[u16]) {
    let re = Utf16Regex::with_flags(pattern, "Uu").unwrap();
    let mut m = re.matcher(input, Limits::default());
    match m.find_from(1).unwrap() {
        Some(m) => writeln!(
            out,
            "{name} found=true [{},{}:{}]",
            m.start,
            m.end,
            hex(&input[m.start..m.end])
        )
        .unwrap(),
        None => writeln!(out, "{name} found=false").unwrap(),
    }
}
#[test]
fn pinned_jdk_cursor_and_unicode15_raw_output() {
    let pair = [0xd83d, 0xde00];
    let high = [0xd800];
    let low = [0xdc00];
    let dot = [0x2e];
    let cs = "\\p{Cs}".encode_utf16().collect::<Vec<_>>();
    let mut out = String::new();
    find(&mut out, "empty-pair", &[], &pair);
    find(&mut out, "dot-pair", &dot, &pair);
    find_one(&mut out, "dot-pair-from-unit-1", &dot, &pair);
    find_one(&mut out, "empty-pair-from-unit-1", &[], &pair);
    find(&mut out, "dot-high", &dot, &high);
    find(&mut out, "dot-low", &dot, &low);
    find(&mut out, "cs-high", &cs, &high);
    find(&mut out, "cs-low", &cs, &low);
    find(&mut out, "cs-pair", &cs, &pair);
    find(&mut out, "literal-high-pair", &high, &pair);
    find(&mut out, "literal-low-pair", &low, &pair);
    // This original DC00-vs-DE00 row is an unequal-value control. The separate
    // exact-low supplement below protects the corrected semantic distinction.
    find_one(&mut out, "literal-low-pair-from-unit-1", &low, &pair);
    find(
        &mut out,
        "unicode15-category-letter",
        &"\\p{L}+".encode_utf16().collect::<Vec<_>>(),
        &[0x1c89, 0xd807, 0xdf02],
    );
    writeln!(
        &mut out,
        "types 1c89={} 11f02={}",
        java_regex::utf16::character_record(0x1c89)
            .unwrap()
            .category,
        java_regex::utf16::character_record(0x11f02)
            .unwrap()
            .category
    )
    .unwrap();
    assert_eq!(
        out.as_bytes(),
        include_bytes!("utf16-authority/jdk-cursor.stdout.bin")
    );
}
#[test]
fn exact_low_surrogate_can_match_at_explicit_unit_start() {
    let pair = [0xd83d, 0xde00];
    let patterns = [
        vec![0xde00],
        "\\uDE00".encode_utf16().collect(),
        "[\\uDE00]".encode_utf16().collect(),
    ];
    for flags in ["", "Uu"] {
        for pattern in &patterns {
            let re = Utf16Regex::with_flags(pattern, flags).unwrap();
            assert_eq!(re.find(&pair, Limits::default()).unwrap(), None);
            let m = re
                .matcher(&pair, Limits::default())
                .find_from(1)
                .unwrap()
                .unwrap();
            assert_eq!((m.start, m.end), (1, 2));
            assert_eq!(&pair[m.start..m.end], &[0xde00]);
        }
    }
}
#[test]
fn budget_failure_is_sticky_and_cannot_become_a_negative_match() {
    for pattern in ["a", "(?!a)", "(?:a|)"] {
        let re = Utf16Regex::from_str(pattern, "Uu").unwrap();
        let text = [0x61];
        let mut m = re.matcher(
            &text,
            Limits {
                steps: 0,
                depth: 500,
            },
        );
        assert_eq!(m.find(), Err(MatchError::StepBudgetExceeded { limit: 0 }));
        assert_eq!(
            m.find_from(0),
            Err(MatchError::StepBudgetExceeded { limit: 0 })
        );
    }
    let re = Utf16Regex::from_str("a+", "Uu").unwrap();
    assert!(matches!(
        re.captures_entire(
            &[0x61; 64],
            Limits {
                steps: 100_000,
                depth: 4
            }
        ),
        Err(MatchError::DepthBudgetExceeded { limit: 4 })
    ));
}
#[test]
fn find_budget_is_not_reset_after_success() {
    let re = Utf16Regex::from_str("", "Uu").unwrap();
    let text = [0x61; 64];
    let mut matcher = re.matcher(
        &text,
        Limits {
            steps: 5,
            depth: 500,
        },
    );
    let mut successes = 0;
    loop {
        match matcher.find() {
            Ok(Some(_)) => successes += 1,
            Err(MatchError::StepBudgetExceeded { limit: 5 }) => break,
            other => panic!("budget was lost: {other:?}"),
        }
    }
    assert!(successes > 0 && successes < text.len() + 1);
}

#[test]
fn unsupported_paths_keep_explicit_failures() {
    use java_regex::utf16::CompileErrorKind;
    assert_eq!(
        Utf16Regex::from_str("\\p{UnimplementedProperty}", "Uu")
            .unwrap_err()
            .kind,
        CompileErrorKind::Unsupported
    );
    let re = Utf16Regex::from_str("(?!\\X)", "Uu").unwrap();
    assert!(matches!(
        re.find(&[0x61], Limits::default()),
        Err(MatchError::Unsupported(_))
    ));
}

#[test]
fn owned_cursor_keeps_budget_and_failure_across_callback_boundaries() {
    use java_regex::utf16::OwnedMatcher;
    use std::sync::Arc;
    let regex = Arc::new(Utf16Regex::from_str("", "Uu").unwrap());
    let mut cursor = OwnedMatcher::new(
        regex,
        Arc::from([0x61u16; 64]),
        Limits {
            steps: 5,
            depth: 500,
        },
    );
    let mut successes = 0;
    loop {
        match cursor.find() {
            Ok(Some(matched)) => {
                assert_eq!((matched.start, matched.end), (successes, successes));
                successes += 1;
            }
            Err(MatchError::StepBudgetExceeded { limit: 5 }) => break,
            other => panic!("owned continuation lost its budget: {other:?}"),
        }
    }
    assert!(successes > 0 && successes < 65);
    assert_eq!(
        cursor.find(),
        Err(MatchError::StepBudgetExceeded { limit: 5 })
    );
    assert_eq!(
        cursor.looking_at_region(0, 0),
        Err(MatchError::StepBudgetExceeded { limit: 5 })
    );
}

#[test]
fn last_match_region_recheck_uses_opaque_bounds() {
    use java_regex::utf16::OwnedMatcher;
    use std::sync::Arc;
    // Java transform148 rows 087/088 leave "ab" unchanged for replaceLast,
    // although ordinary replaceAll/First found these same patterns.
    for pattern in ["(?<=a)b", "a(?=b)"] {
        let regex = Arc::new(Utf16Regex::from_str(pattern, "Uu").unwrap());
        let mut cursor = OwnedMatcher::new(regex, Arc::from([0x61u16, 0x62]), Limits::default());
        let matched = cursor.find().unwrap().unwrap();
        assert!(cursor.find().unwrap().is_none());
        assert!(cursor
            .looking_at_region(matched.start, matched.end)
            .unwrap()
            .is_none());
    }
}
