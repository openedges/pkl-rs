//! Compact columnar/shape-interned value encoder for zero-reparse JS/Wasm/N-API boundary transfer.
//!
//! Traverses ONLY already-forced, acyclic VM value graphs under the exact same soundness
//! conditions as `try_fast_encode_pkl_binary` (zero lazy forcing, zero side effects, zero VM
//! mutations). On any unforced member, cycle, unpaired UTF-16 surrogate, annotation, or edge
//! type, packing returns `false` so callers seamlessly fall back to the full resumable
//! `pkl-binary` `RenderRun` with 100% identical error ordering and diagnostics.

use super::*;
use crate::heap::CollectionMemberKind;

pub const COLUMNAR_MAGIC: u32 = 0x504B_4C01;
const HEADER_U32S: usize = 20;

const OP_NULL: i32 = 0;
const OP_INT: i32 = 1;
const OP_FLOAT: i32 = 2;
const OP_BOOL_FALSE: i32 = 3;
const OP_BOOL_TRUE: i32 = 4;
const OP_STR: i32 = 5;
const OP_BYTES: i32 = 6;
const OP_LIST: i32 = 7;
const OP_SHAPED_OBJ: i32 = 10;
const OP_COLUMNAR_LIST: i32 = 12;
const OP_DURATION: i32 = 13;
const OP_DATASIZE: i32 = 14;
const OP_PAIR: i32 = 15;
const OP_REGEX: i32 = 16;
const OP_INTSEQ: i32 = 17;
const OP_FUNCTION: i32 = 18;

const COL_INT: i32 = 1;
const COL_BOOL: i32 = 2;
const COL_STR: i32 = 3;
const COL_BYTES: i32 = 4;
const COL_FLOAT: i32 = 5;
const COL_NULL: i32 = 6;

const MAX_RETAINED_BYTES: usize = 256 * 1024;

#[inline]
fn utf16_len_of_valid_utf8(s: &str) -> u32 {
    if s.is_ascii() {
        return s.len() as u32;
    }
    let mut units = 0u32;
    for &b in s.as_bytes() {
        if b < 0x80 || (0xC0..0xF0).contains(&b) {
            units += 1;
        } else if b >= 0xF0 {
            units += 2;
        }
    }
    units
}

#[inline]
fn is_valid_binary_ident(s: &str) -> bool {
    !s.is_empty() && !s.bytes().all(|b| b <= 0x20 || b.is_ascii_whitespace())
}

#[inline]
fn align8(n: usize) -> Option<usize> {
    n.checked_add(7).map(|v| v & !7)
}

const MAX_COLUMNAR_NODES: usize = 2_000_000;
const MAX_COLUMNAR_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

struct ShapeKey {
    class_name: String,
    module_uri: String,
    props: Vec<String>,
}

/// Reusable scratch + final aligned wire buffer for columnar value transfer.
/// Holds zero VM heap references so GC and session cleanup remain completely unaffected.
#[derive(Default)]
pub struct ColumnarValueBuffer {
    shape_ops: Vec<i32>,
    shape_count: u32,
    shape_keys: Vec<ShapeKey>,
    shape_str_list: Vec<String>,
    shape_str_lens: Vec<u32>,
    shape_str_utf8: Vec<u8>,

    ops: Vec<i32>,
    bigs: Vec<i64>,
    floats: Vec<f64>,
    bools: Vec<u8>,
    val_str_lens: Vec<u32>,
    val_str_utf8: Vec<u8>,
    bin_lens: Vec<u32>,
    bin_bytes: Vec<u8>,

    packed: Vec<u8>,
    start_offset: usize,
    cached_key: Option<(u64, ObjectId, u8)>,
    pack_generation: u32,
}

