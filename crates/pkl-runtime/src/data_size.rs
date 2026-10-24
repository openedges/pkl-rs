//! Pkl 0.32.0 DataSize payload, unit conversion and reference semantics.
use crate::value_identity::Token;
use std::cmp::Ordering;

/// Declaration order is observable: mixed-unit operations use the later unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DataSizeUnit {
    Bytes,
    Kilobytes,
    Kibibytes,
    Megabytes,
    Mebibytes,
    Gigabytes,
    Gibibytes,
    Terabytes,
    Tebibytes,
    Petabytes,
    Pebibytes,
}

impl DataSizeUnit {
    pub const ALL: [Self; 11] = [
        Self::Bytes,
        Self::Kilobytes,
        Self::Kibibytes,
        Self::Megabytes,
        Self::Mebibytes,
        Self::Gigabytes,
        Self::Gibibytes,
        Self::Terabytes,
        Self::Tebibytes,
        Self::Petabytes,
        Self::Pebibytes,
    ];

    pub fn parse(symbol: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|unit| unit.symbol() == symbol)
    }

    pub fn symbol(self) -> &'static str {
        [
            "b", "kb", "kib", "mb", "mib", "gb", "gib", "tb", "tib", "pb", "pib",
        ][self as usize]
    }

    pub fn bytes(self) -> u64 {
        [
            1,
            1_000,
            1_024,
            1_000_000,
            1_048_576,
            1_000_000_000,
            1_073_741_824,
            1_000_000_000_000,
            1_099_511_627_776,
            1_000_000_000_000_000,
            1_125_899_906_842_624,
        ][self as usize]
    }

    pub fn is_binary(self) -> bool {
        (self as usize).is_multiple_of(2)
    }

    pub fn is_decimal(self) -> bool {
        self == Self::Bytes || !self.is_binary()
    }

    pub(crate) fn symbol_value(self) -> crate::value::PklString {
        // DataSizeNodes.unit exposes the enum's retained Java String.
        static SYMBOLS: std::sync::OnceLock<[crate::value::PklString; 11]> =
            std::sync::OnceLock::new();
        SYMBOLS.get_or_init(|| Self::ALL.map(|unit| unit.symbol().into()))[self as usize].clone()
    }
}

/// Copies preserve the same VmDataSize reference; operations allocate a new one.
#[derive(Clone, Copy, Debug)]
pub struct DataSize {
    value: f64,
    unit: DataSizeUnit,
    reference: Token,
}

impl DataSize {
    pub fn new(value: f64, unit: DataSizeUnit) -> Self {
        Self {
            value,
            unit,
            reference: Token::fresh(),
        }
    }

    pub fn value(self) -> f64 {
        self.value
    }
    pub fn unit(self) -> DataSizeUnit {
        self.unit
    }
    pub(crate) fn reference(self) -> Token {
        self.reference
    }

    pub fn value_in(self, unit: DataSizeUnit) -> f64 {
        // Do not cancel equal unit factors: even a same-unit conversion can
        // round, overflow, or preserve a signed zero through these two steps.
        self.value * self.unit.bytes() as f64 / unit.bytes() as f64
    }

    pub fn to_unit(self, unit: DataSizeUnit) -> Self {
        Self::new(self.value_in(unit), unit)
    }

    pub fn to_binary_unit(self) -> Self {
        if self.unit.is_binary() {
            self
        } else {
            self.to_unit(DataSizeUnit::ALL[self.unit as usize + 1])
        }
    }

    pub fn to_decimal_unit(self) -> Self {
        if self.unit.is_decimal() {
            self
        } else {
            self.to_unit(DataSizeUnit::ALL[self.unit as usize - 1])
        }
    }

    pub(crate) fn common_values(self, other: Self) -> (f64, f64, DataSizeUnit) {
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
        // Double.compare canonicalizes all NaNs and then compares signed bits;
        // unlike numerical equality, -0.0 is below +0.0 and NaN is above +∞.
        let bits = |value: f64| {
            if value.is_nan() {
                0x7ff8_0000_0000_0000i64
            } else {
                value.to_bits() as i64
            }
        };
        bits(left).cmp(&bits(right))
    }

    pub(crate) fn java_hash(self) -> i32 {
        let bytes = self.value_in(DataSizeUnit::Bytes);
        let bits = if bytes.is_nan() {
            0x7ff8_0000_0000_0000
        } else {
            bytes.to_bits()
        };
        (bits ^ (bits >> 32)) as i32
    }
}

impl PartialEq for DataSize {
    fn eq(&self, other: &Self) -> bool {
        self.reference == other.reference
            || self.value_in(DataSizeUnit::Bytes) == other.value_in(DataSizeUnit::Bytes)
    }
}

impl std::fmt::Display for DataSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = if self.value.is_finite() && self.value.fract() == 0.0 {
            (self.value as i64).to_string()
        } else {
            crate::numeric::double_to_string(self.value)
        };
        write!(f, "{value}.{}", self.unit.symbol())
    }
}
