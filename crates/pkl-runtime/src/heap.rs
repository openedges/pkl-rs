//! Evaluator-owned tracing arena. No object edge uses reference counting.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Fast non-cryptographic hasher for internal VM maps whose keys are never
/// exposed in iteration order (e.g. `Object.cache`, `member_values`).
#[derive(Default)]
pub struct FxHasher(u64);

impl FxHasher {
    const K: u64 = 0x517cc1b727220a95;

    #[inline(always)]
    fn add_to_hash(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(Self::K);
    }
}

impl Hasher for FxHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    fn write(&mut self, mut bytes: &[u8]) {
        while bytes.len() >= 8 {
            let chunk = u64::from_ne_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            self.add_to_hash(chunk);
            bytes = &bytes[8..];
        }
        if bytes.len() >= 4 {
            let chunk = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            self.add_to_hash(u64::from(chunk));
            bytes = &bytes[4..];
        }
        if bytes.len() >= 2 {
            let chunk = u16::from_ne_bytes([bytes[0], bytes[1]]);
            self.add_to_hash(u64::from(chunk));
            bytes = &bytes[2..];
        }
        if let Some(&b) = bytes.first() {
            self.add_to_hash(u64::from(b));
        }
    }

    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(u64::from(i));
    }

    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(u64::from(i));
    }

    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(u64::from(i));
    }

    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i);
    }

    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i as u64);
    }
}

pub type FxBuildHasher = BuildHasherDefault<FxHasher>;
pub type FastMap<K, V> = HashMap<K, V, FxBuildHasher>;

use crate::value::{ErrorKind, ObjectId, PklString, RuntimeError, Value};

static NEXT_HEAP: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    pub receiver: ObjectId,
    pub owner: ObjectId,
}

/// Copy-on-write lexical scope stack: empty and single-scope are zero-allocation;
/// larger stacks share via `Arc` bump.
#[derive(Clone, Debug, Default)]
pub enum ScopeList {
    #[default]
    Empty,
    Single([Scope; 1]),
    Many(Arc<Vec<Scope>>),
}

impl ScopeList {
    #[inline]
    pub fn from_single(scope: Scope) -> Self {
        Self::Single([scope])
    }

    #[inline]
    pub fn as_slice(&self) -> &[Scope] {
        match self {
            Self::Empty => &[],
            Self::Single(s) => s,
            Self::Many(arc) => arc.as_slice(),
        }
    }

    #[inline]
    pub fn push(&mut self, scope: Scope) {
        match self {
            Self::Empty => *self = Self::Single([scope]),
            Self::Single([first]) => {
                let first = *first;
                *self = Self::Many(Arc::new(vec![first, scope]));
            }
            Self::Many(arc) => Arc::make_mut(arc).push(scope),
        }
    }

    #[inline]
    pub fn insert(&mut self, index: usize, scope: Scope) {
        match self {
            Self::Empty => {
                assert_eq!(index, 0);
                *self = Self::Single([scope]);
            }
            Self::Single([first]) => {
                let first = *first;
                let mut vec = Vec::with_capacity(2);
                vec.push(first);
                vec.insert(index, scope);
                *self = Self::Many(Arc::new(vec));
            }
            Self::Many(arc) => Arc::make_mut(arc).insert(index, scope),
        }
    }

    #[inline]
    pub fn set_first_receiver(&mut self, receiver: ObjectId) -> bool {
        match self {
            Self::Empty => false,
            Self::Single([first]) => {
                first.receiver = receiver;
                true
            }
            Self::Many(arc) => {
                let Some(first) = arc.first() else {
                    return false;
                };
                if first.receiver != receiver {
                    Arc::make_mut(arc)[0].receiver = receiver;
                }
                true
            }
        }
    }
}

impl std::ops::Deref for ScopeList {
    type Target = [Scope];
    #[inline]
    fn deref(&self) -> &[Scope] {
        self.as_slice()
    }
}

impl<'a> IntoIterator for &'a ScopeList {
    type Item = &'a Scope;
    type IntoIter = std::slice::Iter<'a, Scope>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

pub type LocalOwnerEntry = (ExecutableOwnerId, (usize, usize));

#[derive(Debug, Default)]
struct LocalOwnersInner {
    mask: u8,
    inline: [Option<LocalOwnerEntry>; 8],
    #[allow(clippy::box_collection)]
    overflow: Option<Box<BTreeMap<usize, LocalOwnerEntry>>>,
}

impl Clone for LocalOwnersInner {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            mask: self.mask,
            inline: self.inline,
            overflow: self.overflow.clone(),
        }
    }
}

/// Compact lexical slot owner map: 8 bytes inline (zero-alloc when empty, e.g. on
/// heap objects without captured locals); retains its box across `clear()` on
/// pooled frames to avoid re-allocating on hot `BindLocal`.
#[derive(Debug, Default)]
pub struct LocalOwners(Option<Box<LocalOwnersInner>>);

impl Clone for LocalOwners {
    #[inline]
    fn clone(&self) -> Self {
        match &self.0 {
            Some(inner)
                if inner.mask != 0 || inner.overflow.as_ref().is_some_and(|m| !m.is_empty()) =>
            {
                Self(Some(inner.clone()))
            }
            _ => Self(None),
        }
    }
}

impl LocalOwners {
    #[inline]
    pub fn is_empty(&self) -> bool {
        match &self.0 {
            None => true,
            Some(inner) => inner.mask == 0 && inner.overflow.as_ref().is_none_or(|m| m.is_empty()),
        }
    }

    #[inline]
    pub fn insert(&mut self, index: usize, value: LocalOwnerEntry) {
        let inner = self
            .0
            .get_or_insert_with(|| Box::new(LocalOwnersInner::default()));
        if index < 8 {
            inner.inline[index] = Some(value);
            inner.mask |= 1u8 << index;
        } else {
            inner
                .overflow
                .get_or_insert_with(|| Box::new(BTreeMap::new()))
                .insert(index, value);
        }
    }

    #[inline]
    pub fn get(&self, index: &usize) -> Option<&LocalOwnerEntry> {
        let inner = self.0.as_deref()?;
        let idx = *index;
        if idx < 8 {
            if (inner.mask & (1u8 << idx)) != 0 {
                inner.inline[idx].as_ref()
            } else {
                None
            }
        } else {
            inner.overflow.as_ref().and_then(|map| map.get(&idx))
        }
    }

    #[inline]
    pub fn remove(&mut self, index: &usize) {
        let Some(inner) = self.0.as_deref_mut() else {
            return;
        };
        let idx = *index;
        if idx < 8 {
            inner.inline[idx] = None;
            inner.mask &= !(1u8 << idx);
        } else if let Some(map) = &mut inner.overflow {
            map.remove(&idx);
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        let Some(inner) = self.0.as_deref_mut() else {
            return;
        };
        if inner.mask != 0 {
            inner.inline = [None; 8];
            inner.mask = 0;
        }
        if let Some(map) = &mut inner.overflow {
            map.clear();
        }
    }

    #[inline]
    pub fn truncate_from(&mut self, start: usize) {
        let Some(inner) = self.0.as_deref_mut() else {
            return;
        };
        if start < 8 {
            let keep_mask = ((1u16 << start) - 1) as u8;
            let mut drop_bits = inner.mask & !keep_mask;
            while drop_bits != 0 {
                let idx = drop_bits.trailing_zeros() as usize;
                inner.inline[idx] = None;
                drop_bits &= drop_bits - 1;
            }
            inner.mask &= keep_mask;
        }
        if let Some(map) = &mut inner.overflow {
            map.retain(|&k, _| k < start);
        }
    }

    #[inline]
    pub fn fast_eq(&self, other: &Self) -> bool {
        if self.is_empty() && other.is_empty() {
            return true;
        }
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => {
                a.mask == b.mask && a.inline == b.inline && a.overflow == b.overflow
            }
            _ => false,
        }
    }

    pub fn values(&self) -> impl Iterator<Item = &LocalOwnerEntry> + '_ {
        self.0.as_deref().into_iter().flat_map(|inner| {
            inner
                .inline
                .iter()
                .flatten()
                .chain(inner.overflow.iter().flat_map(|map| map.values()))
        })
    }
}

/// Copy-on-write compact `BTreeMap` (8 bytes inline; O(1) clone via `Arc`).
#[derive(Clone, Debug)]
pub struct CowBTreeMap<K, V>(Option<Arc<BTreeMap<K, V>>>);

impl<K, V> Default for CowBTreeMap<K, V> {
    #[inline]
    fn default() -> Self {
        Self(None)
    }
}

impl<K: Ord, V> CowBTreeMap<K, V> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_none_or(|m| m.is_empty())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |m| m.len())
    }

    #[inline]
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
    {
        self.0.as_ref()?.get(key)
    }

    #[inline]
    pub fn contains_key<Q: Ord + ?Sized>(&self, key: &Q) -> bool
    where
        K: std::borrow::Borrow<Q>,
    {
        self.0.as_ref().is_some_and(|m| m.contains_key(key))
    }

    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    where
        K: Clone,
        V: Clone,
    {
        Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(BTreeMap::new()))).insert(key, value)
    }

    #[inline]
    pub fn remove<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: std::borrow::Borrow<Q> + Clone,
        V: Clone,
    {
        let arc = self.0.as_mut()?;
        if !arc.contains_key(key) {
            return None;
        }
        Arc::make_mut(arc).remove(key)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0 = None;
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.as_deref().into_iter().flat_map(|m| m.iter())
    }
}

impl<K: Ord, V> From<BTreeMap<K, V>> for CowBTreeMap<K, V> {
    #[inline]
    fn from(map: BTreeMap<K, V>) -> Self {
        if map.is_empty() {
            Self(None)
        } else {
            Self(Some(Arc::new(map)))
        }
    }
}

/// Compact `FastMap` (8 bytes inline when empty; avoids `RawTable` drop on cold fields).
#[derive(Debug)]
pub struct ThinFastMap<K, V>(#[allow(clippy::box_collection)] Option<Box<FastMap<K, V>>>);

impl<K, V> Default for ThinFastMap<K, V> {
    #[inline]
    fn default() -> Self {
        Self(None)
    }
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> Clone for ThinFastMap<K, V> {
    #[inline]
    fn clone(&self) -> Self {
        match &self.0 {
            Some(map) if !map.is_empty() => Self(Some(map.clone())),
            _ => Self(None),
        }
    }
}

impl<K: Eq + std::hash::Hash, V> ThinFastMap<K, V> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_none_or(|m| m.is_empty())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |m| m.len())
    }

    #[inline]
    pub fn get<Q: std::hash::Hash + Eq + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
    {
        self.0.as_ref()?.get(key)
    }

    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0
            .get_or_insert_with(|| Box::new(FastMap::default()))
            .insert(key, value)
    }

    #[inline]
    pub fn remove<Q: std::hash::Hash + Eq + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: std::borrow::Borrow<Q>,
    {
        self.0.as_mut()?.remove(key)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0 = None;
    }

    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.0.as_deref().into_iter().flat_map(|m| m.values())
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.as_deref().into_iter().flat_map(|m| m.iter())
    }
}

impl<K: Eq + std::hash::Hash, V> From<FastMap<K, V>> for ThinFastMap<K, V> {
    #[inline]
    fn from(map: FastMap<K, V>) -> Self {
        if map.is_empty() {
            Self(None)
        } else {
            Self(Some(Box::new(map)))
        }
    }
}

impl<K: Eq + std::hash::Hash, V> FromIterator<(K, V)> for ThinFastMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let Some(first) = iter.next() else {
            return Self(None);
        };
        let mut map = FastMap::default();
        map.insert(first.0, first.1);
        map.extend(iter);
        Self(Some(Box::new(map)))
    }
}

impl<K, V> IntoIterator for ThinFastMap<K, V> {
    type Item = (K, V);
    type IntoIter =
        std::iter::Flatten<std::option::IntoIter<std::collections::hash_map::IntoIter<K, V>>>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.map(|b| (*b).into_iter()).into_iter().flatten()
    }
}

/// Compact `BTreeMap` (8 bytes inline when empty).
#[derive(Debug)]
pub struct ThinBTreeMap<K, V>(#[allow(clippy::box_collection)] Option<Box<BTreeMap<K, V>>>);

impl<K, V> Default for ThinBTreeMap<K, V> {
    #[inline]
    fn default() -> Self {
        Self(None)
    }
}

impl<K: Clone, V: Clone> Clone for ThinBTreeMap<K, V> {
    #[inline]
    fn clone(&self) -> Self {
        match &self.0 {
            Some(map) if !map.is_empty() => Self(Some(map.clone())),
            _ => Self(None),
        }
    }
}

impl<K: Ord, V> ThinBTreeMap<K, V> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_none_or(|m| m.is_empty())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |m| m.len())
    }

    #[inline]
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
    {
        self.0.as_ref()?.get(key)
    }

    #[inline]
    pub fn contains_key<Q: Ord + ?Sized>(&self, key: &Q) -> bool
    where
        K: std::borrow::Borrow<Q>,
    {
        self.0.as_ref().is_some_and(|m| m.contains_key(key))
    }

    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0
            .get_or_insert_with(|| Box::new(BTreeMap::new()))
            .insert(key, value)
    }

    #[inline]
    pub fn entry(&mut self, key: K) -> std::collections::btree_map::Entry<'_, K, V> {
        self.0
            .get_or_insert_with(|| Box::new(BTreeMap::new()))
            .entry(key)
    }

    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.0.as_deref().into_iter().flat_map(|m| m.values())
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.as_deref().into_iter().flat_map(|m| m.iter())
    }

    #[inline]
    pub fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        let mut iter = iter.into_iter();
        if let Some((k, v)) = iter.next() {
            let map = self.0.get_or_insert_with(|| Box::new(BTreeMap::new()));
            map.insert(k, v);
            map.extend(iter);
        }
    }
}

