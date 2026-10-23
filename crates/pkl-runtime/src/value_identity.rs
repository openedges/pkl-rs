//! Semantic Java references are independent of immutable payload storage.
//! A numeric value can occupy an unboxed frame slot or carry a boxed reference.
use std::{
    fmt,
    num::NonZeroU64,
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::value::ObjectId;

// Reserved tokens model the pinned JDK Long/Boolean caches. Heap handles are
// compared separately and never inflate every scalar's stored representation.
static NEXT_REFERENCE: AtomicU64 = AtomicU64::new(261);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reference {
    Scalar(Token),
    Heap(ObjectId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Token(NonZeroU64);
impl Token {
    pub(crate) fn fresh() -> Self {
        let id = NEXT_REFERENCE.fetch_add(1, Ordering::Relaxed);
        assert!(id != u64::MAX, "semantic reference identity exhausted");
        Self(NonZeroU64::new(id).expect("nonzero semantic reference"))
    }
    fn cached_long(value: i8) -> Self {
        Self(NonZeroU64::new((i64::from(value) + 129) as u64).unwrap())
    }
    pub(crate) fn boolean(value: bool) -> Self {
        Self(NonZeroU64::new(257 + u64::from(value)).unwrap())
    }
    pub(crate) fn empty_string() -> Self {
        Self(NonZeroU64::new(260).unwrap())
    }
    pub(crate) fn null() -> Self {
        Self(NonZeroU64::new(259).unwrap())
    }
}

/// An experimental Rust value carrier. Equality compares the numeric payload;
/// reference provenance is private to the shared evaluator.
#[derive(Clone, Copy)]
pub struct Number<T> {
    value: T,
    reference: Option<Token>,
}
impl<T: Copy> Number<T> {
    pub fn get(self) -> T {
        self.value
    }
    pub(crate) fn reference(self) -> Option<Token> {
        self.reference
    }
    pub(crate) fn unboxed(self) -> Self {
        Self {
            value: self.value,
            reference: None,
        }
    }
}
impl<T> From<T> for Number<T> {
    fn from(value: T) -> Self {
        Self {
            value,
            reference: None,
        }
    }
}
impl Number<i64> {
    pub(crate) fn boxed(mut self) -> Self {
        if self.reference.is_none() {
            self.reference = Some(match i8::try_from(self.value) {
                Ok(value) => Token::cached_long(value),
                Err(_) => Token::fresh(),
            });
        }
        self
    }
}
impl Number<f64> {
    pub(crate) fn boxed(mut self) -> Self {
        self.reference.get_or_insert_with(Token::fresh);
        self
    }
}
impl<T> Deref for Number<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T: PartialEq> PartialEq for Number<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl<T: Eq> Eq for Number<T> {}
impl<T: PartialOrd> PartialOrd for Number<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}
macro_rules! raw_comparison {
    ($type:ty) => {
        impl PartialEq<$type> for Number<$type> {
            fn eq(&self, other: &$type) -> bool {
                self.value == *other
            }
        }
        impl PartialOrd<$type> for Number<$type> {
            fn partial_cmp(&self, other: &$type) -> Option<std::cmp::Ordering> {
                self.value.partial_cmp(other)
            }
        }
    };
}
raw_comparison!(i64);
raw_comparison!(f64);
impl<T: fmt::Debug> fmt::Debug for Number<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}
impl<T: fmt::Display> fmt::Display for Number<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

/// A VmBytes wrapper can share storage with another wrapper without sharing
/// reference identity. Cloning this carrier preserves the existing wrapper.
#[derive(Clone)]
pub struct Bytes {
    data: Arc<[u8]>,
    reference: Token,
}
impl Bytes {
    pub fn into_data(self) -> Arc<[u8]> {
        self.data
    }
    pub(crate) fn reference(&self) -> Token {
        self.reference
    }
}
impl From<Arc<[u8]>> for Bytes {
    fn from(data: Arc<[u8]>) -> Self {
        Self {
            data,
            reference: Token::fresh(),
        }
    }
}
impl From<Vec<u8>> for Bytes {
    fn from(data: Vec<u8>) -> Self {
        Self::from(Arc::<[u8]>::from(data))
    }
}
impl<const N: usize> From<[u8; N]> for Bytes {
    fn from(data: [u8; N]) -> Self {
        Self::from(Arc::<[u8]>::from(data))
    }
}
impl Deref for Bytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.data
    }
}
impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}
impl PartialEq for Bytes {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}
impl Eq for Bytes {}
impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}
