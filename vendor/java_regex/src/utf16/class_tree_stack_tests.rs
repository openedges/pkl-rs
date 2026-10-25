//! Internal resource contracts for deep class trees at shallow syntax depth.
extern crate std;
use super::{CompileErrorKind, Limits, Utf16Regex};
use alloc::{format, string::String, vec::Vec};

fn compile(source: &str) -> Utf16Regex {
    Utf16Regex::from_str(source, "u").unwrap()
}
fn small_stack(body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("class-tree-256k".into())
        .stack_size(256 * 1024)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap();
}
fn sources() -> [String; 2] {
    // Bracket depth2, left-nested intersection; literal RHS gives the other shape.
    [
        format!("[a{}]", "&&[a]".repeat(4096)),
        format!("[a{}]", "&&a".repeat(4096)),
    ]
}
#[test]
fn shallow_syntax_deep_class_success_and_error_drop_fit_256k() {
    small_stack(|| {
        for source in sources() {
            drop(compile(&source));
            for tail in [r"\q", "("] {
                let invalid = format!("{source}{tail}");
                let error = Utf16Regex::from_str(&invalid, "u").unwrap_err();
                assert_eq!(error.kind, CompileErrorKind::Syntax);
                assert_eq!(error.pattern, invalid.encode_utf16().collect::<Vec<_>>());
                assert_eq!(
                    error.message,
                    if tail == "(" {
                        "Expected ')'"
                    } else {
                        "Illegal/unsupported escape sequence"
                    }
                );
                assert_eq!(
                    error.index,
                    if tail == "(" {
                        invalid.len()
                    } else {
                        invalid.len() - 1
                    }
                );
            }
            let invalid = &source[..source.len() - 1];
            let error = Utf16Regex::from_str(invalid, "u").unwrap_err();
            assert_eq!(error.kind, CompileErrorKind::Syntax);
            assert_eq!(error.message, "Unclosed character class");
            assert_eq!(error.index, invalid.len());
        }
    });
}
#[test]
fn deep_class_regex_clone_and_matcher_branch_clone_fit_256k() {
    small_stack(|| {
        for source in sources() {
            let original = compile(&source);
            let copied = original.clone();
            drop(original);
            for (input, expected) in [(97u16, true), (98, false)] {
                assert_eq!(
                    copied
                        .captures_entire(&[input], Limits::default())
                        .unwrap()
                        .is_some(),
                    expected
                );
            }
            // Class predicates historically do not add matcher budget ticks.
            // Keep the same explicit low-budget failure/value boundary as one
            // shallow class node; do not turn resource exhaustion into false.
            let shallow = compile("[a]");
            for steps in 0..8 {
                let limits = Limits {
                    steps,
                    ..Limits::default()
                };
                for input in [97u16, 98] {
                    assert_eq!(
                        copied.captures_entire(&[input], limits),
                        shallow.captures_entire(&[input], limits)
                    );
                }
            }
            drop(copied);
        }
        // Keep negation, union inside a side, and nested intersection scope.
        for (source, accepted, rejected) in [
            ("[ab&&[^b]]", 'a', 'b'),
            ("[^ab&&[^b]]", 'b', 'a'),
            ("[a&&[ab]b&&b]", 'a', 'b'),
        ] {
            let regex = compile(source);
            assert!(regex
                .captures_entire(&[accepted as u16], Limits::default())
                .unwrap()
                .is_some());
            assert!(regex
                .captures_entire(&[rejected as u16], Limits::default())
                .unwrap()
                .is_none());
        }
    });
}