impl ColumnarValueBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        if self.start_offset <= self.packed.len() {
            &self.packed[self.start_offset..]
        } else {
            &[]
        }
    }

    #[inline]
    pub fn as_ptr_usize(&self) -> usize {
        self.as_bytes().as_ptr() as usize
    }

    #[inline]
    pub fn byte_len(&self) -> usize {
        self.as_bytes().len()
    }

    fn reset_streams(&mut self) {
        self.cached_key = None;
        self.shape_ops.clear();
        self.shape_count = 0;
        self.shape_keys.clear();
        self.shape_str_list.clear();
        self.shape_str_lens.clear();
        self.shape_str_utf8.clear();

        self.ops.clear();
        self.bigs.clear();
        self.floats.clear();
        self.bools.clear();
        self.val_str_lens.clear();
        self.val_str_utf8.clear();
        self.bin_lens.clear();
        self.bin_bytes.clear();
    }

    /// Shrinks oversized internal buffers after a large one-off payload so long-running
    /// soak sessions stay strictly bounded.
    pub fn shrink_if_oversized(&mut self) {
        if self.packed.capacity() > MAX_RETAINED_BYTES {
            self.packed = Vec::new();
            self.start_offset = 0;
            self.cached_key = None;
        }
        if self.val_str_utf8.capacity() > MAX_RETAINED_BYTES {
            self.val_str_utf8 = Vec::new();
        }
        if self.bin_bytes.capacity() > MAX_RETAINED_BYTES {
            self.bin_bytes = Vec::new();
        }
        if self.bigs.capacity() * 8 > MAX_RETAINED_BYTES {
            self.bigs = Vec::new();
        }
        if self.ops.capacity() * 4 > MAX_RETAINED_BYTES {
            self.ops = Vec::new();
        }
    }

    /// Releases all owned buffers immediately (called on evaluator `close()`).
    pub fn clear_and_release(&mut self) {
        *self = Self::default();
    }

    fn intern_shape_str(&mut self, s: &str) -> Option<i32> {
        if s.len() > u32::MAX as usize {
            return None;
        }
        if let Some(pos) = self.shape_str_list.iter().position(|prev| prev == s) {
            return i32::try_from(pos).ok();
        }
        let idx = i32::try_from(self.shape_str_list.len()).ok()?;
        self.shape_str_list.push(s.to_owned());
        self.shape_str_lens.push(utf16_len_of_valid_utf8(s));
        self.shape_str_utf8.extend_from_slice(s.as_bytes());
        Some(idx)
    }

    fn intern_shape(&mut self, class_name: &str, module_uri: &str, props: &[&str]) -> Option<i32> {
        if let Some(pos) = self.shape_keys.iter().position(|sk| {
            sk.class_name == class_name
                && sk.module_uri == module_uri
                && sk.props.len() == props.len()
                && sk.props.iter().zip(props).all(|(a, b)| a == b)
        }) {
            return i32::try_from(pos).ok();
        }
        let shape_id = i32::try_from(self.shape_keys.len()).ok()?;
        let cls_idx = self.intern_shape_str(class_name)?;
        let uri_idx = self.intern_shape_str(module_uri)?;
        let prop_count = i32::try_from(props.len()).ok()?;
        let mut prop_idxs = Vec::with_capacity(props.len());
        for &p in props {
            prop_idxs.push(self.intern_shape_str(p)?);
        }
        self.shape_ops.push(cls_idx);
        self.shape_ops.push(uri_idx);
        self.shape_ops.push(prop_count);
        self.shape_ops.extend_from_slice(&prop_idxs);
        self.shape_keys.push(ShapeKey {
            class_name: class_name.to_owned(),
            module_uri: module_uri.to_owned(),
            props: props.iter().map(|&p| p.to_owned()).collect(),
        });
        self.shape_count = self.shape_count.checked_add(1)?;
        Some(shape_id)
    }

    #[inline]
    fn push_val_str(&mut self, s: &str) -> bool {
        if s.len() > u32::MAX as usize {
            return false;
        }
        self.val_str_lens.push(utf16_len_of_valid_utf8(s));
        self.val_str_utf8.extend_from_slice(s.as_bytes());
        true
    }

    #[inline]
    fn push_bytes(&mut self, b: &[u8]) -> bool {
        let Ok(len) = u32::try_from(b.len()) else {
            return false;
        };
        self.bin_lens.push(len);
        self.bin_bytes.extend_from_slice(b);
        true
    }

    fn finish(&mut self) -> bool {
        let ops_total = match self.shape_ops.len().checked_add(self.ops.len()) {
            Some(v) => v,
            None => return false,
        };
        let str_lens_total = match self
            .shape_str_lens
            .len()
            .checked_add(self.val_str_lens.len())
        {
            Some(v) => v,
            None => return false,
        };
        let str_utf8_total = match self
            .shape_str_utf8
            .len()
            .checked_add(self.val_str_utf8.len())
        {
            Some(v) => v,
            None => return false,
        };

        let header_bytes = HEADER_U32S * 4;
        let ops_off = header_bytes;
        let Some(ops_bytes) = ops_total.checked_mul(4) else {
            return false;
        };
        let Some(bigs_off) = ops_off.checked_add(ops_bytes).and_then(align8) else {
            return false;
        };
        let Some(bigs_bytes) = self.bigs.len().checked_mul(8) else {
            return false;
        };
        let Some(floats_off) = bigs_off.checked_add(bigs_bytes).and_then(align8) else {
            return false;
        };
        let Some(floats_bytes) = self.floats.len().checked_mul(8) else {
            return false;
        };
        let Some(str_lens_off) = floats_off.checked_add(floats_bytes) else {
            return false;
        };
        let Some(str_lens_bytes) = str_lens_total.checked_mul(4) else {
            return false;
        };
        let Some(bin_lens_off) = str_lens_off.checked_add(str_lens_bytes) else {
            return false;
        };
        let Some(bin_lens_bytes) = self.bin_lens.len().checked_mul(4) else {
            return false;
        };
        let Some(bools_off) = bin_lens_off.checked_add(bin_lens_bytes) else {
            return false;
        };
        let Some(str_utf8_off) = bools_off.checked_add(self.bools.len()) else {
            return false;
        };
        let Some(bin_bytes_off) = str_utf8_off.checked_add(str_utf8_total) else {
            return false;
        };
        let Some(total_bytes) = bin_bytes_off.checked_add(self.bin_bytes.len()) else {
            return false;
        };
        if u32::try_from(total_bytes).is_err() {
            return false;
        }
        let Some(reserve_bytes) = total_bytes.checked_add(8) else {
            return false;
        };

        self.packed.clear();
        if self.packed.capacity() < reserve_bytes && self.packed.try_reserve(reserve_bytes).is_err()
        {
            return false;
        }
        let base_ptr = self.packed.as_ptr() as usize;
        let pad = (8 - (base_ptr & 7)) & 7;
        self.start_offset = pad;
        self.packed.resize(pad, 0);

        self.pack_generation = self.pack_generation.wrapping_add(1);
        if self.pack_generation == 0 {
            self.pack_generation = 1;
        }
        let header: [u32; HEADER_U32S] = [
            COLUMNAR_MAGIC,
            ops_off as u32,
            ops_total as u32,
            bigs_off as u32,
            self.bigs.len() as u32,
            floats_off as u32,
            self.floats.len() as u32,
            bools_off as u32,
            self.bools.len() as u32,
            str_lens_off as u32,
            str_lens_total as u32,
            str_utf8_off as u32,
            str_utf8_total as u32,
            bin_lens_off as u32,
            self.bin_lens.len() as u32,
            bin_bytes_off as u32,
            self.bin_bytes.len() as u32,
            self.shape_count,
            self.shape_str_lens.len() as u32,
            self.pack_generation,
        ];
        for word in header {
            self.packed.extend_from_slice(&word.to_le_bytes());
        }
        for &op in &self.shape_ops {
            self.packed.extend_from_slice(&op.to_le_bytes());
        }
        for &op in &self.ops {
            self.packed.extend_from_slice(&op.to_le_bytes());
        }
        let target_bigs = pad + bigs_off;
        if self.packed.len() < target_bigs {
            self.packed.resize(target_bigs, 0);
        }
        for &v in &self.bigs {
            self.packed.extend_from_slice(&v.to_le_bytes());
        }
        for &v in &self.floats {
            self.packed.extend_from_slice(&v.to_le_bytes());
        }
        for &l in &self.shape_str_lens {
            self.packed.extend_from_slice(&l.to_le_bytes());
        }
        for &l in &self.val_str_lens {
            self.packed.extend_from_slice(&l.to_le_bytes());
        }
        for &l in &self.bin_lens {
            self.packed.extend_from_slice(&l.to_le_bytes());
        }
        self.packed.extend_from_slice(&self.bools);
        self.packed.extend_from_slice(&self.shape_str_utf8);
        self.packed.extend_from_slice(&self.val_str_utf8);
        self.packed.extend_from_slice(&self.bin_bytes);

        debug_assert_eq!(self.packed.len(), pad + total_bytes);
        debug_assert_eq!((self.as_bytes().as_ptr() as usize) & 7, 0);
        true
    }
}

