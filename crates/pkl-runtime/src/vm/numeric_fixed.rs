//! The pinned JDK 21 DecimalFormat/DigitList fixed-point path.
//!
//! Pkl disables grouping, hence DecimalFormat's optimized path is disabled.
//! DigitList uses the legacy FloatingDecimal converter, including its exactness
//! and prior-rounding flags; neither Rust fixed formatting nor Double.toString
//! has that contract. The bounded big integer below only supports this converter.
use std::cmp::Ordering;

pub(super) fn integer(value: i64, scale: usize) -> String {
    let mut result = value.to_string();
    if scale != 0 {
        result.push('.');
        result.extend(std::iter::repeat_n('0', scale));
    }
    result
}

pub(super) fn floating(value: f64, scale: usize) -> String {
    if !value.is_finite() {
        return crate::numeric::double_to_string(value);
    }
    let mut decimal = if value == 0.0 {
        Decimal {
            digits: vec![],
            at: 0,
            rounded: false,
            exact: true,
        }
    } else {
        convert(value.abs())
    };
    decimal.fixed(scale as i32);
    let mut result = String::new();
    if value.is_sign_negative() {
        result.push('-');
    }
    if decimal.digits.is_empty() || decimal.at <= 0 {
        result.push('0');
    } else {
        for i in 0..decimal.at as usize {
            result.push(char::from(
                b'0' + decimal.digits.get(i).copied().unwrap_or(0),
            ));
        }
    }
    if scale != 0 {
        result.push('.');
        for i in 0..scale as i32 {
            let index = decimal.at + i;
            let digit = if index < 0 {
                0
            } else {
                decimal.digits.get(index as usize).copied().unwrap_or(0)
            };
            result.push(char::from(b'0' + digit));
        }
    }
    result
}

struct Decimal {
    digits: Vec<u8>,
    at: i32,
    rounded: bool,
    exact: bool,
}
impl Decimal {
    fn should_round(&self, keep: usize) -> bool {
        let Some(&digit) = self.digits.get(keep) else {
            return false;
        };
        if digit != 5 {
            return digit > 5;
        }
        if keep == self.digits.len() - 1 {
            if self.rounded {
                false
            } else if !self.exact {
                true
            } else {
                keep > 0 && !self.digits[keep - 1].is_multiple_of(2)
            }
        } else {
            self.digits[keep + 1..].iter().any(|&d| d != 0)
        }
    }
    fn fixed(&mut self, scale: i32) {
        if -self.at > scale {
            self.digits.clear();
            return;
        }
        if -self.at == scale {
            if self.should_round(0) {
                self.digits = vec![1];
                self.at += 1;
            } else {
                self.digits.clear();
            }
            return;
        }
        self.trim();
        let keep = scale + self.at;
        if keep >= 0 && (keep as usize) < self.digits.len() {
            let mut count = keep as usize;
            if self.should_round(count) {
                loop {
                    if count == 0 {
                        self.digits[0] = 1;
                        self.at += 1;
                        count = 1;
                        break;
                    }
                    count -= 1;
                    self.digits[count] += 1;
                    if self.digits[count] <= 9 {
                        count += 1;
                        break;
                    }
                }
            }
            self.digits.truncate(count);
            self.trim();
        }
    }
    fn trim(&mut self) {
        while self.digits.len() > 1 && self.digits.last() == Some(&0) {
            self.digits.pop();
        }
    }
    fn round_conversion(&mut self) {
        let mut i = self.digits.len() - 1;
        while self.digits[i] == 9 && i > 0 {
            self.digits[i] = 0;
            i -= 1;
        }
        if self.digits[i] == 9 {
            self.at += 1;
            self.digits[0] = 1;
            // Pinned roundup() returns before setting the rounded-up flag.
            return;
        }
        self.digits[i] += 1;
        self.rounded = true;
    }
}

fn n5bits(n: i32) -> i32 {
    if n == 0 {
        0
    } else if n < 27 {
        (64 - 5_u64.pow(n as u32).leading_zeros()) as i32
    } else {
        n * 3
    }
}

