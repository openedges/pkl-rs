//! Paguro 3.10.3 hash-trie routing for strict Map entries.
//!
//! Values stay in the traced heap. This index retains only insertion-slot IDs.
//! A bitmap leaf tests equality after the consumed hash prefix, whereas a
//! collision node scans its insertion-ordered entries. Neither is a full-hash
//! filter. The mutable builder expands a bitmap at its seventeenth occupied slot.
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
enum Branch {
    Leaf(usize),
    Node(Box<Node>),
}
#[derive(Clone, Debug, Default)]
enum Node {
    #[default]
    Empty,
    Bitmap {
        array: bool,
        slots: BTreeMap<u32, Branch>,
    },
    Collision {
        hash: i32,
        indices: Vec<usize>,
    },
}
fn mask(hash: i32, shift: u32) -> u32 {
    (hash as u32 >> shift) & 31
}
impl Node {
    fn candidates(&self, hash: i32, shift: u32) -> Vec<usize> {
        match self {
            Self::Empty => Vec::new(),
            Self::Collision { indices, .. } => indices.clone(),
            Self::Bitmap { slots, .. } => match slots.get(&mask(hash, shift)) {
                None => Vec::new(),
                Some(Branch::Leaf(index)) => vec![*index],
                Some(Branch::Node(node)) => node.candidates(hash, shift + 5),
            },
        }
    }
    fn assoc_candidates(&self, hash: i32, shift: u32) -> Vec<usize> {
        match self {
            Self::Collision { hash: stored, .. } if *stored != hash => Vec::new(),
            Self::Bitmap { slots, .. } => match slots.get(&mask(hash, shift)) {
                Some(Branch::Node(node)) => node.assoc_candidates(hash, shift + 5),
                _ => self.candidates(hash, shift),
            },
            _ => self.candidates(hash, shift),
        }
    }
    fn rehash(&self, hash: i32, shift: u32) -> Vec<usize> {
        match self {
            Self::Empty | Self::Collision { .. } => Vec::new(),
            Self::Bitmap { array, slots } => match slots.get(&mask(hash, shift)) {
                Some(Branch::Leaf(index)) => vec![*index],
                Some(Branch::Node(node)) => node.rehash(hash, shift + 5),
                None if !array && slots.len() >= 16 => slots
                    .values()
                    .filter_map(|branch| {
                        if let Branch::Leaf(index) = branch {
                            Some(*index)
                        } else {
                            None
                        }
                    })
                    .collect(),
                None => Vec::new(),
            },
        }
    }
    fn insert(&mut self, index: usize, hash: i32, hashes: &[i32], shift: u32) {
        if matches!(self, Self::Empty) {
            *self = Self::Bitmap {
                array: false,
                slots: BTreeMap::new(),
            };
        }
        if let Self::Collision {
            hash: old_hash,
            indices,
        } = self
        {
            if *old_hash == hash {
                indices.push(index);
                return;
            }
            let slot = mask(*old_hash, shift);
            let old = std::mem::take(self);
            *self = Self::Bitmap {
                array: false,
                slots: BTreeMap::from([(slot, Branch::Node(Box::new(old)))]),
            };
        }
        let Self::Bitmap { array, slots } = self else {
            unreachable!()
        };
        let slot = mask(hash, shift);
        if let Some(branch) = slots.get_mut(&slot) {
            match branch {
                Branch::Leaf(old) => {
                    let old = *old;
                    let node = if hashes[old] == hash {
                        Node::Collision {
                            hash,
                            indices: vec![old, index],
                        }
                    } else {
                        let mut node = Node::Empty;
                        node.insert(old, hashes[old], hashes, shift + 5);
                        node.insert(index, hash, hashes, shift + 5);
                        node
                    };
                    *branch = Branch::Node(Box::new(node));
                }
                Branch::Node(node) => node.insert(index, hash, hashes, shift + 5),
            }
        } else {
            if !*array && slots.len() >= 16 {
                for branch in slots.values_mut() {
                    if let Branch::Leaf(old) = branch {
                        let mut node = Node::Empty;
                        node.insert(*old, hashes[*old], hashes, shift + 5);
                        *branch = Branch::Node(Box::new(node));
                    }
                }
                *array = true;
            }
            let branch = if *array {
                let mut node = Node::Empty;
                node.insert(index, hash, hashes, shift + 5);
                Branch::Node(Box::new(node))
            } else {
                Branch::Leaf(index)
            };
            slots.insert(slot, branch);
        }
    }
    fn ordered(&self, result: &mut Vec<usize>) {
        match self {
            Self::Empty => (),
            Self::Collision { indices, .. } => result.extend(indices),
            Self::Bitmap { slots, .. } => {
                for branch in slots.values() {
                    match branch {
                        Branch::Leaf(index) => result.push(*index),
                        Branch::Node(node) => node.ordered(result),
                    }
                }
            }
        }
    }

    fn collision_lookup(&self, hash: i32, shift: u32) -> bool {
        match self {
            Self::Collision { .. } => true,
            Self::Bitmap { slots, .. } => match slots.get(&mask(hash, shift)) {
                Some(Branch::Node(node)) => node.collision_lookup(hash, shift + 5),
                _ => false,
            },
            Self::Empty => false,
        }
    }

