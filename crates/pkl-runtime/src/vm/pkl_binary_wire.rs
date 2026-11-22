//! In-memory MessagePack encoding for the shared resumable pkl-binary renderer.
use crate::value::{PklString, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WireError {
    LengthOverflow,
    Allocation,
    /// Heap objects require the VM's lazy/converted traversal and source identity.
    RequiresVmTraversal,
}

#[derive(Default)]
pub(super) struct BinaryWire {
    bytes: Vec<u8>,
}

impl BinaryWire {
    pub(super) fn with_capacity(cap: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(cap),
        }
    }

    fn put(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| WireError::Allocation)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    pub(super) fn raw(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        self.put(bytes)
    }

    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub(super) fn null(&mut self) -> Result<(), WireError> {
        self.put(&[0xc0])
    }

    pub(super) fn boolean(&mut self, value: bool) -> Result<(), WireError> {
        self.put(&[if value { 0xc3 } else { 0xc2 }])
    }

    /// MessagePacker.packLong chooses compact signed/unsigned representations.
    pub(super) fn integer(&mut self, value: i64) -> Result<(), WireError> {
        if (0..=127).contains(&value) || (-32..=-1).contains(&value) {
            return self.put(&[value as u8]);
        }
        if value >= 0 {
            let value = value as u64;
            if value <= u8::MAX.into() {
                self.put(&[0xcc, value as u8])
            } else if value <= u16::MAX.into() {
                self.put(&[0xcd])?;
                self.put(&(value as u16).to_be_bytes())
            } else if value <= u32::MAX.into() {
                self.put(&[0xce])?;
                self.put(&(value as u32).to_be_bytes())
            } else {
                self.put(&[0xcf])?;
                self.put(&value.to_be_bytes())
            }
        } else if value >= i8::MIN.into() {
            self.put(&[0xd0, value as u8])
        } else if value >= i16::MIN.into() {
            self.put(&[0xd1])?;
            self.put(&(value as i16).to_be_bytes())
        } else if value >= i32::MIN.into() {
            self.put(&[0xd2])?;
            self.put(&(value as i32).to_be_bytes())
        } else {
            self.put(&[0xd3])?;
            self.put(&value.to_be_bytes())
        }
    }

    pub(super) fn float(&mut self, value: f64) -> Result<(), WireError> {
        self.put(&[0xcb])?;
        self.put(&value.to_bits().to_be_bytes())
    }

    fn len32(length: usize) -> Result<u32, WireError> {
        u32::try_from(length).map_err(|_| WireError::LengthOverflow)
    }

    pub(super) fn array(&mut self, length: usize) -> Result<(), WireError> {
        let value = Self::len32(length)?;
        if value <= 15 {
            self.put(&[0x90 | value as u8])
        } else if value <= u16::MAX.into() {
            self.put(&[0xdc])?;
            self.put(&(value as u16).to_be_bytes())
        } else {
            self.put(&[0xdd])?;
            self.put(&value.to_be_bytes())
        }
    }

    pub(super) fn map(&mut self, length: usize) -> Result<(), WireError> {
        let value = Self::len32(length)?;
        if value <= 15 {
            self.put(&[0x80 | value as u8])
        } else if value <= u16::MAX.into() {
            self.put(&[0xde])?;
            self.put(&(value as u16).to_be_bytes())
        } else {
            self.put(&[0xdf])?;
            self.put(&value.to_be_bytes())
        }
    }

    pub(super) fn str_bytes(&mut self, value: &str) -> Result<(), WireError> {
        let bytes = value.as_bytes();
        let length = Self::len32(bytes.len())?;
        if length <= 31 {
            self.put(&[0xa0 | length as u8])?;
        } else if length <= u8::MAX.into() {
            self.put(&[0xd9, length as u8])?;
        } else if length <= u16::MAX.into() {
            self.put(&[0xda])?;
            self.put(&(length as u16).to_be_bytes())?;
        } else {
            self.put(&[0xdb])?;
            self.put(&length.to_be_bytes())?;
        }
        self.put(bytes)
    }

    pub(super) fn string(&mut self, value: &PklString) -> Result<(), WireError> {
        // Java UTF-8 replacement and MessagePack string behavior require actual
        // oracle verification, including isolated UTF-16 surrogate controls.
        if let Some(s) = value.as_str() {
            return self.str_bytes(s);
        }
        let owned = value.to_java_utf8();
        self.str_bytes(&owned)
    }

    pub(super) fn binary(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        let length = Self::len32(bytes.len())?;
        if length <= u8::MAX.into() {
            self.put(&[0xc4, length as u8])?;
        } else if length <= u16::MAX.into() {
            self.put(&[0xc5])?;
            self.put(&(length as u16).to_be_bytes())?;
        } else {
            self.put(&[0xc6])?;
            self.put(&length.to_be_bytes())?;
        }
        self.put(bytes)
    }

    /// Encodes scalar cases whose entire data is available without forcing.
    /// Unsupported heap values do not append any bytes.
    pub(super) fn scalar(&mut self, value: &Value) -> Result<(), WireError> {
        match value {
            Value::Null | Value::NullWithDefault(_) => self.null(),
            Value::Bool(value) => self.boolean(*value),
            Value::Int(value) => self.integer(**value),
            Value::Float(value) => self.float(**value),
            Value::String(value) => self.string(value),
            Value::Duration(value) => {
                self.array(3)?;
                self.integer(0x07)?;
                self.float(value.value())?;
                self.string(&PklString::from(value.unit().symbol()))
            }
            Value::DataSize(value) => {
                self.array(3)?;
                self.integer(0x08)?;
                self.float(value.value())?;
                self.string(&PklString::from(value.unit().symbol()))
            }
            Value::Bytes(value) => {
                self.array(2)?;
                self.integer(0x0f)?;
                self.binary(value.as_ref())
            }
            Value::IntSeq(value) => {
                self.array(4)?;
                self.integer(0x0a)?;
                self.integer(value.start())?;
                self.integer(value.end())?;
                self.integer(value.step())
            }
            Value::Regex(value) => {
                self.array(2)?;
                self.integer(0x0b)?;
                self.string(value.pattern())
            }
            Value::Function(_) => {
                self.array(1)?;
                self.integer(0x0e)
            }
            Value::Object(_) | Value::Class(_) | Value::TypeAlias(_) => {
                Err(WireError::RequiresVmTraversal)
            }
        }
    }
}

