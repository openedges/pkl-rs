//! Natural-order string comparator mirroring `org.pkl.formatter.NaturalOrderComparator`.

use crate::unicode_tables::{is_java_digit, java_digit_value, java_to_lower_case};
use std::cmp::Ordering;

pub(crate) fn compare_natural(s1: &str, s2: &str, ignore_case: bool) -> Ordering {
    let u1: Vec<u16> = s1.encode_utf16().collect();
    let u2: Vec<u16> = s2.encode_utf16().collect();
    let mut i = 0;
    let mut j = 0;

    while i < u1.len() && j < u2.len() {
        let c1 = if ignore_case {
            java_to_lower_case(u1[i])
        } else {
            u1[i]
        };
        let c2 = if ignore_case {
            java_to_lower_case(u2[j])
        } else {
            u2[j]
        };

        // Map each digit run to `(0x0030, numeric_value)` and each non-digit to `(code_unit, 0)`.
        // Because `0x0030..=0x0039` are all Java digits, no non-digit shares bucket `0x0030`,
        // matching Java ASCII ordering while guaranteeing a strict weak total order across all
        // Unicode digits and leading-zero runs (preventing `driftsort` panics).
        let k1 = if is_java_digit(c1) {
            let (n1, next_i) = get_number(&u1, i);
            i = next_i;
            (0x0030u16, n1)
        } else {
            i += 1;
            (c1, 0)
        };
        let k2 = if is_java_digit(c2) {
            let (n2, next_j) = get_number(&u2, j);
            j = next_j;
            (0x0030u16, n2)
        } else {
            j += 1;
            (c2, 0)
        };

        let cmp = k1.cmp(&k2);
        if cmp != Ordering::Equal {
            return cmp;
        }
    }

    (i < u1.len())
        .cmp(&(j < u2.len()))
        .then_with(|| u1.len().cmp(&u2.len()))
}

/// Mirrors `Long.parseLong(s, startIndex, i, 10)` returning `0L` on overflow.
fn get_number(units: &[u16], start_index: usize) -> (i64, usize) {
    let mut i = start_index;
    while i < units.len() && is_java_digit(units[i]) {
        i += 1;
    }
    let mut value: i64 = 0;
    let mut overflow = false;
    for &unit in &units[start_index..i] {
        let digit = i64::from(java_digit_value(unit).unwrap_or(0));
        match value.checked_mul(10).and_then(|v| v.checked_add(digit)) {
            Some(next) => value = next,
            None => {
                overflow = true;
                break;
            }
        }
    }
    if overflow { (0, i) } else { (value, i) }
}

#[cfg(test)]
mod tests {
    use super::compare_natural;
    use std::cmp::Ordering;

    #[test]
    fn round5_compare_natural_is_strict_weak_total_order() {
        let samples = [
            "",
            "0",
            "00",
            "000",
            "000a",
            "00a",
            "0a",
            "0b",
            "1",
            "2",
            "10",
            "a",
            "A",
            "b",
            "\u{0660}",
            "\u{0661}",
            "\u{0660}a",
            "999999999999999999999999999",
            "999999999999999999999999999a",
            "foo000",
            "foo000a",
            "foo0b",
        ];
        for ignore_case in [false, true] {
            // Reflexivity & antisymmetry
            for &a in &samples {
                assert_eq!(compare_natural(a, a, ignore_case), Ordering::Equal);
                for &b in &samples {
                    let ab = compare_natural(a, b, ignore_case);
                    let ba = compare_natural(b, a, ignore_case);
                    assert_eq!(ab, ba.reverse(), "antisymmetry failed for {a:?} vs {b:?}");
                }
            }
            // Transitivity across all triples
            for &a in &samples {
                for &b in &samples {
                    let ab = compare_natural(a, b, ignore_case);
                    for &c in &samples {
                        let bc = compare_natural(b, c, ignore_case);
                        let ac = compare_natural(a, c, ignore_case);
                        if ab == Ordering::Less && bc == Ordering::Less {
                            assert_eq!(
                                ac,
                                Ordering::Less,
                                "transitivity (<) failed: {a:?} < {b:?} < {c:?} (ignore_case={ignore_case})"
                            );
                        }
                        if ab == Ordering::Equal && bc == Ordering::Equal {
                            assert_eq!(
                                ac,
                                Ordering::Equal,
                                "transitivity (==) failed: {a:?} == {b:?} == {c:?} (ignore_case={ignore_case})"
                            );
                        }
                    }
                }
            }
            // Verify Rust `slice::sort_by` (driftsort) never panics on any permutation
            let mut list: Vec<&str> = samples
                .iter()
                .copied()
                .cycle()
                .take(samples.len() * 3)
                .collect();
            list.sort_by(|a, b| compare_natural(a, b, ignore_case));
        }
    }
}
