//! Pinned Long.parseLong / Double.parseDouble grammar after Pkl underscore removal.
use super::*;

fn number_units(text: &PklString) -> Vec<u16> {
    let original = text.encode_utf16().collect::<Vec<_>>();
    let mut result = Vec::with_capacity(original.len());
    let mut number_start = true;
    for &unit in &original {
        if unit != b'_' as u16 {
            result.push(unit);
        } else if number_start {
            return original;
        }
        number_start = matches!(unit, 0x2e | 0x65 | 0x45);
    }
    result
}
pub(in crate::vm) fn decimal_digit(unit: u16) -> Option<i64> {
    let point = u32::from(unit);
    if java_regex::utf16::character_record(point)?.category != 9 {
        return None;
    }
    // Every BMP Decimal_Number run is a sequence of complete 0..9 alphabets.
    // This intentionally processes UTF-16 chars: supplementary digits fail Java Long.
    let mut start = point;
    while start > 0 && java_regex::utf16::character_record(start - 1)?.category == 9 {
        start -= 1;
    }
    Some(i64::from((point - start) % 10))
}
pub(in crate::vm) fn integer(text: &PklString) -> Option<i64> {
    let units = number_units(text);
    let (negative, start) = match units.first()? {
        0x2d => (true, 1),
        0x2b => (false, 1),
        _ => (false, 0),
    };
    if start == units.len() {
        return None;
    }
    let limit = if negative { i64::MIN } else { -i64::MAX };
    let mut result = 0i64;
    for &unit in &units[start..] {
        result = result.checked_mul(10)?.checked_sub(decimal_digit(unit)?)?;
        if result < limit {
            return None;
        }
    }
    Some(if negative { result } else { -result })
}
fn exponent(bytes: &[u8]) -> Option<i64> {
    let (negative, start) = match bytes.first()? {
        b'-' => (true, 1),
        b'+' => (false, 1),
        _ => (false, 0),
    };
    if start == bytes.len() {
        return None;
    }
    let mut value = 0i64;
    for &byte in &bytes[start..] {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value
            .saturating_mul(10)
            .saturating_add(i64::from(byte - b'0'));
    }
    Some(if negative { -value } else { value })
}
fn hexadecimal(bytes: &[u8], negative: bool) -> Option<f64> {
    let split = bytes.iter().position(|b| matches!(b, b'p' | b'P'))?;
    let power = exponent(&bytes[split + 1..])?;
    let mut bits = Vec::new();
    let mut point = None;
    let mut digits = 0i64;
    for &byte in &bytes[2..split] {
        if byte == b'.' {
            if point.replace(digits).is_some() {
                return None;
            }
            continue;
        }
        let value = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return None,
        };
        bits.extend((0..4).rev().map(|shift| (value >> shift) & 1));
        digits += 1;
    }
    if digits == 0 {
        return None;
    }
    let sign = if negative { 1u64 << 63 } else { 0 };
    let Some(first) = bits.iter().position(|&b| b != 0) else {
        return Some(f64::from_bits(sign));
    };
    // FloatingDecimal validates the complete grammar, then returns signed zero
    // for a zero significand, before parsing the unsigned exponent as an int.
    // An overflowing exponent decides infinity/zero before position adjustment.
    // The earlier saturating scan preserves this decision even for arbitrarily
    // many exponent digits; it must not compensate overflow with the significand.
    if power > i64::from(i32::MAX) {
        return Some(f64::from_bits(sign | 0x7ff0_0000_0000_0000));
    }
    if power < -i64::from(i32::MAX) {
        return Some(f64::from_bits(sign));
    }
    // The pinned implementation computes exponentAdjust with Java int
    // arithmetic, before widening to long. Preserve its wrapping behavior for
    // enormous significands without needing to construct one in a test.
    let left_digits = point.unwrap_or(digits) - (first / 4) as i64;
    let adjustment = if left_digits > 0 {
        ((left_digits - 1) as i32).wrapping_mul(4)
    } else {
        ((1 - left_digits) as i32).wrapping_mul(-4)
    };
    let mut e = power + i64::from(adjustment) + (3 - first % 4) as i64;
    if e > 1023 {
        return Some(f64::from_bits(sign | 0x7ff0_0000_0000_0000));
    }
    if e < -1075 {
        return Some(f64::from_bits(sign));
    }
    let count = if e >= -1022 { 53 } else { (e + 1075) as usize };
    let significant = &bits[first..];
    let mut mantissa = 0u64;
    for i in 0..count {
        mantissa = (mantissa << 1) | u64::from(significant.get(i).copied().unwrap_or(0));
    }
    let guard = significant.get(count).copied().unwrap_or(0) != 0;
    let sticky = significant
        .get(count + 1..)
        .is_some_and(|tail| tail.iter().any(|&b| b != 0));
    if guard && (sticky || mantissa & 1 != 0) {
        mantissa += 1;
    }
    let magnitude = if e < -1022 {
        mantissa
    } else {
        if mantissa == 1u64 << 53 {
            mantissa >>= 1;
            e += 1;
        }
        if e > 1023 {
            0x7ff0_0000_0000_0000
        } else {
            ((e + 1023) as u64) << 52 | (mantissa & 0x000f_ffff_ffff_ffff)
        }
    };
    Some(f64::from_bits(sign | magnitude))
}
pub(in crate::vm) fn float(text: &PklString) -> Option<f64> {
    let units = number_units(text);
    let first = units.iter().position(|&u| u > 0x20)?;
    let last = units.iter().rposition(|&u| u > 0x20)? + 1;
    let mut bytes = units[first..last]
        .iter()
        .map(|&u| u8::try_from(u).ok().filter(u8::is_ascii))
        .collect::<Option<Vec<_>>>()?;
    let (negative, start) = match bytes.first()? {
        b'-' => (true, 1),
        b'+' => (false, 1),
        _ => (false, 0),
    };
    match &bytes[start..] {
        b"NaN" => return Some(f64::NAN),
        b"Infinity" => {
            return Some(if negative {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            });
        }
        _ => {}
    }
    if matches!(bytes.last(), Some(b'f' | b'F' | b'd' | b'D')) {
        bytes.pop();
    }
    if bytes
        .get(start..)
        .is_some_and(|s| s.starts_with(b"0x") || s.starts_with(b"0X"))
    {
        return hexadecimal(&bytes[start..], negative);
    }
    let mut index = start;
    let mut digits = 0;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
        digits += 1;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        exponent(&bytes[index + 1..])?;
        index = bytes.len();
    }
    if index != bytes.len() {
        return None;
    }
    // The grammar above excludes Rust-only spellings. Both decimal converters
    // round the exact decimal value to binary64, nearest with ties to even.
    std::str::from_utf8(&bytes).ok()?.parse().ok()
}
pub(super) fn boolean(text: &PklString) -> Option<bool> {
    let units = text.encode_utf16().collect::<Vec<_>>();
    for (expected, value) in [(b"true".as_slice(), true), (b"false".as_slice(), false)] {
        if units.len() == expected.len()
            && units.iter().zip(expected).all(|(&unit, &ascii)| {
                let point = u32::from(unit);
                let expected = u32::from(ascii);
                if point == expected {
                    return true;
                }
                let upper = java_regex::utf16::character_record(point).map_or(point, |r| r.upper);
                let folded = java_regex::utf16::character_record(upper).map_or(upper, |r| r.lower);
                folded == expected
            })
        {
            return Some(value);
        }
    }
    None
}