struct Checkpoint {
    shape_ops: usize,
    shape_count: u32,
    shape_keys: usize,
    shape_str_list: usize,
    shape_str_lens: usize,
    shape_str_utf8: usize,
    ops: usize,
    bigs: usize,
    floats: usize,
    bools: usize,
    val_str_lens: usize,
    val_str_utf8: usize,
    bin_lens: usize,
    bin_bytes: usize,
}

impl ColumnarValueBuffer {
    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            shape_ops: self.shape_ops.len(),
            shape_count: self.shape_count,
            shape_keys: self.shape_keys.len(),
            shape_str_list: self.shape_str_list.len(),
            shape_str_lens: self.shape_str_lens.len(),
            shape_str_utf8: self.shape_str_utf8.len(),
            ops: self.ops.len(),
            bigs: self.bigs.len(),
            floats: self.floats.len(),
            bools: self.bools.len(),
            val_str_lens: self.val_str_lens.len(),
            val_str_utf8: self.val_str_utf8.len(),
            bin_lens: self.bin_lens.len(),
            bin_bytes: self.bin_bytes.len(),
        }
    }

    fn restore(&mut self, cp: Checkpoint) {
        self.shape_ops.truncate(cp.shape_ops);
        self.shape_count = cp.shape_count;
        self.shape_keys.truncate(cp.shape_keys);
        self.shape_str_list.truncate(cp.shape_str_list);
        self.shape_str_lens.truncate(cp.shape_str_lens);
        self.shape_str_utf8.truncate(cp.shape_str_utf8);
        self.ops.truncate(cp.ops);
        self.bigs.truncate(cp.bigs);
        self.floats.truncate(cp.floats);
        self.bools.truncate(cp.bools);
        self.val_str_lens.truncate(cp.val_str_lens);
        self.val_str_utf8.truncate(cp.val_str_utf8);
        self.bin_lens.truncate(cp.bin_lens);
        self.bin_bytes.truncate(cp.bin_bytes);
    }
}