    // Called only after the VM has repeated both the hash and the equality
    // lookup for PersistentHashSet.without. Preserve the original trie shape:
    // direct bitmap removal retains an empty bitmap, while a vanished child
    // can collapse its parent or pack an ArrayNode at old count <= 8.
    fn remove_matched(&mut self, hash: i32, shift: u32, removed: usize) -> bool {
        match self {
            Self::Empty => false,
            Self::Collision {
                hash: stored,
                indices,
            } => {
                let Some(position) = indices.iter().position(|index| *index == removed) else {
                    return false;
                };
                indices.remove(position);
                // The persistent Paguro overload uses the probe's hash for
                // the replacement collision node, even if its hash changed.
                *stored = hash;
                if indices.is_empty() {
                    *self = Self::Empty;
                }
                true
            }
            Self::Bitmap { array, slots } => {
                let slot = mask(hash, shift);
                let Some(branch) = slots.get_mut(&slot) else {
                    return false;
                };
                let (found, remove_slot, direct_leaf) = match branch {
                    Branch::Leaf(index) => (*index == removed, *index == removed, true),
                    Branch::Node(node) => {
                        let found = node.remove_matched(hash, shift + 5, removed);
                        (found, matches!(node.as_ref(), Self::Empty), false)
                    }
                };
                if !found {
                    return false;
                }
                if remove_slot {
                    let old_count = slots.len();
                    slots.remove(&slot);
                    if *array {
                        if old_count <= 8 {
                            *array = false;
                        }
                    } else if !direct_leaf && slots.is_empty() {
                        *self = Self::Empty;
                    }
                }
                true
            }
        }
    }

    fn shift_after_removed(&mut self, removed: usize) {
        match self {
            Self::Empty => (),
            Self::Collision { indices, .. } => {
                for index in indices {
                    if *index > removed {
                        *index -= 1;
                    }
                }
            }
            Self::Bitmap { slots, .. } => {
                for branch in slots.values_mut() {
                    match branch {
                        Branch::Leaf(index) => {
                            if *index > removed {
                                *index -= 1;
                            }
                        }
                        Branch::Node(node) => node.shift_after_removed(removed),
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MapIndex {
    root: Node,
    length: usize,
    insertion_order: Vec<usize>,
    identity_dependent: bool,
}
pub(crate) static EMPTY: MapIndex = MapIndex {
    root: Node::Empty,
    length: 0,
    insertion_order: Vec::new(),
    identity_dependent: false,
};
impl MapIndex {
    pub(crate) fn len(&self) -> usize {
        self.length
    }
    pub(crate) fn insertion_order(&self) -> &[usize] {
        &self.insertion_order
    }
    pub(crate) fn assoc_candidates(&self, hash: Option<i32>) -> Vec<usize> {
        match hash {
            Some(hash) if !self.identity_dependent => self.root.assoc_candidates(hash, 0),
            _ => (0..self.length).collect(),
        }
    }

    pub(crate) fn candidates(&self, hash: Option<i32>) -> Vec<usize> {
        match hash {
            Some(hash) if !self.identity_dependent => self.root.candidates(hash, 0),
            _ => (0..self.length).collect(),
        }
    }
    pub(crate) fn rehash(&self, hash: Option<i32>) -> Vec<usize> {
        match hash {
            Some(hash) if !self.identity_dependent => self.root.rehash(hash, 0),
            _ => Vec::new(),
        }
    }
    pub(crate) fn insert(&mut self, hashes: Vec<Option<i32>>, append_order: bool) {
        let index = self.length;
        self.length += 1;
        if append_order {
            self.insertion_order.push(index);
        }
        if let Some(hashes) = hashes.into_iter().collect::<Option<Vec<_>>>() {
            if !self.identity_dependent {
                self.root.insert(index, hashes[index], &hashes, 0);
            }
        } else {
            self.identity_dependent = true;
        }
    }
    pub(crate) fn ordered(&self) -> Vec<usize> {
        if self.identity_dependent {
            return (0..self.length).collect();
        }
        let mut result = Vec::with_capacity(self.length);
        self.root.ordered(&mut result);
        result
    }

    /// HashCollisionNode.find repeats equality on its first matching entry.
    pub(crate) fn repeats_lookup_equality(&self, hash: Option<i32>) -> bool {
        hash.is_some_and(|hash| !self.identity_dependent && self.root.collision_lookup(hash, 0))
    }

    pub(crate) fn without_matched(&self, hash: Option<i32>, removed: usize) -> Self {
        assert!(removed < self.length, "stored Set slot");
        let mut result = self.clone();
        result.length -= 1;
        // A recursively partial key may have had a known zero hash when it
        // was inserted, then expose identity-dependent fields on recomputation.
        // Lookup already uses the conservative candidate policy for None; keep
        // the resulting index in that same explicit model after removal.
        if hash.is_none() {
            result.identity_dependent = true;
        }
        // The identity-dependent model has no complete portable hash trie.
        if !result.identity_dependent {
            assert!(
                result
                    .root
                    .remove_matched(hash.expect("routed Set hash"), 0, removed),
                "matched Set key"
            );
            result.root.shift_after_removed(removed);
        }
        result.insertion_order.retain(|index| *index != removed);
        for index in &mut result.insertion_order {
            if *index > removed {
                *index -= 1;
            }
        }
        result
    }
}
