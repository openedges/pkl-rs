//! Duration scalar storage and Pkl 0.32.0 unit/display/equality semantics.
//! Arithmetic, unit conversion methods and ISO formatting are separate operations.
use crate::value_identity::Token;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub(crate) fn nanos(self) -> f64 {
        self.value * self.unit.nanos()
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