struct PackCtx<'a> {
    vm: &'a Vm,
    dynamic_class: Option<ObjectId>,
    empty_dynamic_parent_cache: Option<ObjectId>,
    active: Vec<ObjectId>,
    remaining_nodes: usize,
}

impl<'a> PackCtx<'a> {
    fn is_empty_dynamic_parent(&mut self, parent: Option<ObjectId>) -> bool {
        let Some(p0) = parent else {
            return true;
        };
        if self.empty_dynamic_parent_cache == Some(p0) {
            return true;
        }
        let mut cur = Some(p0);
        let mut hops = 0u8;
        while let Some(p) = cur {
            hops += 1;
            if hops > 4 {
                return false;
            }
            let Some(pobj) = self.vm.heap.get_fast(p) else {
                return false;
            };
            if pobj.collection_kind != CollectionKind::Dynamic
                || pobj.is_module
                || !pobj.collection_members.is_empty()
            {
                return false;
            }
            if pobj
                .builtin_type
                .as_deref()
                .and_then(classes::BuiltinClass::from_type)
                == Some(classes::BuiltinClass::Dynamic)
            {
                self.empty_dynamic_parent_cache = Some(p0);
                return true;
            }
            if pobj.instance_class.is_some() || !pobj.members.is_empty() {
                return false;
            }
            cur = pobj.parent;
        }
        self.empty_dynamic_parent_cache = Some(p0);
        true
    }

    #[inline]
    fn is_plain_dynamic(&mut self, obj: &Object) -> bool {
        if obj.is_module
            || obj.collection_kind != CollectionKind::Dynamic
            || obj.pair.is_some()
            || obj.reference.is_some()
            || obj.foreign_map_entry.is_some()
            || obj.function.is_some()
            || obj.native_function.is_some()
            || !obj.member_annotations.is_empty()
        {
            return false;
        }
        let instance_ok = match obj.instance_class {
            None => true,
            Some(cls) if Some(cls) == self.dynamic_class => true,
            Some(cls) => {
                self.vm
                    .heap
                    .get_class(cls)
                    .ok()
                    .and_then(|c| self.vm.heap.get_fast(c.prototype))
                    .and_then(|p| p.builtin_type.as_deref())
                    .and_then(classes::BuiltinClass::from_type)
                    == Some(classes::BuiltinClass::Dynamic)
            }
        };
        if !instance_ok || !self.is_empty_dynamic_parent(obj.parent) {
            return false;
        }
        if obj.collection_members.iter().any(|cm| {
            cm.kind != CollectionMemberKind::Property || cm.name.as_str() == Some("default")
        }) {
            return false;
        }
        !obj.members.iter().any(|m| {
            m.entry
                || m.hidden()
                || (!m.local
                    && !m.method
                    && !m.definition()
                    && !m.import()
                    && !m.external()
                    && m.name.as_str() == Some("default"))
        })
    }

