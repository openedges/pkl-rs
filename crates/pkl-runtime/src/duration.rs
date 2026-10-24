//! Pkl 0.32.0 Duration payload, conversion, comparison and ISO formatting.
use crate::value_identity::Token;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DurationUnit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
    Days,
}

impl DurationUnit {
    pub const ALL: [Self; 7] = [
        Self::Nanoseconds,
        Self::Microseconds,
        Self::Milliseconds,
        Self::Seconds,
        Self::Minutes,
        Self::Hours,
        Self::Days,
    ];

    pub fn parse(symbol: &str) -> Option<Self> {
        Some(match symbol {
            "ns" => Self::Nanoseconds,
            "us" => Self::Microseconds,
            "ms" => Self::Milliseconds,
            "s" => Self::Seconds,
            "min" => Self::Minutes,
            "h" => Self::Hours,
            "d" => Self::Days,
            _ => return None,
        })
    }
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Nanoseconds => "ns",
            Self::Microseconds => "us",
            Self::Milliseconds => "ms",
            Self::Seconds => "s",
            Self::Minutes => "min",
            Self::Hours => "h",
            Self::Days => "d",
        }
    }
    pub(crate) fn symbol_value(self) -> crate::value::PklString {
        // DurationNodes.unit returns the enum's retained String, rather than a
        // new equal string. Only these seven Java-owned symbols are shared;
        // authored strings keep their existing literal/dynamic identities.
        static SYMBOLS: std::sync::OnceLock<[crate::value::PklString; 7]> =
            std::sync::OnceLock::new();
        SYMBOLS.get_or_init(|| ["ns", "us", "ms", "s", "min", "h", "d"].map(Into::into))
            [self as usize]
            .clone()
    }
    fn nanos(self) -> f64 {
        match self {
            Self::Nanoseconds => 1.0,
            Self::Microseconds => 1_000.0,
            Self::Milliseconds => 1_000_000.0,
            Self::Seconds => 1_000_000_000.0,
            Self::Minutes => 60_000_000_000.0,
            Self::Hours => 3_600_000_000_000.0,
            Self::Days => 86_400_000_000_000.0,
        }
    }
}

/// A Java VmDuration is always a reference, including when produced from Int.
/// Cloning retains that reference independently of the immutable payload.
#[derive(Clone, Copy, Debug)]
pub struct Duration {
    value: f64,
    unit: DurationUnit,
    reference: Token,
}
impl Duration {
    pub fn new(value: f64, unit: DurationUnit) -> Self {
        Self {
            value,
            unit,
            reference: Token::fresh(),
        }
    }
    pub fn value(self) -> f64 {
        self.value
    }
    pub fn unit(self) -> DurationUnit {
        self.unit
    }
    pub(crate) fn reference(self) -> Token {
        self.reference
    }
    pub fn value_in(self, unit: DurationUnit) -> f64 {
        // VmDuration.getValue multiplies before dividing, even for the same
        // unit. Cancelling factors changes rounding and overflow behavior.
        self.value * self.unit.nanos() / unit.nanos()
    }
    pub fn to_unit(self, unit: DurationUnit) -> Self {
        Self::new(self.value_in(unit), unit)
    }
    pub(crate) fn nanos(self) -> f64 {
        self.value_in(DurationUnit::Nanoseconds)
    }
    pub(crate) fn common_values(self, other: Self) -> (f64, f64, DurationUnit) {
        if self.unit <= other.unit {
            (self.value_in(other.unit), other.value, other.unit)
        } else {
            (self.value, other.value_in(self.unit), self.unit)
        }
    }
    pub(crate) fn compare(self, other: Self) -> Ordering {
        let (left, right, _) = self.common_values(other);
        if left < right {
            return Ordering::Less;
        }
        if left > right {
            return Ordering::Greater;
        }
        // Java Double.compare distinguishes signed zero and places every NaN
        // above positive infinity, after canonicalizing its payload.
        let bits = |value: f64| {
            if value.is_nan() {
                0x7ff8_0000_0000_0000i64
            } else {
                value.to_bits() as i64
            }
        };
        bits(left).cmp(&bits(right))
    }
    pub fn iso_string(self) -> Option<String> {
        // DurationUtils deliberately divides the unit factor first. This must
        // remain separate from value_in, including for sub-nanosecond inputs.
        let total_seconds = self.value * (self.unit.nanos() / 1e9);
        if !total_seconds.is_finite() {
            return None;
        }
        let absolute_seconds = total_seconds.abs();
        // Rust's saturating casts match Java double-to-long narrowing, also
        // for NaN caused by the extreme-value fractional subtraction below.
        let hours = (absolute_seconds / 3600.0) as i64;
        let minutes = (absolute_seconds / 60.0) as i64 % 60;
        let seconds = (absolute_seconds % 60.0) as i64;
        let nanos = (absolute_seconds * 1_000_000_000.0
            - absolute_seconds.floor() * 1_000_000_000.0) as i64;
        let mut result = String::new();
        if total_seconds < 0.0 {
            result.push('-');
        }
        result.push_str("PT");
        if hours != 0 {
            result.push_str(&format!("{hours}H"));
        }
        if minutes != 0 {
            result.push_str(&format!("{minutes}M"));
        }
        if seconds != 0 || nanos != 0 || total_seconds == 0.0 {
            result.push_str(&seconds.to_string());
            if nanos != 0 {
                result.push('.');
                let fraction = format!("{nanos:09}");
                // This removes zeros only from the ISO decimal fraction, as
                // prescribed by DurationUtils; no diagnostic/output cleanup.
                result.push_str(fraction.trim_end_matches('0'));
            }
            result.push('S');
        }
        Some(result)
    }
    pub(crate) fn java_hash(self) -> i32 {
        let nanos = self.nanos();
        let bits = if nanos.is_nan() {
            0x7ff8000000000000
        } else {
            nanos.to_bits()
        };
        (bits ^ (bits >> 32)) as i32
    }
}
impl PartialEq for Duration {
    fn eq(&self, other: &Self) -> bool {
        self.reference == other.reference || self.nanos() == other.nanos()
    }
}
impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // DurationUtils.toPklString uses Java's saturating double-to-long cast
        // for all finite integral doubles, including negative zero and values
        // outside the signed long range.
        let value = if self.value.is_finite() && self.value.fract() == 0.0 {
            (self.value as i64).to_string()
        } else {
            crate::numeric::double_to_string(self.value)
        };
        write!(f, "{value}.{}", self.unit.symbol())
    }
}
