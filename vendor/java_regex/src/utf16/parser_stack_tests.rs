//! Internal resource and grammar regressions, not new Java compatibility rows.
//! Existing Java raw expectations and the original failing syntax test remain unchanged.
extern crate std;

use super::{CompileError, CompileErrorKind, Limits, Utf16Regex};
use alloc::{format, string::ToString, vec::Vec};

fn compile(pattern: &str) -> Result<Utf16Regex, CompileError> {
    Utf16Regex::with_flags(&pattern.encode_utf16().collect::<Vec<_>>(), "u")
}
fn small_stack(body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("parser-256k".into())
        .stack_size(256 * 1024)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap();
}
fn exact_error(pattern: &str, kind: CompileErrorKind, message: &str, index: usize) {
    let error = compile(pattern).unwrap_err();
    assert_eq!(
        error,
        CompileError {
            kind,
            message: message.to_string(),
            pattern: pattern.encode_utf16().collect(),
            index,
        }
    );
}
fn matches(pattern: &str, value: &str) -> bool {
    compile(pattern)
        .unwrap()
        .captures_entire(&value.encode_utf16().collect::<Vec<_>>(), Limits::default())
        .unwrap()
        .is_some()
}

#[test]
fn groups_budget_and_error_cleanup_fit_256k() {
    small_stack(|| {
        // Root is one logical pattern frame. These 255 groups are admissible;
        // parse, lookbehind length walk, and ordinary destruction must all fit.
        for opening in ["(", "(?:", "(?i:"] {
            let source = format!("{}a{}", opening.repeat(255), ")".repeat(255));
            let regex = compile(&source).unwrap();
            assert_eq!(regex.group_count(), if opening == "(" { 255 } else { 0 });
            drop(regex);
            // A fully built deep subtree must also be released on later error.
            let error_source = format!("{source}\\q");
            exact_error(
                &error_source,
                CompileErrorKind::Syntax,
                "Illegal/unsupported escape sequence",
                error_source.len() - 1,
            );
        }
        let behind = format!("(?<={}a{})x", "(?:".repeat(254), ")".repeat(254));
        drop(compile(&behind).unwrap());
        // Preserve the exact existing09 257-group input and the 256 budget.
        let deep = format!("{}a{}", "(".repeat(257), ")".repeat(257));
        exact_error(
            &deep,
            CompileErrorKind::NestingBudgetExceeded,
            "Pattern nesting exceeds backend compilation budget",
            256,
        );
        assert!(matches("(?<x>a)+", "aa"));
    });
}

#[test]
fn classes_and_intersection_continuations_fit_256k() {
    small_stack(|| {
        let accepted = format!("{}a{}", "[".repeat(255), "]".repeat(255));
        drop(compile(&accepted).unwrap());
        let rejected = format!("{}a{}", "[".repeat(256), "]".repeat(256));
        exact_error(
            &rejected,
            CompileErrorKind::NestingBudgetExceeded,
            "Character class nesting exceeds backend compilation budget",
            255,
        );
        let mixed = format!(
            "{}{}a{}{}",
            "(".repeat(128),
            "[".repeat(128),
            "]".repeat(128),
            ")".repeat(128)
        );
        exact_error(
            &mixed,
            CompileErrorKind::NestingBudgetExceeded,
            "Character class nesting exceeds backend compilation budget",
            255,
        );
        // Literal RHS enters a consume=false scope; nested-only RHS does not.
        assert!(matches("[a&&[ab]b&&b]", "a"));
        assert!(!matches("[a&&[ab]b&&b]", "b"));
        assert!(!matches("[a&&[ab]&&b]", "a"));
        assert!(matches("[a-z&&[^b]]", "a"));
        assert!(!matches("[a-z&&[^b]]", "b"));
        // An invalid nested range endpoint is parsed before rejecting its kind.
        exact_error(
            "[a-[b",
            CompileErrorKind::Syntax,
            "Unclosed character class",
            5,
        );
        exact_error(
            "[a-[b]]",
            CompileErrorKind::Syntax,
            "Illegal character range",
            6,
        );
        exact_error(
            "[a&&]",
            CompileErrorKind::Syntax,
            "Empty intersection operand",
            4,
        );
        assert!(matches("[[]a]]", "a"));
    });
}

#[test]
fn group_flags_quantifiers_and_first_error_order_are_preserved() {
    small_stack(|| {
        assert!(matches("(?i:a|b)+C", "aBC"));
        assert!(!matches("(?i:a|b)+C", "aBc"));
        assert!(matches("(?s)|.", "\n"));
        assert!(!matches("(?:(?s))|.", "\n"));
        assert!(matches(r"(\Qab\E)+?", "abab"));
        let regex = compile("(?<word>a|b){2}(?=c)c").unwrap();
        let captures = regex
            .captures_entire(&[97, 98, 99], Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(captures.groups, [Some((0, 3)), Some((1, 2))]);
        exact_error("(?c)(", CompileErrorKind::Syntax, "Expected ')'", 5);
        exact_error(
            r"(?c)\q",
            CompileErrorKind::Syntax,
            "Illegal/unsupported escape sequence",
            5,
        );
        exact_error(
            "(?<x>a)(?<x>(",
            CompileErrorKind::Syntax,
            "Duplicate group name: x",
            12,
        );
        exact_error("(?i:a", CompileErrorKind::Syntax, "Expected ')'", 5);
        exact_error(
            "(?<x>a)(?<=\\k<x>)",
            CompileErrorKind::Syntax,
            "Look-behind group does not have an obvious maximum length",
            17,
        );
        assert!(matches("a", "a"));
    });
}
