use std::iter;
use std::sync::Arc;

use bitvec::prelude::*;

use super::*;

#[derive(Debug, Clone)]
pub struct RelArray {
    ids: Arc<[RelationshipId]>,
    reltypes: Arc<[Arc<str>]>,
    start_ids: Arc<[NodeId]>,
    end_ids: Arc<[NodeId]>,
    props: Arc<[StructValue]>,
    valid: BitVec,
}

impl Array for RelArray {
    type RefItem<'a> = RelValueRef<'a>;

    fn get(&self, idx: usize) -> Option<Self::RefItem<'_>> {
        self.valid.get(idx).and_then(|valid| {
            if *valid {
                Some(RelValueRef {
                    id: self.ids[idx],
                    reltype: &self.reltypes[idx],
                    start_id: self.start_ids[idx],
                    end_id: self.end_ids[idx],
                    props: self.props[idx].as_scalar_ref(),
                })
            } else {
                None
            }
        })
    }

    unsafe fn get_unchecked(&self, idx: usize) -> Self::RefItem<'_> {
        RelValueRef {
            id: self.ids[idx],
            reltype: &self.reltypes[idx],
            start_id: self.start_ids[idx],
            end_id: self.end_ids[idx],
            props: self.props[idx].as_scalar_ref(),
        }
    }

    fn len(&self) -> usize {
        self.valid.len()
    }

    fn physical_type(&self) -> PhysicalType {
        PhysicalType::Rel
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = RelArrayBuilder::with_capacity(new_len);
        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl RelArray {
    pub fn valid_map(&self) -> &BitVec {
        &self.valid
    }

    pub fn set_valid_map(&mut self, valid: BitVec) {
        self.valid = valid;
    }

    pub fn props_iter(&self) -> impl Iterator<Item = Option<StructValueRef<'_>>> + '_ {
        self.valid
            .iter()
            .enumerate()
            .map(|(i, v)| if *v { Some(self.props[i].as_scalar_ref()) } else { None })
    }

    pub fn props_iter_with_visibility<'a>(
        &'a self,
        vis: &'a BitVec,
    ) -> impl Iterator<Item = Option<StructValueRef<'a>>> + 'a {
        debug_assert_eq!(self.len(), vis.len(), "visibility length mismatch");
        self.valid.iter().zip(vis.iter()).enumerate().map(|(i, (v, visible))| {
            if *v && *visible {
                Some(self.props[i].as_scalar_ref())
            } else {
                None
            }
        })
    }

    pub fn slice(&self, start: usize, end: usize) -> Self {
        Self {
            ids: self.ids[start..end].to_vec().into(),
            reltypes: self.reltypes[start..end].to_vec().into(),
            start_ids: self.start_ids[start..end].to_vec().into(),
            end_ids: self.end_ids[start..end].to_vec().into(),
            props: self.props[start..end].to_vec().into(),
            valid: self.valid[start..end].into(),
        }
    }
}

#[derive(Debug)]
pub struct RelArrayBuilder {
    ids: Vec<RelationshipId>,
    reltypes: Vec<Arc<str>>,
    start_ids: Vec<NodeId>,
    end_ids: Vec<NodeId>,
    props: Vec<StructValue>,
    valid: BitVec,
}