    fn collect_plain_dynamic_props<'b>(
        &self,
        obj: &'b Object,
        props: &mut Vec<(&'b PklString, &'b Value)>,
    ) -> bool {
        let base = props.len();
        if !obj.collection_members.is_empty() {
            let slots_aligned = obj.cache_slots.len() == obj.collection_members.len();
            for (idx, cm) in obj.collection_members.iter().enumerate() {
                if props[base..].iter().any(|(prev, _)| *prev == &cm.name) {
                    props.truncate(base);
                    return false;
                }
                let val = if slots_aligned
                    && !obj.cache_slots[idx].key.entry
                    && !obj.cache_slots[idx].key.method
                    && obj.cache_slots[idx].key.owner.is_none()
                    && obj.cache_slots[idx].key.name == cm.name
                {
                    &obj.cache_slots[idx].value
                } else if let Some(val) = super::fast_lookup_cached(obj, None, &cm.name, false) {
                    val
                } else {
                    props.truncate(base);
                    return false;
                };
                props.push((&cm.name, val));
            }
        } else {
            let slots_aligned = obj.cache_slots.len() == obj.members.len();
            for (idx, m) in obj.members.iter().enumerate() {
                if m.local
                    || m.method
                    || m.entry
                    || m.definition()
                    || m.import()
                    || m.external()
                    || m.hidden()
                {
                    continue;
                }
                if props[base..].iter().any(|(prev, _)| *prev == &m.name) {
                    props.truncate(base);
                    return false;
                }
                let val = if slots_aligned
                    && !obj.cache_slots[idx].key.entry
                    && !obj.cache_slots[idx].key.method
                    && obj.cache_slots[idx].key.owner.is_none()
                    && obj.cache_slots[idx].key.name == m.name
                {
                    &obj.cache_slots[idx].value
                } else if let Some(val) = super::fast_lookup_cached(obj, None, &m.name, false) {
                    val
                } else {
                    props.truncate(base);
                    return false;
                };
                props.push((&m.name, val));
            }
        }
        let count = props.len() - base;
        if let Some(expected) = obj.dynamic_regular_member_count
            && expected != count
        {
            props.truncate(base);
            return false;
        }
        true
    }

    #[inline]
    fn primitive_col_type(val: &Value) -> Option<i32> {
        match val {
            Value::Int(_) => Some(COL_INT),
            Value::Bool(_) => Some(COL_BOOL),
            Value::String(s) if s.as_str().is_some() => Some(COL_STR),
            Value::Bytes(_) => Some(COL_BYTES),
            Value::Float(_) => Some(COL_FLOAT),
            Value::Null | Value::NullWithDefault(_) => Some(COL_NULL),
            _ => None,
        }
    }

    fn try_pack_columnar_dynamic_list(
        &mut self,
        elems: &[Value],
        out: &mut ColumnarValueBuffer,
    ) -> bool {
        if elems.len() < 4 || elems.len() > i32::MAX as usize {
            return false;
        }
        let Value::Object(first_id) = elems[0] else {
            return false;
        };
        if self.active.contains(&first_id) {
            return false;
        }
        let Some(first_obj) = self.vm.heap.get_fast(first_id) else {
            return false;
        };
        if !self.is_plain_dynamic(first_obj) {
            return false;
        }
        let mut first_props: Vec<(&PklString, &Value)> = Vec::with_capacity(8);
        if !self.collect_plain_dynamic_props(first_obj, &mut first_props) {
            return false;
        }
        let k = first_props.len();
        if k == 0 || k > 32 {
            return false;
        }
        let mut col_types = [0i32; 32];
        let mut prop_strs = [""; 32];
        for (c, (name, val)) in first_props.iter().enumerate() {
            let Some(ns) = name.as_str() else {
                return false;
            };
            let Some(ct) = Self::primitive_col_type(val) else {
                return false;
            };
            prop_strs[c] = ns;
            col_types[c] = ct;
        }

        let cp = out.checkpoint();
        let Some(shape_id) = out.intern_shape("Dynamic", "pkl:base", &prop_strs[..k]) else {
            out.restore(cp);
            return false;
        };
        out.ops.push(OP_COLUMNAR_LIST);
        out.ops.push(shape_id);
        out.ops.push(elems.len() as i32);
        out.ops.extend_from_slice(&col_types[..k]);

        // Pre-reserve column buffers for zero reallocations in the hot row loop.
        let mut n_int = 0usize;
        let mut n_bool = 0usize;
        let mut n_str = 0usize;
        let mut n_bytes = 0usize;
        let mut n_float = 0usize;
        for &ct in &col_types[..k] {
            match ct {
                COL_INT => n_int += 1,
                COL_BOOL => n_bool += 1,
                COL_STR => n_str += 1,
                COL_BYTES => n_bytes += 1,
                COL_FLOAT => n_float += 1,
                _ => {}
            }
        }
        let count = elems.len();
        let Some(row_budget) = count.checked_mul(k.saturating_add(1)) else {
            out.restore(cp);
            return false;
        };
        if row_budget > self.remaining_nodes {
            out.restore(cp);
            return false;
        }
        self.remaining_nodes -= row_budget;
        let (Some(int_cap), Some(bool_cap), Some(str_cap), Some(bytes_cap), Some(float_cap)) = (
            count.checked_mul(n_int),
            count.checked_mul(n_bool),
            count.checked_mul(n_str),
            count.checked_mul(n_bytes),
            count.checked_mul(n_float),
        ) else {
            out.restore(cp);
            return false;
        };
        if out.bigs.try_reserve(int_cap).is_err()
            || out.bools.try_reserve(bool_cap).is_err()
            || out.val_str_lens.try_reserve(str_cap).is_err()
            || out.bin_lens.try_reserve(bytes_cap).is_err()
            || out.floats.try_reserve(float_cap).is_err()
        {
            out.restore(cp);
            return false;
        }

        let use_collection_members = !first_obj.collection_members.is_empty();
        // Record exact cache_slots permutation from element 0 so property forcing in any order
        // (e.g., `run(0)` forcing `raw/label/active` before `id/negId`) still hits O(1) indexing!
        let mut slot_perm = [usize::MAX; 32];
        let mut perm_valid = use_collection_members
            && first_obj.collection_members.len() == k
            && first_obj.cache_slots.len() == k;
        if perm_valid {
            for c in 0..k {
                let target_name = first_props[c].0;
                if let Some(sidx) = first_obj.cache_slots.iter().position(|slot| {
                    !slot.key.entry
                        && !slot.key.method
                        && slot.key.owner.is_none()
                        && slot.key.name == *target_name
                }) {
                    slot_perm[c] = sidx;
                } else {
                    perm_valid = false;
                    break;
                }
            }
        }
        let mut row_props: Vec<(&PklString, &Value)> = Vec::with_capacity(k);

        for elem in elems {
            let Value::Object(eid) = *elem else {
                out.restore(cp);
                return false;
            };
            if self.active.contains(&eid) {
                out.restore(cp);
                return false;
            }
            let Some(obj) = self.vm.heap.get_fast(eid) else {
                out.restore(cp);
                return false;
            };
            if !self.is_plain_dynamic(obj) {
                out.restore(cp);
                return false;
            }

            // Ultra-fast O(1) permuted slot path when `collection_members` and `cache_slots`
            // share element 0's exact layout.
            let perm_fast = perm_valid
                && obj.collection_members.len() == k
                && obj.cache_slots.len() == k
                && obj
                    .dynamic_regular_member_count
                    .is_none_or(|expected| expected == k);

            if perm_fast {
                let mut ok = true;
                for c in 0..k {
                    let expected_name = first_props[c].0;
                    let cm = &obj.collection_members[c];
                    let slot = &obj.cache_slots[slot_perm[c]];
                    if cm.name != *expected_name
                        || slot.key.entry
                        || slot.key.method
                        || slot.key.owner.is_some()
                        || slot.key.name != *expected_name
                    {
                        ok = false;
                        break;
                    }
                    if !Self::push_column_value(col_types[c], &slot.value, out) {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    if out.val_str_utf8.len().saturating_add(out.bin_bytes.len())
                        > MAX_COLUMNAR_PAYLOAD_BYTES
                    {
                        out.restore(cp);
                        return false;
                    }
                    continue;
                }
                // Fall through cleanly if this specific row has a different cache_slots order.
                out.restore(cp);
                return false;
            }

            row_props.clear();
            if !self.collect_plain_dynamic_props(obj, &mut row_props) || row_props.len() != k {
                out.restore(cp);
                return false;
            }
            for c in 0..k {
                if row_props[c].0 != first_props[c].0
                    || !Self::push_column_value(col_types[c], row_props[c].1, out)
                {
                    out.restore(cp);
                    return false;
                }
            }
            if out.val_str_utf8.len().saturating_add(out.bin_bytes.len())
                > MAX_COLUMNAR_PAYLOAD_BYTES
            {
                out.restore(cp);
                return false;
            }
        }
        true
    }

    #[inline]
    fn push_column_value(col_type: i32, val: &Value, out: &mut ColumnarValueBuffer) -> bool {
        match (col_type, val) {
            (COL_INT, Value::Int(n)) => {
                out.bigs.push(n.get());
                true
            }
            (COL_BOOL, Value::Bool(b)) => {
                out.bools.push(u8::from(*b));
                true
            }
            (COL_STR, Value::String(s)) => s.as_str().is_some_and(|st| out.push_val_str(st)),
            (COL_BYTES, Value::Bytes(b)) => out.push_bytes(b.as_ref()),
            (COL_FLOAT, Value::Float(f)) => {
                out.floats.push(**f);
                true
            }
            (COL_NULL, Value::Null | Value::NullWithDefault(_)) => true,
            _ => false,
        }
    }

    fn pack_value(&mut self, value: &Value, out: &mut ColumnarValueBuffer) -> bool {
        if self.remaining_nodes == 0
            || out.val_str_utf8.len().saturating_add(out.bin_bytes.len())
                > MAX_COLUMNAR_PAYLOAD_BYTES
        {
            return false;
        }
        self.remaining_nodes -= 1;
        match value {
            Value::Null | Value::NullWithDefault(_) => {
                out.ops.push(OP_NULL);
                true
            }
            Value::Bool(false) => {
                out.ops.push(OP_BOOL_FALSE);
                true
            }
            Value::Bool(true) => {
                out.ops.push(OP_BOOL_TRUE);
                true
            }
            Value::Int(n) => {
                out.ops.push(OP_INT);
                out.bigs.push(n.get());
                true
            }
            Value::Float(f) => {
                out.ops.push(OP_FLOAT);
                out.floats.push(**f);
                true
            }
            Value::String(s) => {
                let Some(st) = s.as_str() else {
                    return false;
                };
                out.ops.push(OP_STR);
                out.push_val_str(st)
            }
            Value::Bytes(b) => {
                out.ops.push(OP_BYTES);
                out.push_bytes(b.as_ref())
            }
            Value::Duration(d) => {
                out.ops.push(OP_DURATION);
                out.floats.push(d.value());
                out.push_val_str(d.unit().symbol())
            }
            Value::DataSize(d) => {
                out.ops.push(OP_DATASIZE);
                out.floats.push(d.value());
                out.push_val_str(d.unit().symbol())
            }
            Value::IntSeq(seq) => {
                out.ops.push(OP_INTSEQ);
                out.bigs.push(seq.start());
                out.bigs.push(seq.end());
                out.bigs.push(seq.step());
                true
            }
            Value::Regex(r) => {
                let Some(pat) = r.pattern().as_str() else {
                    return false;
                };
                out.ops.push(OP_REGEX);
                out.push_val_str(pat)
            }
            Value::Function(_) => {
                out.ops.push(OP_FUNCTION);
                true
            }
            Value::Class(_) | Value::TypeAlias(_) => false,
            Value::Object(id) => {
                if self.active.len() >= 32 || self.active.contains(id) {
                    return false;
                }
                let Some(obj) = self.vm.heap.get_fast(*id) else {
                    return false;
                };
                if let Some(pair) = obj.pair.as_ref() {
                    out.ops.push(OP_PAIR);
                    self.active.push(*id);
                    let ok = self.pack_value(&pair[0], out) && self.pack_value(&pair[1], out);
                    self.active.pop();
                    return ok;
                }
                if obj.reference.is_some()
                    || obj.foreign_map_entry.is_some()
                    || obj.function.is_some()
                    || obj.native_function.is_some()
                {
                    return false;
                }
                if obj.collection_kind == CollectionKind::List && obj.parent.is_none() {
                    let Some(elems) = obj.list_elements.as_ref() else {
                        return false;
                    };
                    let Ok(len_i32) = i32::try_from(elems.len()) else {
                        return false;
                    };
                    self.active.push(*id);
                    if self.try_pack_columnar_dynamic_list(elems, out) {
                        self.active.pop();
                        return true;
                    }
                    out.ops.push(OP_LIST);
                    out.ops.push(len_i32);
                    for elem in elems {
                        if !self.pack_value(elem, out) {
                            self.active.pop();
                            return false;
                        }
                    }
                    self.active.pop();
                    return true;
                }
                if obj.collection_kind != CollectionKind::Dynamic
                    || !obj.member_annotations.is_empty()
                    || obj
                        .collection_members
                        .iter()
                        .any(|cm| cm.kind != CollectionMemberKind::Property)
                {
                    return false;
                }
                // Case 1: plain un-amended `new Dynamic { ... }`
                if self.is_plain_dynamic(obj) {
                    let mut props = Vec::with_capacity(obj.collection_members.len().max(8));
                    if !self.collect_plain_dynamic_props(obj, &mut props) {
                        return false;
                    }
                    let mut prop_strs = Vec::with_capacity(props.len());
                    for (name, _) in &props {
                        let Some(ns) = name.as_str() else {
                            return false;
                        };
                        prop_strs.push(ns);
                    }
                    let Some(shape_id) = out.intern_shape("Dynamic", "pkl:base", &prop_strs) else {
                        return false;
                    };
                    out.ops.push(OP_SHAPED_OBJ);
                    out.ops.push(shape_id);
                    self.active.push(*id);
                    for (_, val) in &props {
                        if !self.pack_value(val, out) {
                            self.active.pop();
                            return false;
                        }
                    }
                    self.active.pop();
                    return true;
                }
                // Case 2: plain top-level module whose parent is None, `pkl.base#Module`, or `pkl:base`
                if obj.is_module && self.vm.is_render_directive(*id).ok() == Some(false) {
                    let parent_is_none_or_base = match obj.parent {
                        None => true,
                        Some(p) => self.vm.heap.get_fast(p).is_some_and(|pobj| {
                            pobj.builtin_type
                                .as_deref()
                                .and_then(classes::BuiltinClass::from_type)
                                == Some(classes::BuiltinClass::Ordinary(output::MODULE))
                                || pobj
                                    .module_unit
                                    .and_then(|u| self.vm.program.units.get(u.0 as usize))
                                    .and_then(|u| u.identity.as_ref())
                                    .is_some_and(|ident| ident.original_uri == "pkl:base")
                        }),
                    };
                    if !parent_is_none_or_base {
                        return false;
                    }
                    let Ok(Some(class)) = self.vm.heap.object_class(*id) else {
                        return false;
                    };
                    let Ok((class_name, class_uri)) = self.vm.binary_declared_class_identity(class)
                    else {
                        return false;
                    };
                    let (Some(cls_s), Some(uri_s)) = (class_name.as_str(), class_uri.as_str())
                    else {
                        return false;
                    };
                    if !is_valid_binary_ident(cls_s) || !is_valid_binary_ident(uri_s) {
                        return false;
                    }
                    let mut props: Vec<(&PklString, &Value)> = Vec::with_capacity(8);
                    if !obj.collection_members.is_empty() {
                        for cm in &obj.collection_members {
                            match collections::hidden_property(&self.vm.heap, *id, &cm.name, false)
                            {
                                Ok(true) => continue,
                                Ok(false) => {}
                                Err(_) => return false,
                            }
                            if props.iter().any(|(prev, _)| *prev == &cm.name) {
                                return false;
                            }
                            let Some(val) = super::fast_lookup_cached(obj, None, &cm.name, false)
                            else {
                                return false;
                            };
                            props.push((&cm.name, val));
                        }
                    } else {
                        for m in &obj.members {
                            if m.local
                                || m.method
                                || m.entry
                                || m.definition()
                                || m.import()
                                || m.external()
                                || m.hidden()
                            {
                                continue;
                            }
                            match collections::hidden_property(&self.vm.heap, *id, &m.name, false) {
                                Ok(true) => continue,
                                Ok(false) => {}
                                Err(_) => return false,
                            }
                            if props.iter().any(|(prev, _)| *prev == &m.name) {
                                return false;
                            }
                            let Some(val) = super::fast_lookup_cached(obj, None, &m.name, false)
                            else {
                                return false;
                            };
                            props.push((&m.name, val));
                        }
                    }
                    if self.vm.binary_regular_property_count(*id).ok() != Some(props.len()) {
                        return false;
                    }
                    let mut prop_strs = Vec::with_capacity(props.len());
                    for (name, _) in &props {
                        let Some(ns) = name.as_str() else {
                            return false;
                        };
                        prop_strs.push(ns);
                    }
                    let Some(shape_id) = out.intern_shape(cls_s, uri_s, &prop_strs) else {
                        return false;
                    };
                    out.ops.push(OP_SHAPED_OBJ);
                    out.ops.push(shape_id);
                    self.active.push(*id);
                    for (_, val) in &props {
                        if !self.pack_value(val, out) {
                            self.active.pop();
                            return false;
                        }
                    }
                    self.active.pop();
                    return true;
                }
                false
            }
        }
    }
}

