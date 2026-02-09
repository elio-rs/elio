use std::sync::Arc;

use bitvec::vec::BitVec;
use derive_more::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct PathArray {
    nodes: Arc<ListArray>, // inner should have type of node
    rels: Arc<ListArray>,  // inner should have type of rel
    valid: BitVec,
}

impl Array for PathArray {
    type RefItem<'a> = PathValueRef<'a>;

    fn get(&self, idx: usize) -> Option<Self::RefItem<'_>> {
        self.valid.get(idx).and_then(|valid| {
            if *valid {
                let (nodes, node_start, node_end) = {
                    let list = self.nodes.get(idx).expect("path node list should not be null");
                    match list {
                        ListValueRef::Index { child, start, end } => (child, start, end),
                        _ => unreachable!(),
                    }
                };

                let (rels, rel_start, rel_end) = {
                    match self.rels.get(idx).expect("path rel list should not be null") {
                        ListValueRef::Index { child, start, end } => (child, start, end),
                        _ => unreachable!(),
                    }
                };
                Some(PathValueRef {
                    nodes,
                    node_start,
                    node_end,
                    rels,
                    rel_start,
                    rel_end,
                })
            } else {
                None
            }
        })
    }

    unsafe fn get_unchecked(&self, idx: usize) -> Self::RefItem<'_> {
        let (nodes, node_start, node_end) = {
            let list = unsafe { self.nodes.get_unchecked(idx) };
            match list {
                ListValueRef::Index { child, start, end } => (child, start, end),
                _ => unreachable!(),
            }
        };

        let (rels, rel_start, rel_end) = {
            match unsafe { self.rels.get_unchecked(idx) } {
                ListValueRef::Index { child, start, end } => (child, start, end),
                _ => unreachable!(),
            }
        };
        PathValueRef {
            nodes,
            node_start,
            node_end,
            rels,
            rel_start,
            rel_end,
        }
    }

    fn len(&self) -> usize {
        self.valid.len()
    }

    fn physical_type(&self) -> PhysicalType {
        PhysicalType::Path
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = PathArrayBuilder::with_capacity(new_len);
        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl PathArray {
    pub fn from_parts(nodes: Arc<ListArray>, rels: Arc<ListArray>, valid: BitVec) -> Self {
        Self { nodes, rels, valid }
    }

    pub fn valid_map(&self) -> &BitVec {
        &self.valid
    }

    pub fn set_valid_map(&mut self, valid: BitVec) {
        self.valid = valid;
    }
}

#[derive(Debug)]
pub struct PathArrayBuilder {
    nodes: ListArrayBuilder, // node
    rels: ListArrayBuilder,  // rel
    valid: BitVec,
}

impl PathArrayBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: ListArrayBuilder::new(Box::new(NodeArrayBuilder::with_capacity(capacity).into())),
            rels: ListArrayBuilder::new(Box::new(RelArrayBuilder::with_capacity(capacity).into())),
            valid: BitVec::with_capacity(capacity),
        }
    }

    pub fn push_n(&mut self, value: Option<PathValueRef<'_>>, repeat: usize) {
        if let Some(value) = value {
            self.nodes.push_n(Some(value.node_list_ref()), repeat);
            self.rels.push_n(Some(value.rel_list_ref()), repeat);
            self.valid.extend(std::iter::repeat_n(true, repeat));
        } else {
            self.nodes.push_n(None, repeat);
            self.rels.push_n(None, repeat);
            self.valid.extend(std::iter::repeat_n(false, repeat));
        }
    }

    pub fn push(&mut self, value: Option<PathValueRef<'_>>) {
        self.push_n(value, 1);
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> PathArray {
        PathArray {
            nodes: Arc::new(self.nodes.finish()),
            rels: Arc::new(self.rels.finish()),
            valid: self.valid,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VirtualPathArray {
    pub nodes: Arc<ListArray>,
    pub rels: Arc<ListArray>,
    pub valid: BitVec,
}

impl VirtualPathArray {
    pub fn from_parts(nodes: Arc<ListArray>, rels: Arc<ListArray>, valid: BitVec) -> Self {
        Self { nodes, rels, valid }
    }

    pub fn into_parts(self) -> (Arc<ListArray>, Arc<ListArray>, BitVec) {
        (self.nodes, self.rels, self.valid)
    }
}

impl Array for VirtualPathArray {
    type RefItem<'a> = VirtualPathRef<'a>;

    fn get(&self, idx: usize) -> Option<Self::RefItem<'_>> {
        self.valid.get(idx).and_then(|valid| {
            if *valid {
                let (nodes, node_start, node_end) = {
                    let list = self.nodes.get(idx).expect("path node list should not be null");
                    match list {
                        ListValueRef::Index { child, start, end } => (child, start, end),
                        _ => unreachable!(),
                    }
                };
                let (rels, rel_start, rel_end) = {
                    match self.rels.get(idx).expect("path rel list should not be null") {
                        ListValueRef::Index { child, start, end } => (child, start, end),
                        _ => unreachable!(),
                    }
                };
                Some(VirtualPathRef {
                    nodes,
                    node_start,
                    node_end,
                    rels,
                    rel_start,
                    rel_end,
                })
            } else {
                None
            }
        })
    }

    unsafe fn get_unchecked(&self, idx: usize) -> Self::RefItem<'_> {
        let (nodes, node_start, node_end) = {
            let list = unsafe { self.nodes.get_unchecked(idx) };
            match list {
                ListValueRef::Index { child, start, end } => (child, start, end),
                _ => unreachable!(),
            }
        };

        let (rels, rel_start, rel_end) = {
            match unsafe { self.rels.get_unchecked(idx) } {
                ListValueRef::Index { child, start, end } => (child, start, end),
                _ => unreachable!(),
            }
        };
        VirtualPathRef {
            nodes,
            node_start,
            node_end,
            rels,
            rel_start,
            rel_end,
        }
    }

    fn len(&self) -> usize {
        self.valid.len()
    }

    fn physical_type(&self) -> PhysicalType {
        PhysicalType::VirtualPath
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = VirtualPathArrayBuilder::with_capacity(new_len);
        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl VirtualPathArray {
    pub fn valid_map(&self) -> &BitVec {
        &self.valid
    }

    pub fn set_valid_map(&mut self, valid: BitVec) {
        self.valid = valid;
    }
}

#[derive(Debug)]
pub struct VirtualPathArrayBuilder {
    nodes: ListArrayBuilder,
    rels: ListArrayBuilder,
    valid: BitVec,
}

impl VirtualPathArrayBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: ListArrayBuilder::new(Box::new(VirtualNodeArrayBuilder::with_capacity(capacity).into())),
            rels: ListArrayBuilder::new(Box::new(RelArrayBuilder::with_capacity(capacity).into())),
            valid: BitVec::with_capacity(capacity),
        }
    }

    pub fn push_n(&mut self, value: Option<VirtualPathRef<'_>>, repeat: usize) {
        if let Some(value) = value {
            self.nodes.push_n(Some(value.node_list_ref()), repeat);
            self.rels.push_n(Some(value.rel_list_ref()), repeat);
            self.valid.extend(std::iter::repeat_n(true, repeat));
        } else {
            self.nodes.push_n(None, repeat);
            self.rels.push_n(None, repeat);
            self.valid.extend(std::iter::repeat_n(false, repeat));
        }
    }

    pub fn push(&mut self, value: Option<VirtualPathRef<'_>>) {
        self.push_n(value, 1);
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> VirtualPathArray {
        VirtualPathArray {
            nodes: Arc::new(self.nodes.finish()),
            rels: Arc::new(self.rels.finish()),
            valid: self.valid,
        }
    }
}