impl From<WireError> for crate::value::RuntimeError {
    fn from(error: WireError) -> Self {
        use crate::value::{ErrorKind, RuntimeError};
        match error {
            WireError::RequiresVmTraversal => RuntimeError::new(
                ErrorKind::InvalidBytecode,
                "pkl-binary scalar dispatch received a heap value",
            ),
            WireError::LengthOverflow => RuntimeError::new(
                ErrorKind::Unsupported,
                "pkl-binary MessagePack length exceeds u32; pinned Java overflow behavior remains unverified",
            ),
            WireError::Allocation => RuntimeError::new(
                ErrorKind::Unsupported,
                "pkl-binary output allocation failed; Java allocation-failure equivalence remains unverified",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These are MessagePack boundary controls, not Java differential results.
    #[test]
    fn signed_and_unsigned_integer_boundaries_use_compact_encodings() {
        let values: &[(i64, &[u8])] = &[
            (-33, &[0xd0, 0xdf]),
            (-32, &[0xe0]),
            (-1, &[0xff]),
            (0, &[0x00]),
            (127, &[0x7f]),
            (128, &[0xcc, 0x80]),
            (255, &[0xcc, 0xff]),
            (256, &[0xcd, 0x01, 0x00]),
            (-129, &[0xd1, 0xff, 0x7f]),
            (65536, &[0xce, 0x00, 0x01, 0x00, 0x00]),
        ];
        for (value, expected) in values {
            let mut writer = BinaryWire::default();
            writer.integer(*value).unwrap();
            assert_eq!(writer.into_bytes(), *expected);
        }
    }

    #[test]
    fn container_headers_and_binary_payload_keep_lengths_and_bytes() {
        let mut writer = BinaryWire::default();
        writer.array(2).unwrap();
        writer.integer(0x0f).unwrap();
        writer.binary(&[0, 127, 128, 255]).unwrap();
        assert_eq!(writer.into_bytes(), [0x92, 0x0f, 0xc4, 4, 0, 127, 128, 255]);
        let mut writer = BinaryWire::default();
        writer.array(16).unwrap();
        writer.map(16).unwrap();
        assert_eq!(writer.into_bytes(), [0xdc, 0, 16, 0xde, 0, 16]);
    }

    #[test]
    fn floating_bits_and_string_lengths_are_not_text_normalized() {
        let mut writer = BinaryWire::default();
        writer.float(-0.0).unwrap();
        writer.string(&PklString::from("x\0y")).unwrap();
        assert_eq!(
            writer.into_bytes(),
            [0xcb, 0x80, 0, 0, 0, 0, 0, 0, 0, 0xa3, b'x', 0, b'y']
        );
    }
}
