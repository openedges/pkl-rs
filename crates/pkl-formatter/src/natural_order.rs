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

        if is_java_digit(c1) && is_java_digit(c2) {
            let (n1, next_i) = get_number(&u1, i);
            let (n2, next_j) = get_number(&u2, j);
            let cmp = n1.cmp(&n2);
            if cmp != Ordering::Equal {
                return cmp;
            }
            i = next_i;
            j = next_j;
        } else {
            let cmp = c1.cmp(&c2);
            if cmp != Ordering::Equal {
                return cmp;
            }
            i += 1;
            j += 1;
        }
    }

    u1.len().cmp(&u2.len())
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