/// Compact `BTreeSet` (8 bytes inline when empty).
#[derive(Debug)]
pub struct ThinBTreeSet<T>(#[allow(clippy::box_collection)] Option<Box<BTreeSet<T>>>);

impl<T> Default for ThinBTreeSet<T> {
    #[inline]
    fn default() -> Self {
        Self(None)
    }
}

impl<T: Clone> Clone for ThinBTreeSet<T> {
    #[inline]
    fn clone(&self) -> Self {
        match &self.0 {
            Some(set) if !set.is_empty() => Self(Some(set.clone())),
            _ => Self(None),
        }
    }
}

impl<T: Ord> ThinBTreeSet<T> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_none_or(|s| s.is_empty())
    }

    #[inline]
    pub fn contains<Q: Ord + ?Sized>(&self, value: &Q) -> bool
    where
        T: std::borrow::Borrow<Q>,
    {
        self.0.as_ref().is_some_and(|s| s.contains(value))
    }

    #[inline]
    pub fn insert(&mut self, value: T) -> bool {
        self.0
            .get_or_insert_with(|| Box::new(BTreeSet::new()))
            .insert(value)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0 = None;
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.as_deref().into_iter().flat_map(|s| s.iter())
    }
}

impl<T: Ord> FromIterator<T> for ThinBTreeSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let Some(first) = iter.next() else {
            return Self(None);
        };
        let mut set = BTreeSet::new();
        set.insert(first);
        set.extend(iter);
        Self(Some(Box::new(set)))
    }
}

/// Compact `Vec` (8 bytes inline when empty).
#[derive(Debug)]
pub struct ThinVec<T>(#[allow(clippy::box_collection)] Option<Box<Vec<T>>>);

impl<T> Default for ThinVec<T> {
    #[inline]
    fn default() -> Self {
        Self(None)
    }
}

impl<T: Clone> Clone for ThinVec<T> {
    #[inline]
    fn clone(&self) -> Self {
        match &self.0 {
            Some(v) if !v.is_empty() => Self(Some(v.clone())),
            _ => Self(None),
        }
    }
}

impl<T> ThinVec<T> {
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.0.as_deref().map_or(&[], Vec::as_slice)
    }

    #[inline]
    pub fn push(&mut self, value: T) {
        self.0
            .get_or_insert_with(|| Box::new(Vec::new()))
            .push(value);
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        let v = self.0.as_mut()?;
        let item = v.pop();
        if v.is_empty() {
            self.0 = None;
        }
        item
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0 = None;
    }

    #[inline]
    pub fn truncate(&mut self, len: usize) {
        if let Some(v) = &mut self.0 {
            v.truncate(len);
        }
    }

    #[inline]
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let mut iter = iter.into_iter();
        if let Some(first) = iter.next() {
            let v = self.0.get_or_insert_with(|| Box::new(Vec::new()));
            v.push(first);
            v.extend(iter);
        }
    }

    #[inline]
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        if let Some(v) = &mut self.0 {
            v.retain(f);
        }
    }

    #[inline]
    pub fn into_vec(self) -> Vec<T> {
        self.0.map_or_else(Vec::new, |b| *b)
    }
}

impl<T> std::ops::Deref for ThinVec<T> {
    type Target = [T];
    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> From<Vec<T>> for ThinVec<T> {
    #[inline]
    fn from(v: Vec<T>) -> Self {
        if v.is_empty() {
            Self(None)
        } else {
            Self(Some(Box::new(v)))
        }
    }
}

impl<T> FromIterator<T> for ThinVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from(iter.into_iter().collect::<Vec<_>>())
    }
}

impl<'a, T> IntoIterator for &'a ThinVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Inline bitset for copied lexical slots (zero-alloc for indices < 64).
#[derive(Clone, Debug, Default)]
pub struct CopyLocals {
    inline_mask: u64,
    // Keep `Environment` compact (8 bytes instead of 24) for the common < 64-local fast path.
    #[allow(clippy::box_collection)]
    overflow: Option<Box<BTreeSet<usize>>>,
}

impl CopyLocals {
    #[inline]
    pub fn insert(&mut self, index: usize) {
        if index < 64 {
            self.inline_mask |= 1u64 << index;
        } else {
            self.overflow
                .get_or_insert_with(|| Box::new(BTreeSet::new()))
                .insert(index);
        }
    }

    #[inline]
    pub fn remove(&mut self, index: &usize) {
        let idx = *index;
        if idx < 64 {
            self.inline_mask &= !(1u64 << idx);
        } else if let Some(set) = &mut self.overflow {
            set.remove(&idx);
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inline_mask = 0;
        if let Some(set) = &mut self.overflow {
            set.clear();
        }
    }

    #[inline]
    pub fn fast_eq(&self, other: &Self) -> bool {
        self.inline_mask == other.inline_mask && self.overflow == other.overflow
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        struct MaskIter(u64);
        impl Iterator for MaskIter {
            type Item = usize;
            #[inline]
            fn next(&mut self) -> Option<usize> {
                if self.0 == 0 {
                    return None;
                }
                let bit = self.0.trailing_zeros() as usize;
                self.0 &= self.0 - 1;
                Some(bit)
            }
        }
        MaskIter(self.inline_mask).chain(self.overflow.iter().flat_map(|s| s.iter().copied()))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Environment {
    pub locals: Vec<Value>,
    /// Source descriptor for materialized physical slots retained by closures.
    #[doc(hidden)]
    pub local_owners: LocalOwners,
    /// Parameters, let bindings and generator bindings in the current lexical
    /// frame. Enclosing function captures are not copied into a new descriptor.
    #[doc(hidden)]
    pub copy_locals: CopyLocals,
    /// Generated members restore their saved generator activation on forcing.
    #[doc(hidden)]
    pub restore_for_bindings: bool,
    pub this_value: Option<Value>,
    pub outer_value: Option<Value>,
    pub custom_this: Option<CustomThisSlot>,
    /// Persistent cloned executable identity, independent of the receiver slot.
    /// None keeps the canonical code template implicit across every branch.
    pub executable_owner: Option<ExecutableOwnerId>,
    /// Traced lifetime of dynamically compiled code captured by this environment.
    #[doc(hidden)]
    pub code_owner: Option<ExecutableOwnerId>,
    /// Lexical substitutions captured by reusable generic constraint bodies.
    /// Runtime values are immutable shared type DAGs; environment clones copy handles.
    pub type_arguments: CowBTreeMap<(usize, PklString), crate::vm::TypeCheck>,
    /// Source sections follow actual arguments without participating in type identity.
    pub type_argument_sources: CowBTreeMap<(usize, PklString), crate::vm::TypeArgumentSource>,
    /// Innermost first; immutable captured lexical object scopes.
    pub scopes: ScopeList,
}

#[derive(Clone, Debug)]
pub struct Member {
    /// Authored modifier arithmetic and the pinned internal declaration bits.
    pub flags: u32,
    pub entry: bool,
    pub name: PklString,
    pub code: usize,
    pub local: bool,
    pub method: bool,
    pub ty: Option<crate::vm::TypeCheck>,
    pub type_span: Option<crate::source::Span>,
    pub constant: bool,
    /// A class property may be overridden only by another fixed declaration.
    pub fixed: bool,
}

impl Member {
    const CLASS: u32 = 0x100;
    const TYPE_ALIAS: u32 = 0x200;
    const IMPORT: u32 = 0x80;
    const GLOB: u32 = 0x1000;
    pub(crate) fn declaration_flags(member: &crate::syntax::Member) -> u32 {
        crate::syntax::modifier_flags(&member.modifiers)
            | match member.expr.kind {
                crate::syntax::ExprKind::Import {
                    clause_alias: Some(_),
                    glob,
                    ..
                } => Self::IMPORT | if glob { Self::GLOB } else { 0 },
                crate::syntax::ExprKind::Class { .. } => Self::CLASS,
                crate::syntax::ExprKind::TypeAlias { .. } => Self::TYPE_ALIAS,
                _ => 0,
            }
    }
    pub(crate) fn hidden(&self) -> bool {
        self.flags & crate::syntax::ModifierKind::Hidden.flag() != 0
    }
    pub(crate) fn external(&self) -> bool {
        self.flags & crate::syntax::ModifierKind::External.flag() != 0
    }
    pub(crate) fn abstract_member(&self) -> bool {
        self.flags & crate::syntax::ModifierKind::Abstract.flag() != 0
    }
    pub(crate) fn import(&self) -> bool {
        self.flags & Self::IMPORT != 0
    }
    pub(crate) fn glob(&self) -> bool {
        self.flags & Self::GLOB != 0
    }
    pub(crate) fn definition(&self) -> bool {
        self.flags & (Self::CLASS | Self::TYPE_ALIAS) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CacheEntry<'a> {
    Evaluating,
    Completed(&'a Value),
}

#[derive(Clone, Debug, PartialEq)]
pub enum LazyState {
    Unevaluated,
    Evaluating,
    Completed(Value),
}

/// Local definitions use owner identity; ordinary virtual properties use `None`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey {
    pub entry: bool,
    pub owner: Option<ObjectId>,
    pub name: PklString,
    pub method: bool,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub block: usize,
    pub arity: usize,
    pub method: bool,
    /// Custom-scope receiver captured when the literal is evaluated.
    pub captured_this: Option<Value>,
}

/// Class labels follow PClassInfo.getDisplayName, independently of identity or URI.
pub(crate) fn display_class_name(
    module: impl Into<PklString>,
    simple_name: impl Into<PklString>,
) -> PklString {
    let module = module.into();
    let simple_name = simple_name.into();
    if module == "pkl.base" {
        simple_name
    } else if simple_name == "ModuleClass" {
        module
    } else {
        module.concat(&"#".into()).concat(&simple_name)
    }
}

#[derive(Clone, Debug)]
pub struct Class {
    pub module: Option<PklString>,
    /// Original declaration ModuleKey URI, independent of the displayed module name.
    pub origin_uri: Option<PklString>,
    pub id: usize,
    pub name: PklString,
    pub open: bool,
    pub abstract_: bool,
}

/// A class value is distinct from its instance prototype. Declaration metadata
/// and incremental member publication live on the owned prototype exactly once.
/// Both edges are traced; neither depends on a Rust reference-counted cycle.
#[derive(Clone, Debug)]
pub struct ClassRecord {
    pub prototype: ObjectId,
    pub superclass: Option<ObjectId>,
    pub(crate) class_id: Option<usize>,
    /// Inline ancestor `Class::id` chain (self first, up to root) populated once
    /// `link_superclass` completes over an already-linked superclass chain.
    /// `ancestors_len == u8::MAX` indicates uncached / deeper than 8.
    pub(crate) ancestors: [usize; 8],
    pub(crate) ancestors_len: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CollectionKind {
    #[default]
    Dynamic,
    Listing,
    Mapping,
    List,
    Map,
    Set,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionMemberKind {
    Property,
    Entry,
    Element,
}

#[derive(Clone, Debug)]
pub struct CollectionMember {
    pub key: Value,
    pub name: PklString,
    pub kind: CollectionMemberKind,
}

#[derive(Clone, Debug)]
pub struct CollectionIterator {
    pub source: ObjectId,
    pub members: Vec<CollectionMember>,
    pub position: usize,
    pub force_members: Vec<CollectionMember>,
    pub force_position: usize,
    pub awaiting_value: bool,
    pub members_refreshed: bool,
}

#[derive(Clone, Debug)]
pub struct PredicateIterator {
    pub source: ObjectId,
    pub members: Vec<(ObjectId, CollectionMember)>,
    pub position: usize,
    pub awaiting_value: bool,
    pub previous_this: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct CollectionCast {
    pub source: Option<Arc<crate::value::RuntimeSource>>,
    pub(crate) node: Option<TypeNodeId>,
    pub ty: crate::vm::TypeCheck,
    pub span: crate::source::Span,
    pub member: crate::value::PklString,
    pub environment: Environment,
}

#[derive(Clone, Debug)]
pub struct CacheSlot {
    pub key: CacheKey,
    /// The key retained by the receiver cache, independently of its declaration.
    /// Bootstrap strict slots recover their declaration key when this is None.
    pub entry_key: Option<Value>,
    pub value: Value,
}

/// Copy-on-write member list: empty is zero-allocation, cloning is an `Arc` bump.
/// Eliminates cloning hundreds of `Member` / `TypeCheck` entries on `BaseSnapshot`
/// instantiation and factory/collection copies.
#[derive(Clone, Debug, Default)]
pub struct MemberList(Option<Arc<Vec<Member>>>);

impl MemberList {
    #[inline]
    pub fn as_slice(&self) -> &[Member] {
        self.0.as_deref().map_or(&[], Vec::as_slice)
    }

    #[inline]
    pub fn push(&mut self, member: Member) {
        Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(Vec::new()))).push(member);
    }

    #[inline]
    pub fn extend<I: IntoIterator<Item = Member>>(&mut self, iter: I) {
        let mut iter = iter.into_iter();
        if let Some(first) = iter.next() {
            let vec = Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(Vec::new())));
            vec.push(first);
            vec.extend(iter);
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0 = None;
    }

    #[inline]
    pub fn retain<F: FnMut(&Member) -> bool>(&mut self, f: F) {
        if let Some(arc) = self.0.as_mut() {
            Arc::make_mut(arc).retain(f);
        }
    }

    #[inline]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Member> {
        match self.0.as_mut() {
            Some(arc) => Arc::make_mut(arc).iter_mut(),
            None => [].iter_mut(),
        }
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        if additional > 0 {
            Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(Vec::new()))).reserve(additional);
        }
    }

    #[inline]
    pub fn set(&mut self, index: usize, member: Member) {
        Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(Vec::new())))[index] = member;
    }

    #[inline]
    pub fn from_arc(arc: Arc<Vec<Member>>) -> Self {
        if arc.is_empty() {
            Self(None)
        } else {
            Self(Some(arc))
        }
    }
}

