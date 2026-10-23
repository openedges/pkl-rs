//! Resident executable segments with stable, never-reused committed addresses.
use super::{RuntimeError, invalid};
use std::{
    collections::BTreeMap,
    ops::{Index, IndexMut, Range},
    sync::Arc,
};

type Values<'a, T> = std::collections::btree_map::Values<'a, usize, Arc<Vec<T>>>;
type ValuesMut<'a, T> = std::collections::btree_map::ValuesMut<'a, usize, Arc<Vec<T>>>;
pub type Iter<'a, T> = std::iter::Chain<
    std::slice::Iter<'a, T>,
    std::iter::FlatMap<
        Values<'a, T>,
        std::slice::Iter<'a, T>,
        fn(&'a Arc<Vec<T>>) -> std::slice::Iter<'a, T>,
    >,
>;
pub type IterMut<'a, T> = std::iter::Chain<
    std::slice::IterMut<'a, T>,
    std::iter::FlatMap<
        ValuesMut<'a, T>,
        std::slice::IterMut<'a, T>,
        fn(&'a mut Arc<Vec<T>>) -> std::slice::IterMut<'a, T>,
    >,
>;

/// The fixed prefix has direct indexing. Dynamic directories contain only live
/// segments, so a large logical address does not allocate historical slots.
#[derive(Clone, Debug)]
pub struct CodeStorage<T> {
    fixed: Arc<Vec<T>>,
    segments: BTreeMap<usize, Arc<Vec<T>>>,
    next: usize,
    append: Option<usize>,
}

impl<T> From<Vec<T>> for CodeStorage<T> {
    fn from(fixed: Vec<T>) -> Self {
        Self {
            next: fixed.len(),
            fixed: Arc::new(fixed),
            segments: BTreeMap::new(),
            append: None,
        }
    }
}
impl<T> Default for CodeStorage<T> {
    fn default() -> Self {
        Vec::new().into()
    }
}
impl<T> CodeStorage<T> {
    /// Resident element count, independent of the next logical address.
    pub fn len(&self) -> usize {
        self.fixed.len() + self.segments.values().map(|s| s.len()).sum::<usize>()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn next_id(&self) -> usize {
        self.next
    }
    pub fn capacity(&self) -> usize {
        self.fixed.capacity() + self.segments.values().map(|s| s.capacity()).sum::<usize>()
    }
    pub fn resident_segments(&self) -> usize {
        usize::from(!self.fixed.is_empty()) + self.segments.len()
    }
    pub fn get(&self, id: usize) -> Option<&T> {
        if id < self.fixed.len() {
            return self.fixed.get(id);
        }
        let (&start, segment) = self.segments.range(..=id).next_back()?;
        segment.get(id - start)
    }
    pub fn iter(&self) -> Iter<'_, T> {
        self.fixed.iter().chain(
            self.segments
                .values()
                .flat_map((|s| s.iter()) as fn(&Arc<Vec<T>>) -> std::slice::Iter<'_, T>),
        )
    }
    pub fn iter_with_ids(&self) -> impl DoubleEndedIterator<Item = (usize, &T)> {
        self.fixed
            .iter()
            .enumerate()
            .chain(self.segments.iter().flat_map(|(&start, values)| {
                values
                    .iter()
                    .enumerate()
                    .map(move |(offset, value)| (start + offset, value))
            }))
    }
    pub(super) fn iter_from(&self, start: usize) -> impl Iterator<Item = &T> {
        self.iter_with_ids()
            .filter(move |(id, _)| *id >= start)
            .map(|(_, value)| value)
    }
    pub(super) fn check_allocation(&self) -> Result<(), RuntimeError> {
        self.next
            .checked_add(1)
            .ok_or_else(|| invalid("executable address limit"))
            .map(|_| ())
    }
    /// Close the current segment before an independent owner or transaction.
    pub(super) fn seal(&mut self) {
        self.append = None;
    }
    pub(super) fn retire(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        assert!(
            range.start >= self.fixed.len(),
            "fixed prepared code cannot be retired"
        );
        let starts: Vec<_> = self
            .segments
            .range(..range.end)
            .filter_map(|(&start, segment)| {
                if start + segment.len() <= range.start {
                    return None;
                }
                assert!(
                    range.start <= start && start + segment.len() <= range.end,
                    "retirement must respect segment ownership"
                );
                Some(start)
            })
            .collect();
        for start in starts {
            self.segments.remove(&start);
            if self.append == Some(start) {
                self.append = None;
            }
        }
    }
    /// Only uncommitted compiler addresses may be rolled back and reused.
    pub(super) fn rollback(&mut self, next: usize) {
        self.retire(next..self.next);
        self.next = next;
        self.seal();
    }
}
impl<T: Clone> CodeStorage<T> {
    /// Constructor-only finalization keeps prepared workloads on direct
    /// indexing. Dynamic addresses are never compacted after publication.
    pub(super) fn freeze_prepared_prefix(&mut self) {
        let mut values = Arc::unwrap_or_clone(std::mem::take(&mut self.fixed));
        for (start, segment) in std::mem::take(&mut self.segments) {
            assert_eq!(values.len(), start, "prepared prefix must be dense");
            values.extend(Arc::unwrap_or_clone(segment));
        }
        assert_eq!(values.len(), self.next);
        self.fixed = Arc::new(values);
        self.seal();
    }
    pub fn get_mut(&mut self, id: usize) -> Option<&mut T> {
        if id < self.fixed.len() {
            return Arc::make_mut(&mut self.fixed).get_mut(id);
        }
        let (&start, segment) = self.segments.range_mut(..=id).next_back()?;
        if id - start >= segment.len() {
            return None;
        }
        Arc::make_mut(segment).get_mut(id - start)
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        Arc::make_mut(&mut self.fixed)
            .iter_mut()
            .chain(self.segments.values_mut().flat_map(
                (|s| Arc::make_mut(s).iter_mut())
                    as fn(&mut Arc<Vec<T>>) -> std::slice::IterMut<'_, T>,
            ))
    }
    pub(super) fn iter_from_mut(&mut self, start: usize) -> impl Iterator<Item = &mut T> {
        let fixed: &mut [T] = if start < self.fixed.len() {
            &mut Arc::make_mut(&mut self.fixed)[start..]
        } else {
            &mut []
        };
        fixed.iter_mut().chain(
            self.segments
                .iter_mut()
                .filter(move |(base, values)| **base + values.len() > start)
                .flat_map(move |(&base, values)| {
                    Arc::make_mut(values)
                        .iter_mut()
                        .skip(start.saturating_sub(base))
                }),
        )
    }
    pub fn try_push(&mut self, value: T) -> Result<usize, RuntimeError> {
        self.check_allocation()?;
        let id = self.next;
        let start = *self.append.get_or_insert(id);
        Arc::make_mut(self.segments.entry(start).or_default()).push(value);
        self.next += 1;
        Ok(id)
    }
    /// Raw development adapters can still append prepared bytecode. Runtime
    /// compilation uses checked allocation and propagates exhaustion errors.
    pub fn push(&mut self, value: T) {
        self.try_push(value).expect("raw executable address limit");
    }
    pub fn resize_with(&mut self, next: usize, mut create: impl FnMut() -> T) {
        assert!(next >= self.next, "raw metadata resize cannot retire code");
        while self.next < next {
            self.push(create());
        }
    }
}
impl<T> Index<usize> for CodeStorage<T> {
    type Output = T;
    fn index(&self, id: usize) -> &T {
        self.get(id).expect("retired or invalid executable address")
    }
}
impl<T: Clone> IndexMut<usize> for CodeStorage<T> {
    fn index_mut(&mut self, id: usize) -> &mut T {
        self.get_mut(id)
            .expect("retired or invalid executable address")
    }
}
impl<'a, T> IntoIterator for &'a CodeStorage<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T: Clone> IntoIterator for &'a mut CodeStorage<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retiring_non_tail_segments_releases_storage_without_reusing_committed_addresses() {
        let mut store = CodeStorage::from(vec![Arc::new(vec![0_u8])]);
        let payload = Arc::new(vec![7_u8; 4096]);
        let weak = Arc::downgrade(&payload);
        let old = store.try_push(payload).unwrap();
        store.seal();
        let latest = store.try_push(Arc::new(vec![8])).unwrap();
        store.seal();
        let snapshot = store.clone();
        store.retire(old..old + 1);
        assert!(store.get(old).is_none());
        assert!(snapshot.get(old).is_some());
        assert!(weak.upgrade().is_some());
        drop(snapshot);
        assert!(weak.upgrade().is_none());
        let next = store.try_push(Arc::new(vec![9])).unwrap();
        assert!(next > latest && latest > old);
        assert_eq!(store.len(), 3);
        assert_eq!(store.resident_segments(), 3);
        assert!(store.get(old).is_none());
    }

    #[test]
    fn rollback_releases_only_unpublished_segments_and_preserves_cow_snapshots() {
        let mut store = CodeStorage::from(vec![vec![0_u8]]);
        let committed = store.try_push(vec![1]).unwrap();
        store.seal();
        let checkpoint = store.next_id();
        let snapshot = store.clone();
        *store.get_mut(committed).unwrap() = vec![2];
        let speculative = store.try_push(vec![3; 4096]).unwrap();
        store.seal();
        store.try_push(vec![4]).unwrap();
        store.rollback(checkpoint);
        assert_eq!(store.next_id(), checkpoint);
        assert!(store.get(speculative).is_none());
        assert_eq!(store.len(), snapshot.len());
        assert_eq!(snapshot.get(committed).unwrap(), &[1]);
        assert_eq!(store.get(committed).unwrap(), &[2]);
        assert_eq!(store.try_push(vec![5]).unwrap(), speculative);
    }

    #[test]
    fn exhausted_logical_addresses_return_an_error_without_allocation_or_wrap() {
        let mut store = CodeStorage::<u8> {
            next: usize::MAX,
            ..CodeStorage::default()
        };
        assert!(store.try_push(1).is_err());
        assert_eq!(store.next_id(), usize::MAX);
        assert_eq!(store.len(), 0);
        assert_eq!(store.capacity(), 0);
        assert_eq!(store.resident_segments(), 0);
    }

    #[test]
    fn mutable_suffix_and_stale_lookup_do_not_clone_unrelated_shared_segments() {
        let mut store = CodeStorage::from(vec![vec![0_u8; 4096]]);
        let earlier = store.try_push(vec![1]).unwrap();
        store.seal();
        let hole = store.try_push(vec![2]).unwrap();
        store.seal();
        let later = store.try_push(vec![3]).unwrap();
        store.seal();
        let snapshot = store.clone();
        store.retire(hole..hole + 1);
        assert!(store.get_mut(hole).is_none());
        assert!(Arc::ptr_eq(
            &store.segments[&earlier],
            &snapshot.segments[&earlier]
        ));
        for value in store.iter_from_mut(later) {
            value.push(4);
        }
        assert!(Arc::ptr_eq(&store.fixed, &snapshot.fixed));
        assert!(Arc::ptr_eq(
            &store.segments[&earlier],
            &snapshot.segments[&earlier]
        ));
        assert!(!Arc::ptr_eq(
            &store.segments[&later],
            &snapshot.segments[&later]
        ));
        assert_eq!(snapshot[later], [3]);
        assert_eq!(store[later], [3, 4]);
    }
}
