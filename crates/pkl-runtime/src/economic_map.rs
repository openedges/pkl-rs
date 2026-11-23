//! Insertion slots and bucket links for the pinned Graal EconomicMap default strategy.
//!
//! The VM owns keys, values, identity shortcuts, and resumable hash/equality work.
//! This module never evaluates a key. In particular, callers publish the key and
//! value before hashing a newly appended slot, and retain this index if hashing
//! fails. Rebuilding publishes empty buckets before the first hash request; old
//! collision links survive until their own slot is rebuilt, as in the pinned JAR.
//! Removal, compression, and non-default equivalence strategies are not modeled.

use std::ops::Range;

#[derive(Clone, Debug, Default)]
pub struct EconomicMapIndex {
    capacity: usize,
    links: Vec<Option<usize>>,
    buckets: Option<Vec<Option<usize>>>,
    rehash_incomplete: bool,
}

pub(crate) struct HashCandidates<'a> {
    links: &'a [Option<usize>],
    next: Option<usize>,
}

impl Iterator for HashCandidates<'_> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        let index = self.next?;
        self.next = self.links[index];
        Some(index)
    }
}

impl EconomicMapIndex {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            // EconomicMapImpl.init leaves entries unallocated for sizes <= 4.
            capacity: if capacity > 4 { capacity } else { 0 },
            ..Self::default()
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.links.len()
    }

    pub(crate) fn has_hash(&self) -> bool {
        self.buckets.is_some()
    }

    /// Returns the equality evaluation order, without filtering full hashes.
    /// The caller compares query identity first, then query.equals(stored).
    pub(crate) fn candidates(&self, hash: Option<i32>) -> Vec<usize> {
        let Some(buckets) = &self.buckets else {
            return (0..self.len()).collect();
        };
        let mut next = buckets[bucket(hash.expect("hashed lookup needs a hash"), buckets.len())];
        let mut result = Vec::new();
        while let Some(index) = next {
            result.push(index);
            next = self.links[index];
        }
        result
    }

    /// Zero-allocation iterator over collision-chain indices when `has_hash()` is true.
    #[inline]
    pub(crate) fn hash_candidates(&self, hash: i32) -> HashCandidates<'_> {
        let next = self
            .buckets
            .as_ref()
            .and_then(|buckets| buckets[bucket(hash, buckets.len())]);
        HashCandidates {
            links: &self.links,
            next,
        }
    }

    /// True if the next `grow_before_append()` would cross a capacity-width
    /// threshold (256 or 65,536) and require a pre-publish rehash.
    #[inline]
    pub(crate) fn would_rehash_on_grow(&self) -> bool {
        if self.len() < self.capacity || self.capacity == 0 {
            return false;
        }
        let old_capacity = self.capacity;
        let new_capacity = old_capacity + 8.max(old_capacity / 2);
        (old_capacity < 256 && new_capacity >= 256)
            || (old_capacity < 65_536 && new_capacity > 65_536)
    }

    /// Reserves the next insertion slot. A true result requires a complete
    /// rehash before publishing the new key/value or calling `push`.
    pub(crate) fn grow_before_append(&mut self) -> bool {
        if self.len() < self.capacity {
            return false;
        }
        if self.capacity == 0 {
            self.capacity = 4;
            return false;
        }
        let old_capacity = self.capacity;
        self.capacity += 8.max(old_capacity / 2);
        (old_capacity < 256 && self.capacity >= 256)
            || (old_capacity < 65_536 && self.capacity > 65_536)
    }

    /// Publishes a distinct insertion after any capacity-triggered rehash.
    /// Replacements retain the original slot and do not call this method.
    pub(crate) fn push(&mut self) -> usize {
        let index = self.len();
        assert!(index < self.capacity, "reserve before appending");
        self.links.push(None);
        index
    }

    /// Publishes the empty replacement hash array and returns the insertion
    /// order in which the VM must hash keys, stopping immediately on failure.
    pub(crate) fn start_rehash(&mut self) -> Range<usize> {
        let mut count = 4;
        while count <= self.len() {
            count *= 2;
        }
        count *= if self.capacity < 256 { 4 } else { 2 };
        self.buckets = Some(vec![None; count]);
        self.rehash_incomplete = true;
        0..self.len()
    }

    /// Marks the active rehash as complete after every existing slot has been linked.
    #[inline]
    pub(crate) fn finish_rehash(&mut self) {
        self.rehash_incomplete = false;
    }

    /// True if a rehash wiped `buckets` and has not yet linked all existing slots.
    #[inline]
    pub(crate) fn is_rehash_incomplete(&self) -> bool {
        self.rehash_incomplete
    }

    /// Links one successfully hashed slot. A true result requests a full
    /// rehash and makes no changes. This branch occurs only on a collision;
    /// reaching the load threshold alone does not rebuild the table.
    pub(crate) fn put_hash(&mut self, index: usize, hash: i32, allow_rehash: bool) -> bool {
        assert!(index < self.len());
        let buckets = self.buckets.as_mut().expect("hash array is published");
        let slot = bucket(hash, buckets.len());
        let previous = buckets[slot];
        if previous.is_some() && allow_rehash {
            return true;
        }
        assert_ne!(previous, Some(index), "collision link would form a cycle");
        buckets[slot] = Some(index);
        self.links[index] = previous;
        false
    }

    pub(crate) fn should_rehash(&self) -> bool {
        self.buckets
            .as_ref()
            .is_some_and(|buckets| buckets.len() < self.len() + self.len() / 2)
    }

    /// True if appending one more slot cannot trigger a capacity-threshold or
    /// load-factor rehash (`grow_before_append()` and `put_hash(..., should_rehash())`
    /// are both guaranteed false).
    #[inline]
    pub(crate) fn can_fast_append_no_rehash(&self) -> bool {
        if self.rehash_incomplete {
            return false;
        }
        let next_len = self.links.len() + 1;
        if next_len > self.capacity && self.capacity >= 160 {
            return false;
        }
        match &self.buckets {
            None => next_len <= 4,
            Some(buckets) => buckets.len() >= next_len + next_len / 2,
        }
    }
}