impl From<Vec<Member>> for MemberList {
    #[inline]
    fn from(v: Vec<Member>) -> Self {
        if v.is_empty() {
            Self(None)
        } else {
            Self(Some(Arc::new(v)))
        }
    }
}

impl FromIterator<Member> for MemberList {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Member>>(iter: I) -> Self {
        Self::from(iter.into_iter().collect::<Vec<_>>())
    }
}

impl std::ops::Deref for MemberList {
    type Target = [Member];
    #[inline]
    fn deref(&self) -> &[Member] {
        self.as_slice()
    }
}

impl<'a> IntoIterator for &'a MemberList {
    type Item = &'a Member;
    type IntoIter = std::slice::Iter<'a, Member>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Object {
    /// Lazy mirror factory payload. Edges are traced independently of the
    /// class prototype, including the declaration's executable owner.
    pub mirror: Option<Box<crate::vm::reflection::Payload>>,
    /// Reader payload has no heap edges. Factory property nodes read their
    /// declaration owner's payload, including when inherited by an amendment.
    pub resource: Option<Arc<crate::resources::Resource>>,
    pub(crate) glob: Option<Arc<crate::vm::glob::Payload>>,
    /// Immutable match result has no heap edges; factory reads declaration owner.
    pub regex_match: Option<Arc<crate::pkl_regex::RegexMatchData>>,
    /// Recursive force publishes this mark before traversing children.
    pub forced: bool,
    /// VmDynamic caches its first regular-member count, including a reentrant
    /// force prefix. Later cache publication does not invalidate that count.
    pub dynamic_regular_member_count: Option<usize>,
    /// Evaluator-owned canonical identity, populated only after key hashing succeeds.
    pub structural_key: Option<u64>,
    /// Numeric Java hash, independent of canonical equality identity.
    /// Inner None marks a hash depending on JVM identity allocation.
    pub java_hash: Option<Option<i32>>,
    pub pair: Option<Box<[Value; 2]>>,
    /// Opaque Java Map.entry values exposed by VmCollection.flatten.
    /// Distinct from Pkl Pair; key/value remain traced and structurally comparable.
    pub foreign_map_entry: Option<Box<[Value; 2]>>,
    pub amendment: Option<Box<crate::vm::amendment::Amendment>>,
    /// Argument-check section and function-construction section have distinct
    /// Java source nodes, even when both belong to the same Mixin type use.
    #[allow(clippy::type_complexity)]
    pub mixin: Option<
        Box<(
            crate::vm::TypeCheck,
            Option<crate::value::RuntimeFrame>,
            Option<crate::value::RuntimeFrame>,
        )>,
    >,
    pub function: Option<Function>,
    pub reference: Option<Box<crate::vm::reference::Reference>>,
    pub native_function: Option<Box<crate::vm::reference::NativeFunction>>,
    pub type_alias: Option<PklString>,
    pub type_alias_declaration: Option<usize>,
    /// Compiled declaration identity, separate from resolved type identity.
    /// The environment below retains the owning executable arena.
    pub declaration_code: Option<usize>,
    /// Successful mirror cache belongs to the declaration, never a VM-wide root.
    pub declaration_mirror: Option<ObjectId>,
    pub module_mirror: Option<ObjectId>,
    pub mirror_annotations: ThinBTreeMap<crate::source::Span, Vec<Value>>,
    pub type_alias_initialized: bool,
    /// Declaration-owned annotation values; fields remain ordinary lazy members.
    pub annotations: ThinVec<Value>,
    /// Published property/method annotation metadata, keyed by declaration code.
    pub member_annotations: ThinBTreeMap<usize, Vec<Value>>,
    /// First exact Deprecated annotation's forced message, published with its method.
    #[doc(hidden)]
    pub member_deprecations: ThinBTreeMap<usize, crate::value::PklString>,
    /// Temporary annotation-list metadata, before declaration publication.
    #[doc(hidden)]
    pub deprecation: Option<crate::value::PklString>,
    /// Actual declaration of a materialized method call target.
    #[doc(hidden)]
    pub method_declaration: Option<(ObjectId, usize)>,
    pub builtin_type: Option<Box<crate::vm::TypeCheck>>,
    pub ref_module: bool,
    pub is_module: bool,
    pub module_unit: Option<crate::modules::UnitId>,
    pub module_initializing: bool,
    pub published_module_members: ThinBTreeSet<usize>,
    pub class_definition: Option<Box<Class>>,
    /// The actual runtime class of a typed instance or class prototype.
    pub instance_class: Option<ObjectId>,
    /// Published class whose initialization has not completed (including failure).
    /// Such classes accept new property names, but expose only published metadata.
    pub class_initializing: bool,
    /// Distinguishes a published class with an unresolved superclass from Any,
    /// whose initialized superclass is absent.
    pub class_superclass_initialized: bool,
    pub parent: Option<ObjectId>,
    pub members: MemberList,
    pub environment: Environment,
    pub cache_slots: Vec<CacheSlot>,
    /// Lazy O(1) index (`CacheKey -> slot index`), allocated only once
    /// `cache_slots.len() > 16` or when `PublishMatch` aliases a second key to an existing slot.
    pub cache_map: ThinFastMap<CacheKey, usize>,
    /// Inline cycle-detection marker for the common 0-or-1 evaluating member case (zero alloc).
    pub evaluating_key: Option<CacheKey>,
    /// Overflow cycle-detection markers when >1 members on the same object evaluate concurrently.
    pub evaluating_overflow: ThinVec<CacheKey>,
    pub cache_index: crate::economic_map::EconomicMapIndex,
    pub collection_kind: CollectionKind,
    pub collection_members: Vec<CollectionMember>,
    pub has_object_collection_key: bool,
    pub map_index: Option<Box<crate::strict_map::MapIndex>>,
    /// Captured Map insertion-order keys, independent of stored trie slots.
    /// Removal can change a different slot after recursive key forcing.
    #[allow(clippy::box_collection)]
    pub map_key_order: Option<Box<Vec<CollectionMember>>>,
    /// Strict Set hash-table keys; element order lives in collection_members.
    pub set_entries: ThinVec<Value>,
    pub declaration_index: Option<Box<crate::economic_map::EconomicMapIndex>>,
    pub declaration_hashes: ThinVec<i32>,
    pub length: usize,
    /// Compact dense storage for strict immutable `List` values (`0..length`),
    /// avoiding per-element `BTreeMap` / `PklString` / `Member` overhead.
    pub list_elements: Option<Vec<Value>>,
    pub mapping_length: Option<usize>,
    pub repeat_base_len: Option<usize>,
    /// VmMapping.getAllKeys caches its own persistent Set wrapper.
    pub mapping_keys: Option<ObjectId>,
    pub specialized_literal: bool,
    pub member_environments: ThinFastMap<CacheKey, Environment>,
    pub member_values: ThinFastMap<CacheKey, Value>,
    pub iterator: Option<Box<CollectionIterator>>,
    pub int_seq_iterator: Option<Box<crate::int_seq::Cursor>>,
    pub bytes_iterator: Option<Box<(crate::value::Bytes, usize, bool)>>,
    pub conversion_delegate: Option<Box<crate::vm::collection_conversions::Delegate>>,
    pub predicate_iterator: Option<Box<PredicateIterator>>,
    pub collection_cast: Option<Box<CollectionCast>>,
    /// Inherited default value for inferred element/entry objects. `Some(None)`
    /// means the annotated value type has no default; `None` uses Dynamic's default.
    pub collection_default: Option<Option<Value>>,
    /// Authored declaration of a synthesized undefined collection default.
    pub collection_default_origin: Option<Box<crate::value::RuntimeFrame>>,
}

impl Object {
    pub(crate) fn map_index(&self) -> &crate::strict_map::MapIndex {
        self.map_index
            .as_deref()
            .unwrap_or(&crate::strict_map::EMPTY)
    }

    #[inline(always)]
    pub fn has_evaluating_keys(&self) -> bool {
        self.evaluating_key.is_some()
    }

    #[inline(always)]
    pub fn cache_is_empty(&self) -> bool {
        self.cache_slots.is_empty() && self.evaluating_key.is_none()
    }

    #[inline(always)]
    pub fn cache_len(&self) -> usize {
        self.cache_slots.len()
            + usize::from(self.evaluating_key.is_some())
            + self.evaluating_overflow.len()
    }

    #[inline(always)]
    fn is_evaluating(&self, key: &CacheKey) -> bool {
        match &self.evaluating_key {
            None => false,
            Some(k) if k == key => true,
            Some(_) => {
                !self.evaluating_overflow.is_empty()
                    && self.evaluating_overflow.as_slice().contains(key)
            }
        }
    }

