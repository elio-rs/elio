use std::sync::Arc;

use bitvec::prelude::*;

use super::*;

#[derive(Debug, Clone)]
pub struct ListArray {
    offsets: Arc<[usize]>,
    child: Arc<ArrayImpl>,
    valid: BitVec,
}

impl Array for ListArray {
    type RefItem<'a> = ListValueRef<'a>;

    fn get(&self, idx: usize) -> Option<Self::RefItem<'_>> {
        self.valid.get(idx).and_then(|valid| {
            if *valid {
                let start = self.offsets[idx];
                let end = self.offsets[idx + 1];
                let child = self.child.as_ref();
                Some(ListValueRef::Index { child, start, end })
            } else {
                None
            }
        })
    }

    unsafe fn get_unchecked(&self, idx: usize) -> Self::RefItem<'_> {
        let start = self.offsets[idx];
        let end = self.offsets[idx + 1];
        let child = self.child.as_ref();
        ListValueRef::Index { child, start, end }
    }

    fn len(&self) -> usize {
        self.valid.len()
    }

    fn physical_type(&self) -> PhysicalType {
        PhysicalType::List(Box::new(self.child.physical_type()))
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = self.physical_type().array_builder(new_len).into_list().unwrap();

        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl ListArray {
    pub fn from_parts(offsets: Arc<[usize]>, child: Arc<ArrayImpl>, valid: BitVec) -> Self {
        Self { offsets, child, valid }
    }

    pub fn into_parts(self) -> (Arc<[usize]>, Arc<ArrayImpl>, BitVec) {
        (self.offsets, self.child, self.valid)
    }

    pub fn child(&self) -> &ArrayRef {
        &self.child
    }

    pub fn valid_map(&self) -> &BitVec {
        &self.valid
    }

    pub fn set_valid_map(&mut self, valid: BitVec) {
        self.valid = valid;
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }
}

#[derive(Debug)]
pub struct ListArrayBuilder {
    offsets: Vec<usize>,
    child: Box<ArrayBuilderImpl>,
    valid: BitVec,
}

impl ListArrayBuilder {
    pub fn new(child: Box<ArrayBuilderImpl>) -> Self {
        Self {
            offsets: vec![0],
            child,
            valid: BitVec::new(),
        }
    }

    pub fn child(&mut self) -> &mut ArrayBuilderImpl {
        &mut self.child
    }

    pub fn push_n(&mut self, item: Option<ListValueRef<'_>>, repeat: usize) {
        if let Some(item) = item {
            self.valid.extend(std::iter::repeat_n(true, repeat));
            for _ in 0..repeat {
                for value in item.iter() {
                    self.child.push(Some(value))
                }
                let last_offset = *self.offsets.last().unwrap();
                self.offsets.push(last_offset + item.len());
            }
        } else {
            self.valid.extend(std::iter::repeat_n(false, repeat));
            let last_offset = *self.offsets.last().unwrap();
            for _ in 0..repeat {
                self.offsets.push(last_offset);
            }
        }
    }

    pub fn push(&mut self, item: Option<ListValueRef<'_>>) {
        self.push_n(item, 1);
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> ListArray {
        let offsets = self.offsets.into();
        let child = self.child.finish().into();
        let valid = self.valid;
        ListArray { offsets, child, valid }
    }
}

impl ListArrayBuilder {
    // push virtual node list into list array
    // #panic: if the underlying array builder is not virtual node builder
    pub fn push_virtual_nodes(&mut self, nodes: impl Iterator<Item = NodeId>) {
        let child = self.child.as_virtual_node_mut().expect("expected virtual node");
        let mut len = 0;
        for node in nodes {
            child.push(Some(node));
            len += 1;
        }
        self.valid.push(true);
        self.offsets.push(*self.offsets.last().unwrap() + len);
    }

    pub fn push_rels<'a>(&mut self, rels: impl Iterator<Item = RelValueRef<'a>>) {
        let child = self.child.as_rel_mut().expect("expected rel");
        let mut len = 0;
        for rel in rels {
            child.push(Some(rel));
            len += 1;
        }
        self.valid.push(true);
        self.offsets.push(*self.offsets.last().unwrap() + len);
    }
}