fn bucket(hash: i32, count: usize) -> usize {
    let hash = hash as u32;
    ((hash ^ (hash >> 16)) as usize) & (count - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected orders, capacities, and visibility come from the pinned JAR's
    // M2Stage9MapProbe captures in target/m2/stage9-map-java-01, independently
    // recorded before these tests (collisions, failure-2, and growth). Signed
    // query and failed rebuild vectors are in stage9-map-java-02.
    fn append(map: &mut EconomicMapIndex, hashes: &mut Vec<i32>, hash: i32) {
        if map.grow_before_append() {
            rebuild(map, hashes);
        }
        let index = map.push();
        hashes.push(hash);
        if !map.has_hash() {
            if map.len() > 4 {
                rebuild(map, hashes);
            }
        } else if map.put_hash(index, hash, map.should_rehash()) {
            rebuild(map, hashes);
        }
    }

    fn rebuild(map: &mut EconomicMapIndex, hashes: &[i32]) {
        for index in map.start_rehash() {
            assert!(!map.put_hash(index, hashes[index], false));
        }
        map.finish_rehash();
    }

    #[test]
    fn fifth_insertion_switches_from_forward_to_reverse_collision_order() {
        let mut map = EconomicMapIndex::with_capacity(0);
        let mut hashes = Vec::new();
        for hash in [0, 32, 64, 96] {
            append(&mut map, &mut hashes, hash);
        }
        assert!(!map.has_hash());
        assert_eq!(map.candidates(None), [0, 1, 2, 3]);
        append(&mut map, &mut hashes, 128);
        assert_eq!(map.capacity, 12);
        assert_eq!(map.buckets.as_ref().unwrap().len(), 32);
        // These are different full hashes in one bucket. No full-hash filter
        // removes the candidates whose hash differs from the query's 64.
        assert_eq!(map.candidates(Some(64)), [4, 3, 2, 1, 0]);
        assert_eq!(map.candidates(Some(1)), []);
        // Java's unsigned shift places both negative queries in bucket zero.
        assert_eq!(map.candidates(Some(-1)), [4, 3, 2, 1, 0]);
        assert_eq!(map.candidates(Some(i32::MIN)), [4, 3, 2, 1, 0]);
        assert_eq!(map.candidates(Some(0x1234_5678)), []);
    }

    #[test]
    fn failed_fifth_hash_keeps_published_slots_and_hides_unhashed_keys() {
        let mut map = EconomicMapIndex::default();
        for _ in 0..5 {
            assert!(!map.grow_before_append());
            map.push();
        }
        let mut pending = map.start_rehash();
        assert_eq!(pending.next(), Some(0));
        assert!(!map.put_hash(0, 0, false));
        assert_eq!(pending.next(), Some(1));
        assert!(!map.put_hash(1, 1, false));
        assert_eq!(pending.next(), Some(2));
        // The VM fails while hashing slot 2 and drops the continuation.
        assert_eq!(map.len(), 5);
        assert_eq!(map.capacity, 12);
        assert_eq!(map.candidates(Some(0)), [0]);
        assert_eq!(map.candidates(Some(1)), [1]);
        for hash in [2, 3, 4] {
            assert_eq!(map.candidates(Some(hash)), []);
        }
        // Java's retry cannot find hidden k4 and appends a second k4 slot.
        assert!(!map.grow_before_append());
        assert_eq!(map.push(), 5);
        assert!(!map.put_hash(5, 4, map.should_rehash()));
        assert_eq!(map.len(), 6);
        assert_eq!(map.candidates(Some(4)), [5]);
        assert_eq!(map.candidates(Some(2)), []);
    }

    #[test]
    fn load_threshold_rebuilds_only_when_the_new_bucket_is_occupied() {
        let mut map = EconomicMapIndex::default();
        let mut hashes = Vec::new();
        for hash in 0..22 {
            append(&mut map, &mut hashes, hash);
        }
        assert_eq!(map.capacity, 30);
        assert_eq!(map.buckets.as_ref().unwrap().len(), 32);
        assert!(map.should_rehash());
        append(&mut map, &mut hashes, 32);
        assert_eq!(map.buckets.as_ref().unwrap().len(), 128);
        assert_eq!(map.candidates(Some(32)), [22]);
        assert_eq!(map.candidates(Some(0)), [0]);
    }

    #[test]
    fn failed_load_rebuild_keeps_published_new_slot_but_only_exposes_prefix() {
        let mut map = EconomicMapIndex::default();
        let mut hashes = Vec::new();
        for _ in 0..21 {
            append(&mut map, &mut hashes, 0);
        }
        assert!(!map.grow_before_append());
        assert_eq!(map.push(), 21);
        assert!(map.put_hash(21, 0, map.should_rehash()));
        assert_eq!(map.candidates(Some(0)), (0..21).rev().collect::<Vec<_>>());
        assert_eq!(map.start_rehash(), 0..22);
        assert_eq!(map.candidates(Some(0)), []);
        assert!(!map.put_hash(0, 0, false));
        assert!(!map.put_hash(1, 0, false));
        // k2's hash fails, retaining every cursor slot and only the prefix's
        // bucket links. Subsequent lookup cannot fall back to a linear scan.
        assert_eq!(map.len(), 22);
        assert_eq!(map.buckets.as_ref().unwrap().len(), 128);
        assert_eq!(map.candidates(Some(0)), [1, 0]);
    }

    #[test]
    fn index_width_growth_rehashes_existing_slots_before_the_new_slot() {
        let mut map = EconomicMapIndex::default();
        let mut hashes = Vec::new();
        for hash in 0..225 {
            append(&mut map, &mut hashes, hash);
        }
        assert_eq!(map.capacity, 225);
        assert!(map.would_rehash_on_grow());
        assert!(!map.can_fast_append_no_rehash());
        assert!(map.grow_before_append());
        assert_eq!(map.capacity, 337);
        assert!(!map.would_rehash_on_grow());
        // The capacity has grown, but the new insertion is still unpublished.
        assert_eq!(map.len(), 225);
        rebuild(&mut map, &hashes);
        assert!(!map.is_rehash_incomplete());
        assert_eq!(map.buckets.as_ref().unwrap().len(), 512);
        assert_eq!(map.candidates(Some(225)), []);
        assert_eq!(map.push(), 225);
        assert!(!map.put_hash(225, 225, map.should_rehash()));
        assert_eq!(map.candidates(Some(225)), [225]);
    }
}