fn convert(value: f64) -> Decimal {
    let bits = value.to_bits();
    let mut fraction = bits & ((1_u64 << 52) - 1);
    let mut exponent = ((bits >> 52) & 2047) as i32;
    let significant;
    if exponent == 0 {
        let leading = fraction.leading_zeros() as i32;
        let shift = leading - 11;
        fraction <<= shift;
        exponent = 1 - shift;
        significant = 64 - leading;
    } else {
        fraction |= 1_u64 << 52;
        significant = 53;
    }
    exponent -= 1023;
    let trailing = fraction.trailing_zeros() as i32;
    let fraction_bits = 53 - trailing;
    let tiny = (fraction_bits - exponent - 1).max(0);
    if (-21..=62).contains(&exponent) && tiny == 0 {
        let insignificant = if exponent > significant {
            let power = exponent - significant - 1;
            if power > 1 {
                (1_u64 << power).ilog10()
            } else {
                0
            }
        } else {
            0
        };
        let mut integer = if exponent >= 52 {
            fraction << (exponent - 52)
        } else {
            fraction >> (52 - exponent)
        };
        let divisor = 10_u64.pow(insignificant);
        let remainder = integer % divisor;
        integer /= divisor;
        if insignificant != 0 && remainder >= divisor / 2 {
            integer += 1;
        }
        let text = integer.to_string();
        let mut result = Decimal {
            digits: text.bytes().map(|b| b - b'0').collect(),
            at: text.len() as i32 + insignificant as i32,
            rounded: false,
            exact: false,
        };
        result.trim();
        return result;
    }
    let d2 = f64::from_bits((1023_u64 << 52) | (fraction & ((1_u64 << 52) - 1)));
    // Keep FloatingDecimal.estimateDecExp's JDK 21 approximation exactly;
    // LOG10_2 has different bits and would change the conversion algorithm.
    #[allow(clippy::approx_constant)]
    let estimate = (d2 - 1.5) * 0.289529654 + 0.176091259 + f64::from(exponent) * 0.301029995663981;
    let mut dec_exp = estimate.floor() as i32;
    let b5 = (-dec_exp).max(0);
    let mut b2 = b5 + tiny + exponent;
    let s5 = dec_exp.max(0);
    let mut s2 = s5 + tiny;
    let m5 = b5;
    let mut m2 = b2 - significant;
    fraction >>= trailing;
    b2 -= fraction_bits - 1;
    let common = b2.min(s2);
    b2 -= common;
    s2 -= common;
    m2 -= common;
    if fraction_bits == 1 {
        m2 -= 1;
    }
    if m2 < 0 {
        b2 -= m2;
        s2 -= m2;
        m2 = 0;
    }
    let b_bits = fraction_bits + b2 + n5bits(b5);
    let ten_s_bits = s2 + 1 + n5bits(s5 + 1);
    let mut digits = Vec::new();
    let (low, high, difference, exact) = if b_bits < 64 && ten_s_bits < 64 {
        // Reproduce the JVM's signed int/long arithmetic, including overflow in
        // the stopping comparison and its explicit margin-overflow branch.
        let narrow = b_bits < 32 && ten_s_bits < 32;
        let wrap = |n: i64| if narrow { i64::from(n as i32) } else { n };
        let mut b = wrap((fraction * 5_u64.pow(b5 as u32)) as i64).wrapping_shl(b2 as u32);
        b = wrap(b);
        let s = wrap((5_i64.pow(s5 as u32)).wrapping_shl(s2 as u32));
        let mut m = wrap((5_i64.pow(m5 as u32)).wrapping_shl(m2 as u32));
        let ten_s = wrap(s.wrapping_mul(10));
        let q = b / s;
        b = wrap((b % s).wrapping_mul(10));
        m = wrap(m.wrapping_mul(10));
        let mut low = b < m;
        let mut high = wrap(b.wrapping_add(m)) > ten_s;
        if q == 0 && !high {
            dec_exp -= 1;
        } else {
            digits.push(q as u8);
        }
        if !(-3..8).contains(&dec_exp) {
            low = false;
            high = false;
        }
        while !low && !high {
            let q = b / s;
            b = wrap((b % s).wrapping_mul(10));
            m = wrap(m.wrapping_mul(10));
            if m > 0 {
                low = b < m;
                high = wrap(b.wrapping_add(m)) > ten_s;
            } else {
                low = true;
                high = true;
            }
            digits.push(q as u8);
        }
        (
            low,
            high,
            wrap(wrap(b.wrapping_shl(1)).wrapping_sub(ten_s)).cmp(&0),
            b == 0,
        )
    } else {
        let s = Big::scaled(1, s5, s2);
        let mut b = Big::scaled(fraction, b5, b2);
        let mut m = Big::scaled(1, m5 + 1, m2 + 1);
        let ten_s = Big::scaled(1, s5 + 1, s2 + 1);
        let q = b.quotient_iteration(&s);
        let mut low = b.cmp(&m) == Ordering::Less;
        let mut high = b.plus(&m).cmp(&ten_s) != Ordering::Less;
        if q == 0 && !high {
            dec_exp -= 1;
        } else {
            digits.push(q);
        }
        if !(-3..8).contains(&dec_exp) {
            low = false;
            high = false;
        }
        while !low && !high {
            digits.push(b.quotient_iteration(&s));
            m.multiply(10);
            low = b.cmp(&m) == Ordering::Less;
            high = b.plus(&m).cmp(&ten_s) != Ordering::Less;
        }
        let exact = b.0.is_empty();
        b.multiply(2);
        (low, high, b.cmp(&ten_s), exact)
    };
    let mut result = Decimal {
        digits,
        at: dec_exp + 1,
        rounded: false,
        exact,
    };
    if high
        && (!low
            || difference == Ordering::Greater
            || (difference == Ordering::Equal && result.digits.last().is_some_and(|d| d % 2 != 0)))
    {
        result.round_conversion();
    }
    result
}

