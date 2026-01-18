use std::iter::once;
use std::sync::Arc;

use bitvec::prelude::*;

use super::*;

#[derive(Debug, Clone)]
pub struct NodeArray {
    ids: Arc<[NodeId]>,
    label_offsets: Arc<[usize]>,
    label_values: Arc<[Arc<str>]>,
    // TODO(pgao): this is actually not struct value
    // the properties can be stored in node and rel have some constraint.
    // only supported type can be stored in it
    // an optimization would be store an row bytes as props
    // and when query props, we can deserialize it on the fly
    props: Arc<[StructValue]>,
    valid: BitVec,
}

impl Array for NodeArray {
    type RefItem<'a> = NodeValueRef<'a>;

    fn get(&self, idx: usize) -> Option<Self::RefItem<'_>> {
        self.valid.get(idx).and_then(|valid| {
            if *valid {
                Some(NodeValueRef {
                    id: self.ids[idx],
                    labels: &self.label_values[self.label_offsets[idx]..self.label_offsets[idx + 1]],
                    props: self.props[idx].as_scalar_ref(),
                })
            } else {
                None
            }
        })
    }

    unsafe fn get_unchecked(&self, idx: usize) -> Self::RefItem<'_> {
        NodeValueRef {
            id: self.ids[idx],
            labels: &self.label_values[self.label_offsets[idx]..self.label_offsets[idx + 1]],
            props: self.props[idx].as_scalar_ref(),
        }
    }

    fn len(&self) -> usize {
        self.valid.len()
    }

    fn physical_type(&self) -> PhysicalType {
        PhysicalType::Node
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = NodeArrayBuilder::with_capacity(new_len);
        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl NodeArray {
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
        self.valid.iter().zip(vis.iter()).enumerate().map(|(i, (v, visible))| {
            if *v && *visible {
                Some(self.props[i].as_scalar_ref())
            } else {
                None
            }
        })
    }

    pub fn slice(&self, start: usize, end: usize) -> Self {
        let mut offsets = Vec::with_capacity(end - start + 1);
        let mut values = Vec::new();
        for i in start..end + 1 {
            offsets.push(self.label_offsets[i] - self.label_offsets[start]);
        }
        values.extend_from_slice(&self.label_values[self.label_offsets[start]..self.label_offsets[end]]);

        Self {
            ids: self.ids[start..end].to_vec().into(),
            label_offsets: offsets.into_boxed_slice().into(),
            label_values: values.into_boxed_slice().into(),
            props: self.props[start..end].to_vec().into(),
            valid: self.valid[start..end].to_bitvec(),
        }
    }
}

#[derive(Debug)]
pub struct NodeArrayBuilder {
    ids: Vec<NodeId>,
    labels: Vec<Vec<Arc<str>>>,
    props: Vec<StructValue>,
    valid: BitVec,
}

impl NodeArrayBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: Vec::with_capacity(capacity),
            labels: Vec::with_capacity(capacity),
            props: Vec::with_capacity(capacity),
            valid: BitVec::with_capacity(capacity),
        }
    }

    pub fn push_n(&mut self, value: Option<NodeValueRef<'_>>, repeat: usize) {
        if let Some(value) = value {
            self.ids.extend(std::iter::repeat_n(value.id, repeat));
            self.labels.extend(std::iter::repeat_n(value.labels.to_vec(), repeat));
            self.props
                .extend(std::iter::repeat_n(value.props.to_owned_scalar(), repeat));
            self.valid.extend(std::iter::repeat_n(true, repeat));
        } else {
            self.ids.extend(std::iter::repeat_n(NodeId::default(), repeat));
            self.labels.extend(std::iter::repeat_n(Vec::new(), repeat));
            self.props.extend(std::iter::repeat_n(StructValue::default(), repeat));
            self.valid.extend(std::iter::repeat_n(false, repeat));
        }
    }

    pub fn push(&mut self, value: Option<NodeValueRef<'_>>) {
        self.push_n(value, 1);
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> NodeArray {
        let ids = self.ids.into();
        let label_offsets = once(0)
            .chain(self.labels.iter().scan(0, |acc, x| {
                *acc += x.len();
                let offset = *acc;
                Some(offset)
            }))
            .collect::<Vec<_>>()
            .into();
        let label_values = self.labels.into_iter().flatten().collect::<Vec<_>>().into();
        let props = self.props.into();
        let valid = self.valid;
        NodeArray {
            ids,
            label_offsets,
            label_values,
            props,
            valid,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VirtualNodeArray {
    data: Arc<[NodeId]>,
    valid: BitVec,
}

impl Array for VirtualNodeArray {
    type RefItem<'a> = NodeId;

    fn get(&self, idx: usize) -> Option<Self::RefItem<'_>> {
        self.valid
            .get(idx)
            .and_then(|valid| if *valid { Some(self.data[idx]) } else { None })
    }

    unsafe fn get_unchecked(&self, idx: usize) -> Self::RefItem<'_> {
        self.data[idx]
    }

    fn len(&self) -> usize {
        self.valid.len()
    }

    fn physical_type(&self) -> PhysicalType {
        PhysicalType::VirtualNode
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = VirtualNodeArrayBuilder::with_capacity(new_len);
        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl VirtualNodeArray {
    pub fn valid_map(&self) -> &BitVec {
        &self.valid
    }

    pub fn set_valid_map(&mut self, valid: BitVec) {
        self.valid = valid;
    }

    pub fn slice(&self, start: usize, end: usize) -> Self {
        Self {
            data: self.data[start..end].to_vec().into(),
            valid: self.valid[start..end].into(),
        }
    }
}

#[derive(Debug)]
pub struct VirtualNodeArrayBuilder {
    data: Vec<NodeId>,
    valid: BitVec,
}

impl VirtualNodeArrayBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            valid: BitVec::with_capacity(capacity),
        }
    }

    pub fn push_n(&mut self, value: Option<NodeId>, repeat: usize) {
        if let Some(value) = value {
            self.data.extend(std::iter::repeat_n(value, repeat));
            self.valid.extend(std::iter::repeat_n(true, repeat));
        } else {
            self.data.extend(std::iter::repeat_n(NodeId::default(), repeat));
            self.valid.extend(std::iter::repeat_n(false, repeat));
        }
    }

    pub fn push(&mut self, value: Option<NodeId>) {
        self.push_n(value, 1);
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> VirtualNodeArray {
        let data = self.data.into();
        let valid = self.valid;
        VirtualNodeArray { data, valid }
    }
}

impl EntityArray for NodeArray {}