impl RelArrayBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: Vec::with_capacity(capacity),
            reltypes: Vec::with_capacity(capacity),
            start_ids: Vec::with_capacity(capacity),
            end_ids: Vec::with_capacity(capacity),
            props: Vec::with_capacity(capacity),
            valid: BitVec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, item: Option<RelValueRef<'_>>) {
        self.push_n(item, 1);
    }

    pub fn push_n(&mut self, item: Option<RelValueRef<'_>>, repeat: usize) {
        match item {
            Some(item) => {
                self.ids.extend(std::iter::repeat_n(item.id, repeat));
                self.reltypes
                    .extend(std::iter::repeat_n(item.reltype.to_owned(), repeat));
                self.start_ids.extend(std::iter::repeat_n(item.start_id, repeat));
                self.end_ids.extend(std::iter::repeat_n(item.end_id, repeat));
                self.props.extend(iter::repeat_n(item.props.to_owned_scalar(), repeat));
                self.valid.extend(std::iter::repeat_n(true, repeat));
            }
            None => {
                self.ids.extend(std::iter::repeat_n(RelationshipId::default(), repeat));
                self.reltypes.extend(std::iter::repeat_n(Arc::default(), repeat));
                self.start_ids.extend(std::iter::repeat_n(NodeId::default(), repeat));
                self.end_ids.extend(std::iter::repeat_n(NodeId::default(), repeat));
                self.props.extend(iter::repeat_n(StructValue::default(), repeat));
                self.valid.extend(std::iter::repeat_n(false, repeat));
            }
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> RelArray {
        let ids = self.ids.into();
        let reltypes = self.reltypes.into();
        let start_ids = self.start_ids.into();
        let end_ids = self.end_ids.into();
        let props = self.props.into();
        let valid = self.valid;
        RelArray {
            ids,
            reltypes,
            start_ids,
            end_ids,
            props,
            valid,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VirtualRelArray {
    ids: Arc<[RelationshipId]>,
    reltypes: Arc<[String]>,
    start_ids: Arc<[NodeId]>,
    end_ids: Arc<[NodeId]>,
    valid: BitVec,
}

impl Array for VirtualRelArray {
    type RefItem<'a> = VirtualRelRef<'a>;

    fn get(&self, idx: usize) -> Option<Self::RefItem<'_>> {
        self.valid.get(idx).and_then(|valid| {
            if *valid {
                Some(VirtualRelRef {
                    id: self.ids[idx],
                    reltype: &self.reltypes[idx],
                    start_id: self.start_ids[idx],
                    end_id: self.end_ids[idx],
                })
            } else {
                None
            }
        })
    }

    unsafe fn get_unchecked(&self, idx: usize) -> Self::RefItem<'_> {
        VirtualRelRef {
            id: self.ids[idx],
            reltype: &self.reltypes[idx],
            start_id: self.start_ids[idx],
            end_id: self.end_ids[idx],
        }
    }

    fn len(&self) -> usize {
        self.valid.len()
    }

    fn physical_type(&self) -> PhysicalType {
        PhysicalType::VirtualRel
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = VirtualRelArrayBuilder::with_capacity(new_len);
        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl VirtualRelArray {
    pub fn physical_type(&self) -> PhysicalType {
        PhysicalType::VirtualRel
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
pub struct VirtualRelArrayBuilder {
    ids: Vec<RelationshipId>,
    reltypes: Vec<String>,
    start_ids: Vec<NodeId>,
    end_ids: Vec<NodeId>,
    valid: BitVec,
}

impl VirtualRelArrayBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: Vec::with_capacity(capacity),
            reltypes: Vec::with_capacity(capacity),
            start_ids: Vec::with_capacity(capacity),
            end_ids: Vec::with_capacity(capacity),
            valid: BitVec::with_capacity(capacity),
        }
    }

    pub fn push_n(&mut self, item: Option<VirtualRelRef<'_>>, repeat: usize) {
        match item {
            Some(item) => {
                self.ids.extend(std::iter::repeat_n(item.id, repeat));
                self.reltypes
                    .extend(std::iter::repeat_n(item.reltype.to_string(), repeat));
                self.start_ids.extend(std::iter::repeat_n(item.start_id, repeat));
                self.end_ids.extend(std::iter::repeat_n(item.end_id, repeat));
                self.valid.extend(std::iter::repeat_n(true, repeat));
            }
            None => {
                self.ids.extend(std::iter::repeat_n(RelationshipId::default(), repeat));
                self.reltypes.extend(std::iter::repeat_n(String::default(), repeat));
                self.start_ids.extend(std::iter::repeat_n(NodeId::default(), repeat));
                self.end_ids.extend(std::iter::repeat_n(NodeId::default(), repeat));
                self.valid.extend(std::iter::repeat_n(false, repeat));
            }
        }
    }

    pub fn push(&mut self, item: Option<VirtualRelRef<'_>>) {
        self.push_n(item, 1);
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> VirtualRelArray {
        let ids = self.ids.into();
        let reltypes = self.reltypes.into();
        let start_ids = self.start_ids.into();
        let end_ids = self.end_ids.into();
        let valid = self.valid;
        VirtualRelArray {
            ids,
            reltypes,
            start_ids,
            end_ids,
            valid,
        }
    }
}

impl EntityArray for RelArray {}