    #[inline]
    pub fn cache_get(&self, key: &CacheKey) -> Option<CacheEntry<'_>> {
        if self.is_evaluating(key) {
            return Some(CacheEntry::Evaluating);
        }
        if self.cache_map.is_empty() {
            for slot in self.cache_slots.iter().rev() {
                if slot.key == *key {
                    return Some(CacheEntry::Completed(&slot.value));
                }
            }
            None
        } else {
            self.cache_map
                .get(key)
                .map(|&idx| CacheEntry::Completed(&self.cache_slots[idx].value))
        }
    }

    #[inline]
    pub fn cache_get_completed(&self, key: &CacheKey) -> Option<&Value> {
        match self.cache_get(key) {
            Some(CacheEntry::Completed(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn cache_contains_key(&self, key: &CacheKey) -> bool {
        self.cache_get(key).is_some()
    }

    #[inline]
    pub fn cache_mark_evaluating(&mut self, key: CacheKey) {
        match &self.evaluating_key {
            None => self.evaluating_key = Some(key),
            Some(k) if k == &key => {}
            Some(_) => {
                if !self.evaluating_overflow.as_slice().contains(&key) {
                    self.evaluating_overflow.push(key);
                }
            }
        }
    }

    #[inline]
    pub fn cache_clear_evaluating(&mut self, key: &CacheKey) {
        if self.evaluating_key.as_ref() == Some(key) {
            self.evaluating_key = self.evaluating_overflow.pop();
        } else if !self.evaluating_overflow.is_empty() {
            self.evaluating_overflow.retain(|k| k != key);
            if self.evaluating_overflow.is_empty() {
                self.evaluating_overflow.clear();
            }
        }
    }

    fn ensure_cache_map(&mut self) {
        if self.cache_map.is_empty() && !self.cache_slots.is_empty() {
            let mut map = FastMap::default();
            map.reserve(self.cache_slots.len());
            for (idx, slot) in self.cache_slots.iter().enumerate() {
                map.insert(slot.key.clone(), idx);
            }
            self.cache_map = map.into();
        }
    }

    #[inline]
    pub fn push_completed_slot(&mut self, key: CacheKey, entry_key: Option<Value>, value: Value) {
        self.cache_clear_evaluating(&key);
        let idx = self.cache_slots.len();
        if !self.cache_map.is_empty() {
            self.cache_map.insert(key.clone(), idx);
            self.cache_slots.push(CacheSlot {
                key,
                entry_key,
                value,
            });
        } else {
            self.cache_slots.push(CacheSlot {
                key,
                entry_key,
                value,
            });
            if self.cache_slots.len() > 16 {
                self.ensure_cache_map();
            }
        }
    }

    #[inline]
    pub fn update_completed_slot(&mut self, index: usize, key: CacheKey, value: Value) {
        self.cache_clear_evaluating(&key);
        self.cache_slots[index].value = value;
        if self.cache_slots[index].key != key {
            self.ensure_cache_map();
            self.cache_map.insert(key, index);
        } else if !self.cache_map.is_empty() {
            self.cache_map.insert(key, index);
        }
    }

    pub(crate) fn cache_clear(&mut self) {
        self.cache_slots.clear();
        self.cache_map.clear();
        self.evaluating_key = None;
        self.evaluating_overflow.clear();
    }

    pub(crate) fn cache_remove_key(&mut self, key: &CacheKey) {
        self.cache_clear_evaluating(key);
        self.cache_slots.retain(|s| &s.key != key);
        self.rebuild_cache_map();
    }

    pub(crate) fn rebuild_cache_map(&mut self) {
        self.cache_map.clear();
        if self.cache_slots.len() > 16 {
            self.ensure_cache_map();
        }
    }

    pub(crate) fn complete_cache(&mut self, key: CacheKey, value: Value) {
        self.complete_cache_with_key(key, None, value);
    }

    pub(crate) fn complete_cache_with_key(
        &mut self,
        key: CacheKey,
        entry_key: Option<Value>,
        value: Value,
    ) {
        self.cache_clear_evaluating(&key);
        let existing = if self.cache_map.is_empty() {
            self.cache_slots.iter().rposition(|slot| slot.key == key)
        } else {
            self.cache_map.get(&key).copied()
        };
        if let Some(idx) = existing {
            if entry_key.is_some() {
                self.cache_slots[idx].entry_key = entry_key;
            }
            self.cache_slots[idx].value = value;
        } else {
            self.push_completed_slot(key, entry_key, value);
        }
    }
}

/// Identity of one logical call frame's auxiliary receiver slot. This is never
/// exposed as a Pkl value; its arena entry participates in ordinary tracing.
#[derive(Clone, Copy, Debug)]
pub struct CustomThisSlot(ObjectId);

impl CustomThisSlot {
    pub(crate) fn root(self) -> Value {
        Value::Object(self.0)
    }
}

/// Persistent evaluator-local executable state. Handles are generation checked
/// and traced, but are never exposed as language values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutableOwnerId(ObjectId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TypeNodeId(pub(crate) ExecutableOwnerId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ExecutionSite {
    TypeUse { block: usize, instruction: usize },
    PropertyType(usize),
    TypeChild(usize),
    Constraint(usize),
    Function { block: usize, instruction: usize },
    RegexConstructor { block: usize, instruction: usize },
    Generator { block: usize, instruction: usize },
    Member(usize),
}

#[derive(Clone, Default)]
struct ExecutableState {
    code_roots: Vec<Value>,
    children: BTreeMap<ExecutionSite, ExecutableOwnerId>,
    // ObjectMember belongs to the declaration, whereas owner/receiver are read
    // from the current frame on every invocation. Never retain the first receiver.
    local_properties: BTreeMap<(usize, usize), Member>,
    // ReadAmbiguousLocalityPropertyNode replaces itself on its first execution.
    // The choice belongs to this executable clone, never to a receiver object.
    ambiguous_localities: BTreeMap<(usize, usize), bool>,
    local_kinds: Vec<((usize, usize), crate::vm::identity::SlotKind)>,
    // ImportNode caches its successful result, not the importing receiver or
    // activation. Distinct executable clones retain distinct node state.
    imported_modules: BTreeMap<(usize, usize), ObjectId>,
    glob_mappings: BTreeMap<(usize, usize, crate::value::PklString), ObjectId>,
    method_calls: Vec<((usize, usize), crate::vm::deprecation::CallCache)>,
    regex_constructor: crate::pkl_regex::ConstructorCache,
}

impl ExecutableOwnerId {
    pub(crate) fn root(self) -> Value {
        Value::Object(self.0)
    }
}

#[derive(Clone)]
enum Entry {
    Object(Box<Object>),
    Class(Box<ClassRecord>),
    NullDefault(Box<Value>),
    CustomThis(Option<Box<Value>>),
    Executable(Box<ExecutableState>),
}

#[derive(Clone)]
struct Slot {
    generation: u64,
    entry: Option<Entry>,
    edges: Vec<ObjectId>,
    edges_valid: bool,
}

impl Slot {
    fn object(&self) -> Option<&Object> {
        match &self.entry {
            Some(Entry::Object(object)) => Some(object),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeapStats {
    pub live: usize,
    /// Persistent executable/type nodes included in `live`, excluding objects and receiver slots.
    pub executable_live: usize,
    pub slots: usize,
    pub collections: usize,
    pub reclaimed: usize,
}

/// Frozen immutable `pkl:base` heap prefix shared via copy-on-write across fresh evaluators.
pub(crate) struct FrozenBaseHeap {
    identity: u64,
    slots: Arc<[Slot]>,
    stats: HeapStats,
    structural_keys: BTreeMap<crate::vm::equality::StructuralKey, u64>,
    next_structural_key: u64,
    key_equalities: std::collections::BTreeSet<(ObjectId, ObjectId)>,
    bytes_lists: BytesListMap,
    has_structural_keys: bool,
    class_index: Arc<FastMap<usize, ObjectId>>,
    module_index: Arc<FastMap<u32, ObjectId>>,
}

pub struct Heap {
    identity: u64,
    /// Optional shared frozen `pkl:base` prefix (`0..base_len`, owned by `base_id`).
    base_slots: Option<Arc<[Slot]>>,
    base_len: usize,
    base_id: u64,
    base_has_structural_keys: bool,
    base_class_index: Option<Arc<FastMap<usize, ObjectId>>>,
    base_module_index: Option<Arc<FastMap<u32, ObjectId>>>,
    /// Sparse copy-on-write overrides for mutated slots in `0..base_len`.
    base_overrides: Vec<(usize, Slot)>,
    /// Lazy O(1) index (`pos + 1`, `0` = not overridden) allocated once `base_overrides.len() >= 4`.
    base_override_index: Option<Box<[u32]>>,
    /// User-allocated slots (`index = base_len + local_idx`, owned by `identity`).
    slots: Vec<Slot>,
    free: Vec<usize>,
    class_index: FastMap<usize, ObjectId>,
    module_index: FastMap<u32, ObjectId>,
    stats: HeapStats,
    mark_scratch: Vec<bool>,
    pending_scratch: Vec<ObjectId>,
    structural_keys_dirty: bool,
    last_retained_structural_keys_len: usize,
    last_retained_key_equalities_len: usize,
    pub(crate) structural_keys: BTreeMap<crate::vm::equality::StructuralKey, u64>,
    pub(crate) next_structural_key: u64,
    pub(crate) key_equalities: std::collections::BTreeSet<(ObjectId, ObjectId)>,
    // Heap-local VmBytes.vmList backlinks never put evaluator handles in code constants.
    pub(crate) bytes_lists:
        BTreeMap<crate::value_identity::Token, (std::sync::Weak<()>, ObjectId, bool)>,
    allocations_since_gc: usize,
    external_bytes_since_gc: usize,
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Heap {
    pub fn new() -> Self {
        Self {
            identity: NEXT_HEAP.fetch_add(1, Ordering::Relaxed),
            base_slots: None,
            base_len: 0,
            base_id: 0,
            base_has_structural_keys: false,
            base_class_index: None,
            base_module_index: None,
            base_overrides: Vec::new(),
            base_override_index: None,
            slots: Vec::new(),
            free: Vec::new(),
            class_index: FastMap::default(),
            module_index: FastMap::default(),
            stats: HeapStats::default(),
            mark_scratch: Vec::new(),
            pending_scratch: Vec::new(),
            structural_keys_dirty: false,
            last_retained_structural_keys_len: 0,
            last_retained_key_equalities_len: 0,
            structural_keys: BTreeMap::new(),
            next_structural_key: 1,
            key_equalities: std::collections::BTreeSet::new(),
            bytes_lists: BTreeMap::new(),
            allocations_since_gc: 0,
            external_bytes_since_gc: 0,
        }
    }

    /// Freeze a freshly bootstrapped `pkl:base` heap into a shared immutable CoW prefix.
    pub(crate) fn freeze_base(self) -> FrozenBaseHeap {
        debug_assert!(self.base_slots.is_none());
        let has_structural_keys = self
            .slots
            .iter()
            .any(|slot| slot.object().is_some_and(|o| o.structural_key.is_some()));
        let slots: Arc<[Slot]> = Arc::from(self.slots);
        let mut stats = self.stats;
        stats.collections = 0;
        stats.reclaimed = 0;
        stats.slots = slots.len();
        FrozenBaseHeap {
            identity: self.identity,
            slots,
            stats,
            structural_keys: self.structural_keys,
            next_structural_key: self.next_structural_key,
            key_equalities: self.key_equalities,
            bytes_lists: self.bytes_lists,
            has_structural_keys,
            class_index: Arc::new(self.class_index),
            module_index: Arc::new(self.module_index),
        }
    }

    /// O(1) fresh heap sharing the frozen `pkl:base` prefix via copy-on-write.
    pub(crate) fn from_frozen_base(frozen: &FrozenBaseHeap) -> Self {
        Self {
            identity: NEXT_HEAP.fetch_add(1, Ordering::Relaxed),
            base_slots: Some(Arc::clone(&frozen.slots)),
            base_len: frozen.slots.len(),
            base_id: frozen.identity,
            base_has_structural_keys: frozen.has_structural_keys,
            base_class_index: Some(Arc::clone(&frozen.class_index)),
            base_module_index: Some(Arc::clone(&frozen.module_index)),
            base_overrides: Vec::new(),
            base_override_index: None,
            slots: Vec::new(),
            free: Vec::new(),
            class_index: FastMap::default(),
            module_index: FastMap::default(),
            stats: frozen.stats,
            mark_scratch: Vec::new(),
            pending_scratch: Vec::new(),
            structural_keys_dirty: false,
            last_retained_structural_keys_len: frozen.structural_keys.len(),
            last_retained_key_equalities_len: frozen.key_equalities.len(),
            structural_keys: frozen.structural_keys.clone(),
            next_structural_key: frozen.next_structural_key,
            key_equalities: frozen.key_equalities.clone(),
            bytes_lists: frozen.bytes_lists.clone(),
            allocations_since_gc: 0,
            external_bytes_since_gc: 0,
        }
    }

    #[inline]
    fn owner(&self) -> HeapOwner {
        HeapOwner {
            identity: self.identity,
            base_id: self.base_id,
            base_len: self.base_len,
        }
    }

    #[inline]
    fn base_override_pos(&self, index: usize) -> Option<usize> {
        if self.base_overrides.is_empty() {
            return None;
        }
        if let Some(table) = self.base_override_index.as_deref() {
            let entry = table[index];
            return if entry != 0 {
                Some((entry - 1) as usize)
            } else {
                None
            };
        }
        for (pos, (ov_idx, _)) in self.base_overrides.iter().enumerate() {
            if *ov_idx == index {
                return Some(pos);
            }
        }
        None
    }

    #[inline(always)]
    fn slot_ref(&self, index: usize) -> Option<&Slot> {
        if index >= self.base_len {
            self.slots.get(index - self.base_len)
        } else {
            if let Some(pos) = self.base_override_pos(index) {
                return Some(&self.base_overrides[pos].1);
            }
            self.base_slots.as_deref()?.get(index)
        }
    }

    #[inline]
    fn slot_mut(&mut self, index: usize) -> &mut Slot {
        if index >= self.base_len {
            &mut self.slots[index - self.base_len]
        } else {
            if let Some(pos) = self.base_override_pos(index) {
                return &mut self.base_overrides[pos].1;
            }
            let cloned = self.base_slots.as_deref().expect("base slot")[index].clone();
            let pos = self.base_overrides.len();
            self.base_overrides.push((index, cloned));
            if let Some(table) = self.base_override_index.as_deref_mut() {
                table[index] = (pos + 1) as u32;
            } else if self.base_overrides.len() >= 4 {
                let mut table = vec![0u32; self.base_len].into_boxed_slice();
                for (p, &(ov_idx, _)) in self.base_overrides.iter().enumerate() {
                    table[ov_idx] = (p + 1) as u32;
                }
                self.base_override_index = Some(table);
            }
            &mut self.base_overrides[pos].1
        }
    }

    #[inline]
    pub(crate) fn allocations_since_gc(&self) -> usize {
        self.allocations_since_gc
    }

    #[inline]
    pub(crate) fn external_bytes_since_gc(&self) -> usize {
        self.external_bytes_since_gc
    }

    #[inline]
    pub(crate) fn note_external_allocation(&mut self, bytes: usize) {
        self.external_bytes_since_gc = self.external_bytes_since_gc.saturating_add(bytes);
    }

    pub fn allocate(&mut self, mut object: Object) -> ObjectId {
        object.forced = false;
        object.dynamic_regular_member_count = None;
        let module_unit = object.module_unit;
        let id = self.allocate_entry(Entry::Object(Box::new(object)));
        if let Some(unit) = module_unit {
            self.module_index.insert(unit.0, id);
        }
        id
    }

    pub(crate) fn register_module(&mut self, unit: crate::modules::UnitId, id: ObjectId) {
        self.module_index.insert(unit.0, id);
    }

    /// Allocates a strict dense `List` directly into a heap `Box<Object>` without a
    /// stack-to-heap `Object` copy.
    #[inline]
    pub(crate) fn allocate_strict_list(&mut self, elements: Vec<Value>) -> ObjectId {
        let len = elements.len();
        let mut boxed = Box::<Object>::default();
        boxed.collection_kind = CollectionKind::List;
        boxed.length = len;
        boxed.list_elements = Some(elements);
        self.allocate_entry(Entry::Object(boxed))
    }

    /// Allocates a `Function` object directly into a heap `Box<Object>` without a
    /// stack-to-heap `Object` copy.
    #[inline]
    pub(crate) fn allocate_function(
        &mut self,
        function: Function,
        method_declaration: Option<(ObjectId, usize)>,
        environment: Environment,
    ) -> ObjectId {
        let mut boxed = Box::<Object>::default();
        boxed.function = Some(function);
        boxed.method_declaration = method_declaration;
        boxed.environment = environment;
        self.allocate_entry(Entry::Object(boxed))
    }

    pub(crate) fn allocate_class(&mut self, prototype: ObjectId) -> ObjectId {
        let class_id = self
            .get(prototype)
            .ok()
            .and_then(|obj| obj.class_definition.as_ref().map(|c| c.id));
        let id = self.allocate_entry(Entry::Class(Box::new(ClassRecord {
            prototype,
            superclass: None,
            class_id,
            ancestors: [0; 8],
            ancestors_len: u8::MAX,
        })));
        if let Some(cid) = class_id {
            self.class_index.insert(cid, id);
        }
        self.get_mut(prototype)
            .expect("new class prototype")
            .instance_class = Some(id);
        id
    }

    #[inline(always)]
    pub fn get_class(&self, id: ObjectId) -> Result<&ClassRecord, RuntimeError> {
        if id.index >= self.base_len
            && id.heap == self.identity
            && let Some(slot) = self.slots.get(id.index - self.base_len)
            && slot.generation == id.generation
            && let Some(Entry::Class(class)) = &slot.entry
        {
            return Ok(class);
        }
        self.get_class_slow(id)
    }

    #[inline]
    fn get_class_slow(&self, id: ObjectId) -> Result<&ClassRecord, RuntimeError> {
        if id.index < self.base_len
            && id.heap == self.base_id
            && self.base_overrides.is_empty()
            && let Some(slot) = self.base_slots.as_deref().and_then(|s| s.get(id.index))
            && slot.generation == id.generation
            && let Some(Entry::Class(class)) = &slot.entry
        {
            return Ok(class);
        }
        self.get_class_cold(id)
    }

    #[cold]
    #[inline(never)]
    fn get_class_cold(&self, id: ObjectId) -> Result<&ClassRecord, RuntimeError> {
        match self.entry(id)? {
            Entry::Class(class) => Ok(class),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "value is not a class",
            )),
        }
    }

    pub(crate) fn link_superclass(
        &mut self,
        id: ObjectId,
        superclass: Option<ObjectId>,
    ) -> Result<(), RuntimeError> {
        let prototype = self.get_class(id)?.prototype;
        let self_class_id = self.get(prototype)?.class_definition.as_ref().map(|c| c.id);
        let (parent, ancestors, ancestors_len) = match superclass {
            Some(sup) => {
                let sup_rec = self.get_class(sup)?;
                let parent = Some(sup_rec.prototype);
                let mut ancestors = [0usize; 8];
                let ancestors_len = if sup != id
                    && sup_rec.ancestors_len != u8::MAX
                    && (sup_rec.ancestors_len as usize) < ancestors.len()
                    && let Some(cid) = self_class_id
                {
                    let len = sup_rec.ancestors_len as usize;
                    ancestors[0] = cid;
                    ancestors[1..1 + len].copy_from_slice(&sup_rec.ancestors[..len]);
                    (1 + len) as u8
                } else {
                    u8::MAX
                };
                (parent, ancestors, ancestors_len)
            }
            None => {
                let mut ancestors = [0usize; 8];
                let ancestors_len = if let Some(cid) = self_class_id {
                    ancestors[0] = cid;
                    1
                } else {
                    u8::MAX
                };
                (None, ancestors, ancestors_len)
            }
        };
        let slot = self.slot_mut(id.index);
        if let Some(Entry::Class(class)) = &mut slot.entry {
            class.superclass = superclass;
            class.class_id = self_class_id;
            class.ancestors = ancestors;
            class.ancestors_len = ancestors_len;
            slot.edges_valid = false;
        }
        if let Some(cid) = self_class_id {
            self.class_index.insert(cid, id);
        }
        let prototype = self.get_mut(prototype)?;
        prototype.parent = parent;
        prototype.class_superclass_initialized = true;
        Ok(())
    }

    pub(crate) fn class_definition(&self, id: ObjectId) -> Result<&Class, RuntimeError> {
        self.get(self.get_class(id)?.prototype)?
            .class_definition
            .as_deref()
            .ok_or_else(|| {
                RuntimeError::new(ErrorKind::InvalidBytecode, "missing class definition")
            })
    }

    pub(crate) fn class_simple_name(&self, id: ObjectId) -> Result<PklString, RuntimeError> {
        let prototype = self.get(self.get_class(id)?.prototype)?;
        if prototype.is_module {
            return Ok("ModuleClass".into());
        }
        Ok(self.class_definition(id)?.name.clone())
    }

    pub(crate) fn class_qualified_name(&self, id: ObjectId) -> Result<PklString, RuntimeError> {
        let prototype = self.get(self.get_class(id)?.prototype)?;
        let definition = self.class_definition(id)?;
        let module = if prototype.is_module {
            definition.name.clone()
        } else {
            definition.module.clone().unwrap_or_default()
        };
        Ok(module
            .concat(&"#".into())
            .concat(&self.class_simple_name(id)?))
    }

    pub(crate) fn class_display_name(&self, id: ObjectId) -> Result<PklString, RuntimeError> {
        let prototype = self.get(self.get_class(id)?.prototype)?;
        let definition = self.class_definition(id)?;
        let module = if prototype.is_module {
            definition.name.clone()
        } else {
            definition.module.clone().unwrap_or_default()
        };
        Ok(display_class_name(module, self.class_simple_name(id)?))
    }

    pub(crate) fn declaration_object(&self, value: &Value) -> Result<ObjectId, RuntimeError> {
        match value {
            Value::Class(id) => Ok(self.get_class(*id)?.prototype),
            _ => value.heap_id().ok_or_else(|| {
                RuntimeError::new(ErrorKind::InvalidBytecode, "declaration has no owner")
            }),
        }
    }

    #[inline]
    pub(crate) fn object_class(&self, id: ObjectId) -> Result<Option<ObjectId>, RuntimeError> {
        let obj0 = self.get(id)?;
        if let Some(class) = obj0.instance_class {
            self.get_class(class)?;
            return Ok(Some(class));
        }
        let Some(p1) = obj0.parent else {
            return Ok(None);
        };
        if p1 != id {
            let obj1 = self.get(p1)?;
            if let Some(class) = obj1.instance_class {
                self.get_class(class)?;
                return Ok(Some(class));
            }
            let Some(p2) = obj1.parent else {
                return Ok(None);
            };
            return self.object_class_deep(id, p1, p2);
        }
        Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"))
    }

    #[cold]
    #[inline(never)]
    fn object_class_deep(
        &self,
        id0: ObjectId,
        id1: ObjectId,
        mut id: ObjectId,
    ) -> Result<Option<ObjectId>, RuntimeError> {
        let mut inline_seen = [ObjectId {
            heap: 0,
            index: 0,
            generation: 0,
        }; 8];
        inline_seen[0] = id0;
        inline_seen[1] = id1;
        let mut inline_len = 2usize;
        let mut overflow: Option<BTreeSet<ObjectId>> = None;
        loop {
            if let Some(visited) = &mut overflow {
                if !visited.insert(id) {
                    return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
                }
            } else if inline_seen[..inline_len].contains(&id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            } else if inline_len < inline_seen.len() {
                inline_seen[inline_len] = id;
                inline_len += 1;
            } else {
                let mut set: BTreeSet<ObjectId> = inline_seen.into_iter().collect();
                set.insert(id);
                overflow = Some(set);
            }
            let object = self.get(id)?;
            if let Some(class) = object.instance_class {
                self.get_class(class)?;
                return Ok(Some(class));
            }
            let Some(parent) = object.parent else {
                return Ok(None);
            };
            id = parent;
        }
    }

    pub(crate) fn is_subclass(&self, class: ObjectId, expected: ObjectId) -> bool {
        let mut current = Some(class);
        let mut inline_seen = [ObjectId {
            heap: 0,
            index: 0,
            generation: 0,
        }; 8];
        let mut inline_len = 0usize;
        let mut overflow: Option<BTreeSet<ObjectId>> = None;
        while let Some(class) = current {
            if class == expected {
                return true;
            }
            if let Some(seen) = &mut overflow {
                if !seen.insert(class) {
                    return false;
                }
            } else if inline_seen[..inline_len].contains(&class) {
                return false;
            } else if inline_len < inline_seen.len() {
                inline_seen[inline_len] = class;
                inline_len += 1;
            } else {
                let mut set: BTreeSet<ObjectId> = inline_seen.into_iter().collect();
                set.insert(class);
                overflow = Some(set);
            }
            let Ok(record) = self.get_class(class) else {
                return false;
            };
            current = record.superclass;
        }
        false
    }

    /// O(depth) subclass check against a class definition ID, avoiding an O(heap) `find_class` scan.
    #[inline]
    pub(crate) fn class_has_ancestor_id(&self, class: ObjectId, target_id: usize) -> bool {
        let Ok(rec0) = self.get_class(class) else {
            return false;
        };
        if rec0.ancestors_len != u8::MAX {
            return rec0.ancestors[..rec0.ancestors_len as usize].contains(&target_id);
        }
        if rec0.class_id == Some(target_id)
            || (rec0.class_id.is_none()
                && self
                    .get_fast(rec0.prototype)
                    .and_then(|obj| obj.class_definition.as_ref())
                    .is_some_and(|def| def.id == target_id))
        {
            return true;
        }
        let Some(c1) = rec0.superclass else {
            return false;
        };
        if c1 == class {
            return false;
        }
        let Ok(rec1) = self.get_class(c1) else {
            return false;
        };
        if rec1.ancestors_len != u8::MAX {
            return rec1.ancestors[..rec1.ancestors_len as usize].contains(&target_id);
        }
        if rec1.class_id == Some(target_id)
            || (rec1.class_id.is_none()
                && self
                    .get_fast(rec1.prototype)
                    .and_then(|obj| obj.class_definition.as_ref())
                    .is_some_and(|def| def.id == target_id))
        {
            return true;
        }
        let Some(c2) = rec1.superclass else {
            return false;
        };
        self.class_has_ancestor_id_deep(class, c1, c2, target_id)
    }

    #[cold]
    #[inline(never)]
    fn class_has_ancestor_id_deep(
        &self,
        c0: ObjectId,
        c1: ObjectId,
        start: ObjectId,
        target_id: usize,
    ) -> bool {
        let mut current = Some(start);
        let mut inline_seen = [ObjectId {
            heap: 0,
            index: 0,
            generation: 0,
        }; 8];
        inline_seen[0] = c0;
        inline_seen[1] = c1;
        let mut inline_len = 2usize;
        let mut overflow: Option<BTreeSet<ObjectId>> = None;
        while let Some(cls) = current {
            if let Some(seen) = &mut overflow {
                if !seen.insert(cls) {
                    return false;
                }
            } else if inline_seen[..inline_len].contains(&cls) {
                return false;
            } else if inline_len < inline_seen.len() {
                inline_seen[inline_len] = cls;
                inline_len += 1;
            } else {
                let mut set: BTreeSet<ObjectId> = inline_seen.into_iter().collect();
                set.insert(cls);
                overflow = Some(set);
            }
            let Ok(record) = self.get_class(cls) else {
                return false;
            };
            if self
                .get_fast(record.prototype)
                .and_then(|obj| obj.class_definition.as_ref())
                .is_some_and(|def| def.id == target_id)
            {
                return true;
            }
            current = record.superclass;
        }
        false
    }

    pub(crate) fn allocate_null(&mut self, default: Option<Value>) -> Value {
        default.map_or(Value::Null, |value| {
            Value::NullWithDefault(self.allocate_entry(Entry::NullDefault(Box::new(value))))
        })
    }

    pub(crate) fn null_default(&self, id: ObjectId) -> Result<Value, RuntimeError> {
        match self.entry(id)? {
            Entry::NullDefault(value) => Ok((**value).clone()),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "invalid null default slot",
            )),
        }
    }

    pub(crate) fn validate_value(&self, value: &Value) -> Result<(), RuntimeError> {
        if let Value::NullWithDefault(id) = value {
            self.null_default(*id)?;
        } else if let Value::Class(id) = value {
            self.get_class(*id)?;
        } else if let Some(id) = value.heap_id() {
            self.get(id)?;
        }
        Ok(())
    }

    pub(crate) fn allocate_custom_this(&mut self, value: Option<Value>) -> CustomThisSlot {
        CustomThisSlot(self.allocate_entry(Entry::CustomThis(value.map(Box::new))))
    }

    pub(crate) fn custom_this(&self, slot: CustomThisSlot) -> Result<Option<Value>, RuntimeError> {
        match self.entry(slot.0)? {
            Entry::CustomThis(value) => Ok(value.as_deref().cloned()),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "invalid custom receiver slot",
            )),
        }
    }

    pub(crate) fn set_custom_this(
        &mut self,
        slot: CustomThisSlot,
        value: Option<Value>,
    ) -> Result<(), RuntimeError> {
        self.custom_this(slot)?;
        let s = self.slot_mut(slot.0.index);
        s.entry = Some(Entry::CustomThis(value.map(Box::new)));
        s.edges_valid = false;
        Ok(())
    }

    pub(crate) fn allocate_executable(&mut self) -> ExecutableOwnerId {
        ExecutableOwnerId(self.allocate_entry(Entry::Executable(Box::default())))
    }

    fn executable(&self, owner: ExecutableOwnerId) -> Result<&ExecutableState, RuntimeError> {
        match self.entry(owner.0)? {
            Entry::Executable(state) => Ok(state),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "invalid executable owner",
            )),
        }
    }

    pub(crate) fn regex_constructor_cache(
        &mut self,
        owner: ExecutableOwnerId,
    ) -> Result<&mut crate::pkl_regex::ConstructorCache, RuntimeError> {
        Ok(&mut self.executable_mut(owner)?.regex_constructor)
    }

    pub(crate) fn executable_is_live(&self, owner: ExecutableOwnerId) -> bool {
        self.executable(owner).is_ok()
    }

    pub(crate) fn set_code_roots(&mut self, owner: ExecutableOwnerId, roots: Vec<Value>) {
        self.executable_mut(owner)
            .expect("live code owner")
            .code_roots = roots;
    }

    #[cfg(test)]
    pub(crate) fn code_roots_for_test(&self, owner: ExecutableOwnerId) -> &[Value] {
        &self
            .executable(owner)
            .expect("live executable owner")
            .code_roots
    }

    fn executable_mut(
        &mut self,
        owner: ExecutableOwnerId,
    ) -> Result<&mut ExecutableState, RuntimeError> {
        self.executable(owner)?;
        let slot = self.slot_mut(owner.0.index);
        slot.edges_valid = false;
        match slot.entry.as_mut() {
            Some(Entry::Executable(state)) => Ok(state),
            _ => unreachable!("validated executable owner"),
        }
    }

    pub(crate) fn executable_child(
        &mut self,
        owner: ExecutableOwnerId,
        site: ExecutionSite,
    ) -> Result<ExecutableOwnerId, RuntimeError> {
        if let Some(child) = self.executable(owner)?.children.get(&site) {
            return Ok(*child);
        }
        let child = self.allocate_executable();
        self.executable_mut(owner)?.children.insert(site, child);
        Ok(child)
    }

    #[inline]
    pub(crate) fn existing_method_call_cache(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<Option<&crate::vm::deprecation::CallCache>, RuntimeError> {
        let calls = &self.executable(owner)?.method_calls;
        for (k, v) in calls {
            if *k == site {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    pub(crate) fn method_call_cache(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<&mut crate::vm::deprecation::CallCache, RuntimeError> {
        let calls = &mut self.executable_mut(owner)?.method_calls;
        if let Some(pos) = calls.iter().position(|(k, _)| *k == site) {
            return Ok(&mut calls[pos].1);
        }
        calls.push((site, crate::vm::deprecation::CallCache::default()));
        Ok(&mut calls.last_mut().unwrap().1)
    }

    #[inline]
    pub(crate) fn local_kind(
        &self,
        owner: ExecutableOwnerId,
        slot: (usize, usize),
    ) -> Result<Option<crate::vm::identity::SlotKind>, RuntimeError> {
        let kinds = &self.executable(owner)?.local_kinds;
        for &(k, v) in kinds {
            if k == slot {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    pub(crate) fn glob_mapping(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        pattern: &crate::value::PklString,
    ) -> Result<Option<ObjectId>, RuntimeError> {
        Ok(self
            .executable(owner)?
            .glob_mappings
            .get(&(site.0, site.1, pattern.clone()))
            .copied())
    }

    pub(crate) fn publish_glob_mapping(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        pattern: crate::value::PklString,
        mapping: ObjectId,
    ) -> Result<(), RuntimeError> {
        self.get(mapping)?;
        self.executable_mut(owner)?
            .glob_mappings
            .insert((site.0, site.1, pattern), mapping);
        Ok(())
    }

    pub(crate) fn imported_module(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<Option<ObjectId>, RuntimeError> {
        Ok(self.executable(owner)?.imported_modules.get(&site).copied())
    }

    pub(crate) fn publish_imported_module(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        receiver: ObjectId,
    ) -> Result<(), RuntimeError> {
        self.get(receiver)?;
        self.executable_mut(owner)?
            .imported_modules
            .insert(site, receiver);
        Ok(())
    }

    pub(crate) fn set_local_kind(
        &mut self,
        owner: ExecutableOwnerId,
        slot: (usize, usize),
        kind: crate::vm::identity::SlotKind,
    ) -> Result<(), RuntimeError> {
        let kinds = &mut self.executable_mut(owner)?.local_kinds;
        for (k, v) in kinds.iter_mut() {
            if *k == slot {
                *v = kind;
                return Ok(());
            }
        }
        kinds.push((slot, kind));
        Ok(())
    }

    pub(crate) fn ambiguous_locality(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<Option<bool>, RuntimeError> {
        Ok(self
            .executable(owner)?
            .ambiguous_localities
            .get(&site)
            .copied())
    }

    pub(crate) fn specialize_locality(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        local: bool,
    ) -> Result<(), RuntimeError> {
        self.executable_mut(owner)?
            .ambiguous_localities
            .insert(site, local);
        Ok(())
    }

    pub(crate) fn resolved_local(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<Option<Member>, RuntimeError> {
        Ok(self.executable(owner)?.local_properties.get(&site).cloned())
    }

    pub(crate) fn publish_local(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        member: Member,
    ) -> Result<(), RuntimeError> {
        self.executable_mut(owner)?
            .local_properties
            .insert(site, member);
        Ok(())
    }

    fn allocate_entry(&mut self, entry: Entry) -> ObjectId {
        self.stats.executable_live += usize::from(matches!(entry, Entry::Executable(_)));
        if matches!(&entry, Entry::Object(object) if object.structural_key.is_some()) {
            self.structural_keys_dirty = true;
        }
        let index = if let Some(index) = self.free.pop() {
            let local = index - self.base_len;
            self.slots[local].entry = Some(entry);
            self.slots[local].edges.clear();
            self.slots[local].edges_valid = false;
            index
        } else {
            self.slots.push(Slot {
                generation: 0,
                entry: Some(entry),
                edges: Vec::new(),
                edges_valid: false,
            });
            self.base_len + self.slots.len() - 1
        };
        self.stats.live += 1;
        self.stats.slots = self.base_len + self.slots.len();
        self.allocations_since_gc = self.allocations_since_gc.saturating_add(1);
        ObjectId {
            heap: self.identity,
            index,
            generation: self.slots[index - self.base_len].generation,
        }
    }

    pub(crate) fn find_type_alias(&self, name: &PklString) -> Option<ObjectId> {
        if let Some(found) = self.slots.iter().enumerate().find_map(|(local, slot)| {
            slot.object()
                .filter(|o| o.type_alias.as_ref() == Some(name))
                .map(|_| ObjectId {
                    heap: self.identity,
                    index: self.base_len + local,
                    generation: slot.generation,
                })
        }) {
            return Some(found);
        }
        if self.base_len > 0 {
            for index in 0..self.base_len {
                if let Some(slot) = self.slot_ref(index)
                    && slot
                        .object()
                        .is_some_and(|o| o.type_alias.as_ref() == Some(name))
                {
                    return Some(ObjectId {
                        heap: self.base_id,
                        index,
                        generation: slot.generation,
                    });
                }
            }
        }
        None
    }

    #[inline]
    pub(crate) fn find_module(&self, unit: crate::modules::UnitId) -> Option<ObjectId> {
        if let Some(&id) = self.module_index.get(&unit.0)
            && self
                .get_fast(id)
                .is_some_and(|object| object.module_unit == Some(unit))
        {
            return Some(id);
        }
        if let Some(&id) = self
            .base_module_index
            .as_deref()
            .and_then(|idx| idx.get(&unit.0))
            && self
                .get_fast(id)
                .is_some_and(|object| object.module_unit == Some(unit))
        {
            return Some(id);
        }
        None
    }

    #[inline]
    pub(crate) fn find_class(&self, class_id: usize) -> Option<ObjectId> {
        if let Some(&id) = self.class_index.get(&class_id)
            && let Ok(cls) = self.get_class(id)
            && self.get_fast(cls.prototype).is_some_and(|object| {
                object
                    .class_definition
                    .as_ref()
                    .is_some_and(|class| class.id == class_id)
                    && object.instance_class == Some(id)
            })
        {
            return Some(id);
        }
        if let Some(&id) = self
            .base_class_index
            .as_deref()
            .and_then(|idx| idx.get(&class_id))
            && let Ok(cls) = self.get_class(id)
            && self.get_fast(cls.prototype).is_some_and(|object| {
                object
                    .class_definition
                    .as_ref()
                    .is_some_and(|class| class.id == class_id)
                    && object.instance_class == Some(id)
            })
        {
            return Some(id);
        }
        None
    }

    #[inline(always)]
    fn entry(&self, id: ObjectId) -> Result<&Entry, RuntimeError> {
        if self.owner().owns(id)
            && let Some(slot) = self.slot_ref(id.index)
            && slot.generation == id.generation
            && let Some(entry) = &slot.entry
        {
            return Ok(entry);
        }
        Err(stale_handle_error())
    }

    #[inline(always)]
    pub fn get(&self, id: ObjectId) -> Result<&Object, RuntimeError> {
        if id.index >= self.base_len
            && id.heap == self.identity
            && let Some(slot) = self.slots.get(id.index - self.base_len)
            && slot.generation == id.generation
            && let Some(Entry::Object(object)) = &slot.entry
        {
            return Ok(object);
        }
        self.get_slow(id)
    }

    #[inline]
    fn get_slow(&self, id: ObjectId) -> Result<&Object, RuntimeError> {
        if id.index < self.base_len
            && id.heap == self.base_id
            && self.base_overrides.is_empty()
            && let Some(slot) = self.base_slots.as_deref().and_then(|s| s.get(id.index))
            && slot.generation == id.generation
            && let Some(Entry::Object(object)) = &slot.entry
        {
            return Ok(object);
        }
        self.get_cold(id)
    }

    #[cold]
    #[inline(never)]
    fn get_cold(&self, id: ObjectId) -> Result<&Object, RuntimeError> {
        match self.entry(id)? {
            Entry::Object(object) => Ok(object),
            Entry::Class(_)
            | Entry::CustomThis(_)
            | Entry::Executable(_)
            | Entry::NullDefault(_) => Err(non_object_slot_error()),
        }
    }

    /// Tight inlined lookup for verified hot loops that bail cleanly on `None`.
    #[inline(always)]
    pub(crate) fn get_fast(&self, id: ObjectId) -> Option<&Object> {
        if id.index >= self.base_len {
            if id.heap != self.identity {
                return None;
            }
            let slot = self.slots.get(id.index - self.base_len)?;
            if slot.generation != id.generation {
                return None;
            }
            match slot.entry.as_ref()? {
                Entry::Object(object) => Some(object),
                _ => None,
            }
        } else {
            self.get_fast_base(id)
        }
    }

    #[inline]
    fn get_fast_base(&self, id: ObjectId) -> Option<&Object> {
        if id.heap != self.base_id {
            return None;
        }
        let slot = if self.base_overrides.is_empty() {
            self.base_slots.as_deref()?.get(id.index)?
        } else {
            self.slot_ref(id.index)?
        };
        if slot.generation != id.generation {
            return None;
        }
        match slot.entry.as_ref()? {
            Entry::Object(object) => Some(object),
            _ => None,
        }
    }

    #[inline]
    pub fn get_mut(&mut self, id: ObjectId) -> Result<&mut Object, RuntimeError> {
        if id.index >= self.base_len {
            if id.heap != self.identity {
                return Err(stale_handle_error());
            }
            let Some(slot) = self.slots.get_mut(id.index - self.base_len) else {
                return Err(stale_handle_error());
            };
            if slot.generation != id.generation || slot.entry.is_none() {
                return Err(stale_handle_error());
            }
            if !matches!(slot.entry, Some(Entry::Object(_))) {
                return Err(non_object_slot_error());
            }
            slot.edges_valid = false;
            self.structural_keys_dirty = true;
            match slot.entry.as_mut() {
                Some(Entry::Object(object)) => Ok(object),
                _ => unreachable!("validated object slot"),
            }
        } else {
            self.get(id)?;
            self.structural_keys_dirty = true;
            let slot = self.slot_mut(id.index);
            slot.edges_valid = false;
            match slot.entry.as_mut() {
                Some(Entry::Object(object)) => Ok(object),
                _ => unreachable!("validated object slot"),
            }
        }
    }
}

#[cold]
#[inline(never)]
fn stale_handle_error() -> RuntimeError {
    RuntimeError::new(
        ErrorKind::StaleHandle,
        "object does not belong to this live heap",
    )
}

#[cold]
#[inline(never)]
fn non_object_slot_error() -> RuntimeError {
    RuntimeError::new(ErrorKind::StaleHandle, "internal slot is not a Pkl object")
}

#[derive(Clone, Copy)]
struct HeapOwner {
    identity: u64,
    base_id: u64,
    base_len: usize,
}

impl HeapOwner {
    #[inline(always)]
    fn owns(self, id: ObjectId) -> bool {
        if id.index >= self.base_len {
            id.heap == self.identity
        } else {
            id.heap == self.base_id
        }
    }
}

type BytesListMap = BTreeMap<crate::value_identity::Token, (std::sync::Weak<()>, ObjectId, bool)>;

fn resolve_traced_id(bytes_lists: &BytesListMap, value: &Value) -> Option<ObjectId> {
    if let Value::Bytes(bytes) = value {
        if bytes_lists.is_empty() {
            None
        } else {
            bytes_lists.get(&bytes.reference()).map(|(_, id, _)| *id)
        }
    } else {
        value.heap_id()
    }
}

#[inline]
fn push_id(owner: HeapOwner, out: &mut Vec<ObjectId>, id: ObjectId) {
    if owner.owns(id) {
        out.push(id);
    }
}

#[inline]
fn push_value(
    owner: HeapOwner,
    bytes_lists: &BytesListMap,
    out: &mut Vec<ObjectId>,
    value: &Value,
) {
    if let Some(id) = resolve_traced_id(bytes_lists, value)
        && owner.owns(id)
    {
        out.push(id);
    }
}

fn trace_environment_edges(
    owner: HeapOwner,
    bytes_lists: &BytesListMap,
    environment: &Environment,
    out: &mut Vec<ObjectId>,
) {
    if let Some(slot) = environment.custom_this {
        push_id(owner, out, slot.0);
    }
    if let Some(exec) = environment.executable_owner {
        push_id(owner, out, exec.0);
    }
    if let Some(exec) = environment.code_owner {
        push_id(owner, out, exec.0);
    }
    for &(exec, _) in environment.local_owners.values() {
        push_id(owner, out, exec.0);
    }
    for scope in &environment.scopes {
        push_id(owner, out, scope.receiver);
        push_id(owner, out, scope.owner);
    }
    for value in &environment.locals {
        push_value(owner, bytes_lists, out, value);
    }
    if let Some(value) = &environment.this_value {
        push_value(owner, bytes_lists, out, value);
    }
    if let Some(value) = &environment.outer_value {
        push_value(owner, bytes_lists, out, value);
    }
}

fn trace_entry_edges(
    owner: HeapOwner,
    bytes_lists: &BytesListMap,
    entry: &Entry,
    out: &mut Vec<ObjectId>,
) {
    let identity = owner;
    let object = match entry {
        Entry::Class(class) => {
            push_id(identity, out, class.prototype);
            if let Some(superclass) = class.superclass {
                push_id(identity, out, superclass);
            }
            return;
        }
        Entry::NullDefault(value) => {
            push_value(identity, bytes_lists, out, value);
            return;
        }
        Entry::Executable(state) => {
            for value in &state.code_roots {
                push_value(identity, bytes_lists, out, value);
            }
            for owner in state.children.values() {
                push_id(identity, out, owner.0);
            }
            for &id in state.imported_modules.values() {
                push_id(identity, out, id);
            }
            for &id in state.glob_mappings.values() {
                push_id(identity, out, id);
            }
            for (_, cache) in &state.method_calls {
                cache.trace(out);
            }
            return;
        }
        Entry::CustomThis(value) => {
            if let Some(value) = value {
                push_value(identity, bytes_lists, out, value);
            }
            return;
        }
        Entry::Object(object) => object,
    };
    if let Some(mirror) = &object.mirror {
        mirror.for_each_edge(
            out,
            |out, id| push_id(identity, out, id),
            |out, value| push_value(identity, bytes_lists, out, value),
        );
    }
    if let Some(parent) = object.parent {
        push_id(identity, out, parent);
    }
    if let Some(class) = object.instance_class {
        push_id(identity, out, class);
    }
    if let Some((owner, _)) = object.method_declaration {
        push_id(identity, out, owner);
    }
    if let Some(amendment) = &object.amendment {
        push_value(identity, bytes_lists, out, &amendment.parent);
    }
    if let Some(pair) = &object.pair {
        for value in pair.iter() {
            push_value(identity, bytes_lists, out, value);
        }
    }
    if let Some(entry) = &object.foreign_map_entry {
        for value in entry.iter() {
            push_value(identity, bytes_lists, out, value);
        }
    }
    for value in &object.annotations {
        push_value(identity, bytes_lists, out, value);
    }
    for values in object.member_annotations.values() {
        for value in values {
            push_value(identity, bytes_lists, out, value);
        }
    }
    if let Some(node) = object.collection_cast.as_ref().and_then(|cast| cast.node) {
        push_id(identity, out, (node.0).0);
    }
    if let Some(function) = &object.function
        && let Some(captured) = &function.captured_this
    {
        push_value(identity, bytes_lists, out, captured);
    }
    if let Some(reference) = &object.reference {
        push_value(identity, bytes_lists, out, &reference.domain);
        push_value(identity, bytes_lists, out, &reference.data);
        for value in &reference.path {
            push_value(identity, bytes_lists, out, value);
        }
        for &class in &reference.classes {
            push_id(identity, out, class);
        }
    }
    if let Some(native) = &object.native_function {
        push_value(identity, bytes_lists, out, &native.receiver);
    }
    if let Some(order) = object.map_key_order.as_deref() {
        for member in order {
            push_value(identity, bytes_lists, out, &member.key);
        }
    }
    if let Some(mapping_keys) = object.mapping_keys {
        push_id(identity, out, mapping_keys);
    }
    for member in &object.collection_members {
        push_value(identity, bytes_lists, out, &member.key);
    }
    if let Some(elements) = &object.list_elements {
        for value in elements {
            push_value(identity, bytes_lists, out, value);
        }
    }
    if let Some(iterator) = &object.iterator {
        push_id(identity, out, iterator.source);
        for member in iterator.members.iter().chain(&iterator.force_members) {
            push_value(identity, bytes_lists, out, &member.key);
        }
    }
    if let Some(delegate) = &object.conversion_delegate {
        push_id(identity, out, delegate.source);
    }
    if !bytes_lists.is_empty()
        && let Some(iterator) = &object.bytes_iterator
        && let Some((_, id, _)) = bytes_lists.get(&iterator.0.reference())
    {
        push_id(identity, out, *id);
    }
    if let Some(iterator) = &object.predicate_iterator {
        push_id(identity, out, iterator.source);
        if let Some(previous) = &iterator.previous_this {
            push_value(identity, bytes_lists, out, previous);
        }
        for (owner, member) in &iterator.members {
            push_id(identity, out, *owner);
            push_value(identity, bytes_lists, out, &member.key);
        }
    }
    for environment in object.member_environments.values() {
        trace_environment_edges(identity, bytes_lists, environment, out);
    }
    if let Some(cast) = &object.collection_cast {
        trace_environment_edges(identity, bytes_lists, &cast.environment, out);
    }
    if let Some(mirror) = object.declaration_mirror {
        push_id(identity, out, mirror);
    }
    if let Some(mirror) = object.module_mirror {
        push_id(identity, out, mirror);
    }
    for values in object.mirror_annotations.values() {
        for value in values {
            push_value(identity, bytes_lists, out, value);
        }
    }
    trace_environment_edges(identity, bytes_lists, &object.environment, out);
    if let Some(Some(value)) = &object.collection_default {
        push_value(identity, bytes_lists, out, value);
    }
    for value in object.member_values.values() {
        push_value(identity, bytes_lists, out, value);
    }
    for value in &object.set_entries {
        push_value(identity, bytes_lists, out, value);
    }
    for slot in &object.cache_slots {
        push_value(identity, bytes_lists, out, &slot.value);
        if let Some(entry_key) = &slot.entry_key {
            push_value(identity, bytes_lists, out, entry_key);
        }
        if let Some(owner) = slot.key.owner {
            push_id(identity, out, owner);
        }
    }
    if let Some(owner) = object.evaluating_key.as_ref().and_then(|k| k.owner) {
        push_id(identity, out, owner);
    }
    for key in &object.evaluating_overflow {
        if let Some(owner) = key.owner {
            push_id(identity, out, owner);
        }
    }
    for (key, _) in object.cache_map.iter() {
        if let Some(owner) = key.owner {
            push_id(identity, out, owner);
        }
    }
}

impl Heap {
    pub fn stats(&self) -> HeapStats {
        self.stats
    }

    /// Trace all object edges, including lazy captures and completed values, iteratively.
    /// The caller must include every externally retained handle in `roots`.
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub fn collect(&mut self, roots: &[Value]) -> HeapStats {
        let owner = self.owner();
        let base_len = self.base_len;
        let slot_count = base_len + self.slots.len();
        let mut marked = std::mem::take(&mut self.mark_scratch);
        marked.clear();
        marked.resize(slot_count, false);
        let mut pending = std::mem::take(&mut self.pending_scratch);
        pending.clear();
        if pending.capacity() < slot_count.max(roots.len()) {
            pending.reserve(slot_count.max(roots.len()) - pending.capacity());
        }
        for value in roots {
            push_value(owner, &self.bytes_lists, &mut pending, value);
        }
        // Any copy-on-write override on the immutable `pkl:base` prefix may hold
        // newly attached edges into user slots (`>= base_len`), e.g. cached mirrors.
        for (ov_idx, ov_slot) in &self.base_overrides {
            if ov_slot.entry.is_some() {
                pending.push(ObjectId {
                    heap: self.base_id,
                    index: *ov_idx,
                    generation: ov_slot.generation,
                });
            }
        }
        if !self.bytes_lists.is_empty() {
            for (lifetime, id, constant) in self.bytes_lists.values() {
                if *constant && lifetime.strong_count() != 0 && owner.owns(*id) {
                    pending.push(*id);
                }
            }
        }
        // Per-slot cached outgoing edges are only used/populated when bytes_lists is
        // empty, so Value::Bytes backlink insertions can never make cached edges stale.
        let cache_edges = self.bytes_lists.is_empty();
        while let Some(id) = pending.pop() {
            if !owner.owns(id) || id.index >= slot_count || marked[id.index] {
                continue;
            }
            if id.index < base_len {
                let ov_pos = if let Some(table) = self.base_override_index.as_deref() {
                    let e = table[id.index];
                    if e != 0 { Some((e - 1) as usize) } else { None }
                } else {
                    self.base_overrides.iter().position(|(i, _)| *i == id.index)
                };
                if let Some(pos) = ov_pos {
                    let slot = &self.base_overrides[pos].1;
                    if slot.generation != id.generation || slot.entry.is_none() {
                        continue;
                    }
                    marked[id.index] = true;
                    if cache_edges {
                        if !self.base_overrides[pos].1.edges_valid {
                            let mut edges = std::mem::take(&mut self.base_overrides[pos].1.edges);
                            edges.clear();
                            trace_entry_edges(
                                owner,
                                &self.bytes_lists,
                                self.base_overrides[pos]
                                    .1
                                    .entry
                                    .as_ref()
                                    .expect("live entry"),
                                &mut edges,
                            );
                            if edges.len() > 1 {
                                edges.sort_unstable_by_key(|edge| (edge.index, edge.generation));
                                edges.dedup();
                            }
                            self.base_overrides[pos].1.edges = edges;
                            self.base_overrides[pos].1.edges_valid = true;
                        }
                        for &edge in &self.base_overrides[pos].1.edges {
                            if edge.index < slot_count && !marked[edge.index] {
                                pending.push(edge);
                            }
                        }
                    } else {
                        trace_entry_edges(
                            owner,
                            &self.bytes_lists,
                            self.base_overrides[pos]
                                .1
                                .entry
                                .as_ref()
                                .expect("live entry"),
                            &mut pending,
                        );
                    }
                } else if let Some(base) = self.base_slots.as_deref()
                    && base[id.index].generation == id.generation
                    && base[id.index].entry.is_some()
                {
                    // Unmodified `base_slots` entries only reference `< base_len`.
                    marked[id.index] = true;
                }
                continue;
            }
            let local = id.index - base_len;
            let slot = &self.slots[local];
            if slot.generation != id.generation || slot.entry.is_none() {
                continue;
            }
            marked[id.index] = true;
            if cache_edges {
                if !self.slots[local].edges_valid {
                    let mut edges = std::mem::take(&mut self.slots[local].edges);
                    edges.clear();
                    trace_entry_edges(
                        owner,
                        &self.bytes_lists,
                        self.slots[local].entry.as_ref().expect("live entry"),
                        &mut edges,
                    );
                    if edges.len() > 1 {
                        edges.sort_unstable_by_key(|edge| (edge.index, edge.generation));
                        edges.dedup();
                    }
                    self.slots[local].edges = edges;
                    self.slots[local].edges_valid = true;
                }
                for &edge in &self.slots[local].edges {
                    if edge.index < slot_count && !marked[edge.index] {
                        pending.push(edge);
                    }
                }
            } else {
                trace_entry_edges(
                    owner,
                    &self.bytes_lists,
                    self.slots[local].entry.as_ref().expect("live entry"),
                    &mut pending,
                );
            }
        }
        let mut reclaimed_this_pass = 0usize;
        for (local, slot) in self.slots.iter_mut().enumerate() {
            let index = base_len + local;
            if !marked[index]
                && let Some(entry) = slot.entry.take()
            {
                slot.edges.clear();
                slot.edges_valid = false;
                reclaimed_this_pass += 1;
                self.stats.executable_live -= usize::from(matches!(entry, Entry::Executable(_)));
                self.stats.live -= 1;
                self.stats.reclaimed += 1;
                // Exhausted generations are retired rather than wrapped.
                if let Some(generation) = slot.generation.checked_add(1) {
                    slot.generation = generation;
                    self.free.push(index);
                }
            }
        }
        if reclaimed_this_pass > 0 {
            let slots = &self.slots;
            let identity = self.identity;
            self.class_index.retain(|_, id| {
                if id.index >= base_len {
                    id.heap == identity
                        && slots
                            .get(id.index - base_len)
                            .is_some_and(|s| s.generation == id.generation && s.entry.is_some())
                } else {
                    true
                }
            });
            self.module_index.retain(|_, id| {
                if id.index >= base_len {
                    id.heap == identity
                        && slots
                            .get(id.index - base_len)
                            .is_some_and(|s| s.generation == id.generation && s.entry.is_some())
                } else {
                    true
                }
            });
        }
        let slot_live_object = |base_slots: Option<&[Slot]>,
                                base_overrides: &[(usize, Slot)],
                                base_override_index: Option<&[u32]>,
                                slots: &[Slot],
                                id: ObjectId|
         -> bool {
            if !owner.owns(id) {
                return false;
            }
            let slot = if id.index >= base_len {
                slots.get(id.index - base_len)
            } else if let Some(table) = base_override_index {
                let e = table[id.index];
                if e != 0 {
                    Some(&base_overrides[(e - 1) as usize].1)
                } else {
                    base_slots.and_then(|b| b.get(id.index))
                }
            } else if let Some((_, ov)) = base_overrides.iter().find(|(i, _)| *i == id.index) {
                Some(ov)
            } else {
                base_slots.and_then(|b| b.get(id.index))
            };
            slot.is_some_and(|s| s.generation == id.generation && s.object().is_some())
        };
        if !self.bytes_lists.is_empty() {
            let base_slots = self.base_slots.as_deref();
            let base_overrides = &self.base_overrides;
            let base_override_index = self.base_override_index.as_deref();
            let slots = &self.slots;
            self.bytes_lists.retain(|_, (lifetime, id, _)| {
                lifetime.strong_count() != 0
                    && slot_live_object(base_slots, base_overrides, base_override_index, slots, *id)
            });
        }
        if self.key_equalities.is_empty() {
            self.last_retained_key_equalities_len = 0;
        } else if reclaimed_this_pass > 0
            || self.key_equalities.len() != self.last_retained_key_equalities_len
        {
            let base_slots = self.base_slots.as_deref();
            let base_overrides = &self.base_overrides;
            let base_override_index = self.base_override_index.as_deref();
            let slots = &self.slots;
            self.key_equalities.retain(|(a, b)| {
                slot_live_object(base_slots, base_overrides, base_override_index, slots, *a)
                    && slot_live_object(base_slots, base_overrides, base_override_index, slots, *b)
            });
            self.last_retained_key_equalities_len = self.key_equalities.len();
        }
        // Canonical keys are metadata, not roots. Retain only keys referenced by
        // live objects and their structural components.
        if self.structural_keys.is_empty() {
            self.structural_keys_dirty = false;
            self.last_retained_structural_keys_len = 0;
        } else if reclaimed_this_pass > 0
            || self.structural_keys_dirty
            || self.structural_keys.len() != self.last_retained_structural_keys_len
        {
            let by_id: BTreeMap<_, _> = self
                .structural_keys
                .iter()
                .map(|(key, id)| (*id, key))
                .collect();
            let mut key_pending: Vec<_> = self
                .slots
                .iter()
                .filter_map(|slot| slot.object().and_then(|o| o.structural_key))
                .collect();
            for (_, ov_slot) in &self.base_overrides {
                if let Some(key) = ov_slot.object().and_then(|o| o.structural_key) {
                    key_pending.push(key);
                }
            }
            if self.base_has_structural_keys
                && let Some(base) = self.base_slots.as_deref()
            {
                for slot in base {
                    if let Some(key) = slot.object().and_then(|o| o.structural_key) {
                        key_pending.push(key);
                    }
                }
            }
            let mut retained = std::collections::BTreeSet::new();
            while let Some(id) = key_pending.pop() {
                if retained.insert(id)
                    && let Some(key) = by_id.get(&id)
                {
                    key_pending.extend(key.children());
                }
            }
            self.structural_keys.retain(|_, id| retained.contains(id));
            self.structural_keys_dirty = false;
            self.last_retained_structural_keys_len = self.structural_keys.len();
        }
        self.mark_scratch = marked;
        self.pending_scratch = pending;
        self.stats.collections += 1;
        self.allocations_since_gc = 0;
        self.external_bytes_since_gc = 0;
        self.stats
    }

    pub fn clear(&mut self) {
        self.collect(&[]);
    }

    /// Permanently discard this arena's backing storage; all old IDs become invalid.
    pub fn release(&mut self) {
        self.stats.reclaimed += self.stats.live;
        self.stats.live = 0;
        self.stats.executable_live = 0;
        self.stats.slots = 0;
        self.stats.collections += 1;
        self.allocations_since_gc = 0;
        self.external_bytes_since_gc = 0;
        self.base_slots = None;
        self.base_len = 0;
        self.base_id = 0;
        self.base_has_structural_keys = false;
        self.base_class_index = None;
        self.base_module_index = None;
        self.base_overrides = Vec::new();
        self.base_override_index = None;
        self.slots = Vec::new();
        self.free = Vec::new();
        self.class_index.clear();
        self.module_index.clear();
        self.mark_scratch = Vec::new();
        self.pending_scratch = Vec::new();
        self.structural_keys_dirty = false;
        self.last_retained_structural_keys_len = 0;
        self.last_retained_key_equalities_len = 0;
        self.structural_keys = BTreeMap::new();
        self.key_equalities.clear();
        self.bytes_lists.clear();
        self.identity = NEXT_HEAP.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_defaults_are_null_values_with_checked_private_heap_edges() {
        let mut heap = Heap::new();
        let value = heap.allocate_null(Some(Value::Int(7.into())));
        assert_eq!(value, Value::Null);
        let id = value.heap_id().expect("default payload is traced");
        assert_eq!(heap.null_default(id), Ok(Value::Int(7.into())));
        assert_eq!(heap.get(id).unwrap_err().kind, ErrorKind::StaleHandle);
        assert_eq!(
            Heap::new().null_default(id).unwrap_err().kind,
            ErrorKind::StaleHandle
        );
        assert_eq!(heap.collect(std::slice::from_ref(&value)).live, 1);
        assert_eq!(heap.collect(&[]).live, 0);
        let replacement = heap.allocate_null(Some(Value::Int(8.into())));
        assert_ne!(replacement.heap_id(), Some(id));
        assert_eq!(
            heap.null_default(id).unwrap_err().kind,
            ErrorKind::StaleHandle
        );
        assert_eq!(heap.allocate_null(None), Value::Null);
    }

    #[test]
    fn deep_nullable_defaults_trace_and_release_iteratively_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut heap = Heap::new();
                let mut value = Value::Object(heap.allocate(Object::default()));
                for _ in 0..20_000 {
                    value = heap.allocate_null(Some(value));
                }
                assert_eq!(heap.collect(std::slice::from_ref(&value)).live, 20_001);
                assert_eq!(heap.collect(&[]).live, 0);
                value = Value::Object(heap.allocate(Object::default()));
                for _ in 0..20_000 {
                    value = heap.allocate_null(Some(value));
                }
                // Arena destruction releases edges as handles, not Rust recursion.
                drop(heap);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn captured_executable_graphs_trace_children_without_retaining_receivers() {
        let mut heap = Heap::new();
        let owner = heap.allocate_executable();
        let child = heap
            .executable_child(owner, ExecutionSite::Member(7))
            .unwrap();
        let capture = heap.allocate(Object {
            environment: Environment {
                executable_owner: Some(owner),
                ..Environment::default()
            },
            ..Object::default()
        });
        let retained = heap.collect(&[Value::Object(capture)]);
        assert_eq!(retained.live, 3);
        assert_eq!(retained.executable_live, 2);
        assert_eq!(
            heap.executable_child(owner, ExecutionSite::Member(7))
                .unwrap(),
            child
        );
        assert!(heap.get(owner.0).is_err());
        // A static owner may persist, but must not keep a prior lexical receiver.
        let retained = heap.collect(&[owner.root()]);
        assert_eq!(retained.live, 2);
        assert_eq!(retained.executable_live, 2);
        assert!(heap.get(capture).is_err());
        assert_eq!(heap.collect(&[]).live, 0);
        assert_eq!(heap.stats().executable_live, 0);
        let replacement = heap.allocate_executable();
        assert!(
            heap.executable_child(owner, ExecutionSite::Member(7))
                .is_err()
        );
        assert!(
            Heap::new()
                .executable_child(replacement, ExecutionSite::Member(7))
                .is_err()
        );
        heap.release();
        assert!(
            heap.executable_child(replacement, ExecutionSite::Member(7))
                .is_err()
        );
    }

    #[test]
    fn lazy_cast_retains_its_type_node_without_a_live_execution_frame() {
        let mut heap = Heap::new();
        let owner = heap.allocate_executable();
        let node = TypeNodeId(
            heap.executable_child(owner, ExecutionSite::TypeChild(0))
                .unwrap(),
        );
        let wrapper = heap.allocate(Object {
            collection_cast: Some(Box::new(CollectionCast {
                source: None,
                node: Some(node),
                ty: crate::vm::TypeCheck::Int,
                span: crate::source::Span::new(0, 1),
                member: "#values".into(),
                environment: Environment::default(),
            })),
            ..Object::default()
        });
        let retained = heap.collect(&[Value::Object(wrapper)]);
        assert_eq!(retained.live, 2);
        assert_eq!(retained.executable_live, 1);
        assert!(heap.executable(owner).is_err());
        assert!(heap.executable(node.0).is_ok());
        assert_eq!(heap.collect(&[]).live, 0);
        assert!(heap.executable(node.0).is_err());
    }

    #[test]
    fn custom_receiver_cycles_are_traced_and_reclaimed_in_the_object_arena() {
        let mut heap = Heap::new();
        let slot = heap.allocate_custom_this(None);
        let object = heap.allocate(Object {
            environment: Environment {
                custom_this: Some(slot),
                ..Environment::default()
            },
            ..Object::default()
        });
        heap.set_custom_this(slot, Some(Value::Object(object)))
            .unwrap();
        assert_eq!(heap.collect(&[slot.root()]).live, 2);
        assert_eq!(heap.custom_this(slot).unwrap(), Some(Value::Object(object)));
        assert!(heap.get(object).is_ok());
        assert!(
            heap.get(slot.0).is_err(),
            "internal slots are not Pkl objects"
        );
        assert_eq!(heap.collect(&[Value::Object(object)]).live, 2);
        assert_eq!(heap.collect(&[]).live, 0);
        assert!(heap.custom_this(slot).is_err());
        let replacement = heap.allocate_custom_this(Some(Value::Null));
        assert!(
            heap.custom_this(slot).is_err(),
            "reused slots reject old generations"
        );
        assert_eq!(heap.custom_this(replacement).unwrap(), Some(Value::Null));
        assert!(Heap::new().custom_this(replacement).is_err());
        heap.release();
        assert!(heap.custom_this(replacement).is_err());
        assert_eq!(heap.stats().slots, 0);
    }

    #[test]
    fn function_snapshot_survives_an_independent_receiver_slot_mutation() {
        let mut heap = Heap::new();
        let snapshot = heap.allocate(Object::default());
        let slot = heap.allocate_custom_this(Some(Value::Object(snapshot)));
        let function = heap.allocate(Object {
            function: Some(Function {
                block: 0,
                arity: 0,
                method: false,
                captured_this: Some(Value::Object(snapshot)),
            }),
            environment: Environment {
                custom_this: Some(slot),
                ..Environment::default()
            },
            ..Object::default()
        });
        heap.set_custom_this(slot, Some(Value::Function(function)))
            .unwrap();
        assert_eq!(heap.collect(&[Value::Function(function)]).live, 3);
        assert!(heap.get(snapshot).is_ok());
        assert_eq!(
            heap.custom_this(slot).unwrap(),
            Some(Value::Function(function))
        );
        assert_eq!(heap.collect(&[]).live, 0);
    }

    #[test]
    fn class_and_module_indexes_track_base_local_gc_and_release() {
        let mut base = Heap::new();
        let base_unit = crate::modules::UnitId(7);
        let base_mod = base.allocate(Object {
            is_module: true,
            module_unit: Some(base_unit),
            ..Object::default()
        });
        let base_proto = base.allocate(Object {
            class_definition: Some(Box::new(Class {
                id: 101,
                name: "BaseClass".into(),
                module: None,
                origin_uri: None,
                open: true,
                abstract_: false,
            })),
            ..Object::default()
        });
        let base_cls = base.allocate_class(base_proto);
        assert_eq!(base.find_module(base_unit), Some(base_mod));
        assert_eq!(base.find_class(101), Some(base_cls));

        let frozen = base.freeze_base();
        let mut heap = Heap::from_frozen_base(&frozen);
        assert_eq!(heap.find_module(base_unit), Some(base_mod));
        assert_eq!(heap.find_class(101), Some(base_cls));

        let local_unit = crate::modules::UnitId(42);
        let local_mod = heap.allocate(Object {
            is_module: true,
            module_unit: Some(local_unit),
            ..Object::default()
        });
        let local_proto = heap.allocate(Object {
            class_definition: Some(Box::new(Class {
                id: 202,
                name: "LocalClass".into(),
                module: None,
                origin_uri: None,
                open: false,
                abstract_: false,
            })),
            ..Object::default()
        });
        let local_cls = heap.allocate_class(local_proto);
        assert_eq!(heap.find_module(local_unit), Some(local_mod));
        assert_eq!(heap.find_class(202), Some(local_cls));

        // GC keeping only local_cls reclaims local_mod and removes it from module_index.
        heap.collect(&[Value::Class(local_cls)]);
        assert_eq!(heap.find_module(local_unit), None);
        assert_eq!(heap.find_class(202), Some(local_cls));
        assert_eq!(heap.find_module(base_unit), Some(base_mod));
        assert_eq!(heap.find_class(101), Some(base_cls));

        // Full GC reclaims local_cls too.
        heap.collect(&[]);
        assert_eq!(heap.find_class(202), None);

        // release() clears base and local indexes.
        heap.release();
        assert_eq!(heap.find_module(base_unit), None);
        assert_eq!(heap.find_class(101), None);
    }

    #[test]
    fn compact_heap_object_and_slot_layouts() {
        let env_size = std::mem::size_of::<Environment>();
        let slot_size = std::mem::size_of::<Slot>();
        let obj_size = std::mem::size_of::<Object>();
        let instr_size = std::mem::size_of::<crate::vm::Instruction>();
        let ty_size = std::mem::size_of::<crate::vm::TypeCheck>();
        let member_size = std::mem::size_of::<Member>();
        eprintln!(
            "sizes: Environment={env_size}, Slot={slot_size}, Object={obj_size}, Instruction={instr_size}, TypeCheck={ty_size}, Member={member_size}"
        );
        assert!(env_size <= 320, "Environment grew to {env_size} bytes");
        assert!(slot_size <= 64, "Slot grew to {slot_size} bytes");
        assert!(obj_size <= 1280, "Object grew to {obj_size} bytes");
        assert!(
            instr_size <= 64,
            "Instruction grew to {instr_size} bytes (must fit in one 64-byte cache line)"
        );
        assert!(ty_size <= 112, "TypeCheck grew to {ty_size} bytes");
    }

    #[test]
    fn object_cache_inline_promotion_evaluating_and_alias() {
        let mut obj = Object::default();
        assert!(obj.cache_is_empty());

        let mk_key = |name: &str| CacheKey {
            entry: false,
            owner: None,
            name: PklString::from(name),
            method: false,
        };

        // Inline evaluating marker (0 -> 1 -> 2 -> 1 -> 0).
        let k_a = mk_key("a");
        let k_b = mk_key("b");
        obj.cache_mark_evaluating(k_a.clone());
        assert!(obj.has_evaluating_keys());
        assert!(obj.evaluating_overflow.is_empty());
        assert!(matches!(obj.cache_get(&k_a), Some(CacheEntry::Evaluating)));
        obj.cache_mark_evaluating(k_b.clone());
        assert_eq!(obj.evaluating_overflow.len(), 1);
        assert!(matches!(obj.cache_get(&k_b), Some(CacheEntry::Evaluating)));
        obj.cache_clear_evaluating(&k_a);
        assert!(obj.cache_get(&k_a).is_none());
        assert!(matches!(obj.cache_get(&k_b), Some(CacheEntry::Evaluating)));
        assert!(obj.evaluating_overflow.is_empty());
        obj.push_completed_slot(k_b.clone(), None, Value::Int(42.into()));
        assert!(!obj.has_evaluating_keys());
        assert_eq!(obj.cache_get_completed(&k_b), Some(&Value::Int(42.into())));

        // Up to 16 slots stay inline without allocating cache_map.
        for i in 1..16 {
            obj.push_completed_slot(
                mk_key(&format!("k{i}")),
                None,
                Value::Int((i as i64).into()),
            );
        }
        assert_eq!(obj.cache_slots.len(), 16);
        assert!(obj.cache_map.is_empty());
        assert_eq!(
            obj.cache_get_completed(&mk_key("k15")),
            Some(&Value::Int(15.into()))
        );

        // 17th slot promotes to cache_map.
        obj.push_completed_slot(mk_key("k16"), None, Value::Int(16.into()));
        assert_eq!(obj.cache_slots.len(), 17);
        assert!(!obj.cache_map.is_empty());
        assert_eq!(
            obj.cache_get_completed(&mk_key("k16")),
            Some(&Value::Int(16.into()))
        );
        assert_eq!(obj.cache_get_completed(&k_b), Some(&Value::Int(42.into())));

        // PublishMatch alias on an inline-sized object promotes cache_map and serves both keys.
        let mut aliased = Object::default();
        let orig = mk_key("orig");
        let alias = mk_key("alias");
        aliased.push_completed_slot(orig.clone(), None, Value::Int(1.into()));
        assert!(aliased.cache_map.is_empty());
        aliased.update_completed_slot(0, alias.clone(), Value::Int(99.into()));
        assert!(!aliased.cache_map.is_empty());
        assert_eq!(
            aliased.cache_get_completed(&orig),
            Some(&Value::Int(99.into()))
        );
        assert_eq!(
            aliased.cache_get_completed(&alias),
            Some(&Value::Int(99.into()))
        );
    }
}