#[derive(Clone)]
struct Big(Vec<u32>);
impl Big {
    fn scaled(value: u64, fives: i32, twos: i32) -> Self {
        debug_assert!(fives >= 0 && twos >= 0);
        let mut result = Self(vec![value as u32, (value >> 32) as u32]);
        result.normalize();
        for _ in 0..fives {
            result.multiply(5);
        }
        for _ in 0..twos {
            result.multiply(2);
        }
        result
    }
    fn normalize(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }
    fn multiply(&mut self, by: u32) {
        let mut carry = 0_u64;
        for word in &mut self.0 {
            let product = u64::from(*word) * u64::from(by) + carry;
            *word = product as u32;
            carry = product >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
    }
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }
    fn plus(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.0.resize(self.0.len().max(other.0.len()), 0);
        let mut carry = 0_u64;
        for (i, word) in result.0.iter_mut().enumerate() {
            let sum = u64::from(*word) + u64::from(other.0.get(i).copied().unwrap_or(0)) + carry;
            *word = sum as u32;
            carry = sum >> 32;
        }
        if carry != 0 {
            result.0.push(carry as u32);
        }
        result
    }
    fn subtract(&mut self, other: &Self) {
        let mut borrow = 0_u64;
        for (i, word) in self.0.iter_mut().enumerate() {
            let sub = u64::from(other.0.get(i).copied().unwrap_or(0)) + borrow;
            let old = u64::from(*word);
            *word = old.wrapping_sub(sub) as u32;
            borrow = u64::from(old < sub);
        }
        debug_assert_eq!(borrow, 0);
        self.normalize();
    }
    fn quotient_iteration(&mut self, denominator: &Self) -> u8 {
        let mut quotient = 0;
        while self.cmp(denominator) != Ordering::Less {
            self.subtract(denominator);
            quotient += 1;
        }
        debug_assert!(quotient < 10);
        self.multiply(10);
        quotient
    }
}