impl Vm {
    /// Attempts to pack an already-warm-cached module's `"module"` or `"output.value"`
    /// directly into a compact columnar buffer without running VM bytecode or MessagePack
    /// serialization. Returns `false` whenever any member needs evaluation, the module is
    /// not warm-cached, or an edge case requires the full resumable `pkl-binary` renderer.
    pub(crate) fn try_pack_warm_expression_columnar(
        &mut self,
        uri: &PklString,
        expression: &str,
        out: &mut ColumnarValueBuffer,
    ) -> bool {
        if self.available().is_err() || self.code_validation.is_err() {
            return false;
        }
        let receiver = if let Some((last_uri, receiver)) = &self.last_warm_module
            && last_uri == uri
        {
            *receiver
        } else {
            if !self.cached_module_uris.contains(uri) {
                return false;
            }
            let Some(modules::CachedModule::Value(receiver)) = self.modules_original.get_ref(uri)
            else {
                return false;
            };
            let receiver = *receiver;
            self.last_warm_module = Some((uri.clone(), receiver));
            receiver
        };

        let expr_tag = match expression {
            "module" => 0u8,
            "output.value" => 1u8,
            _ => return false,
        };
        if out.cached_key == Some((self.gc_epoch, receiver, expr_tag))
            && self.heap.get_fast(receiver).is_some()
            && !out.as_bytes().is_empty()
        {
            return true;
        }

        let root = match expr_tag {
            0 => Value::Object(receiver),
            1 => {
                let Some(mod_obj) = self.heap.get_fast(receiver) else {
                    return false;
                };
                let output_name = PklString::from("output");
                let Some(Value::Object(out_id)) =
                    super::fast_lookup_cached(mod_obj, None, &output_name, false)
                else {
                    return false;
                };
                // Match `Instruction::CheckModuleOutput`: `output` must be a `ModuleOutput`.
                if !collections::typed_class_ref(&self.heap, *out_id)
                    .ok()
                    .flatten()
                    .is_some_and(|c| c.id == output::MODULE_OUTPUT)
                {
                    return false;
                }
                let Some(out_obj) = self.heap.get_fast(*out_id) else {
                    return false;
                };
                let value_name = PklString::from("value");
                let Some(val) = super::fast_lookup_cached(out_obj, None, &value_name, false) else {
                    return false;
                };
                val.clone()
            }
            _ => return false,
        };

        if self.try_pack_columnar_value(&root, out) {
            if out.byte_len() <= MAX_RETAINED_BYTES {
                out.cached_key = Some((self.gc_epoch, receiver, expr_tag));
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn try_pack_columnar_value(
        &self,
        root: &Value,
        out: &mut ColumnarValueBuffer,
    ) -> bool {
        out.reset_streams();
        let dynamic_class = self
            .builtin_classes
            .get(&classes::BuiltinClass::Dynamic)
            .copied();
        let mut ctx = PackCtx {
            vm: self,
            dynamic_class,
            empty_dynamic_parent_cache: None,
            active: Vec::with_capacity(8),
            remaining_nodes: MAX_COLUMNAR_NODES,
        };
        if !ctx.pack_value(root, out) {
            out.reset_streams();
            return false;
        }
        out.finish()
    }
}
