use std::sync::Arc;

use bitvec::prelude::*;

use super::*;
use crate::array::{Array, PhysicalType};

#[derive(Debug, Clone)]
pub struct AnyArray {
    data: Arc<[ScalarValue]>,
    valid: BitVec,
}

impl Array for AnyArray {
    type RefItem<'a> = ScalarRef<'a>;

    fn get(&self, idx: usize) -> Option<Self::RefItem<'_>> {
        self.valid.get(idx).and_then(|valid| {
            if *valid {
                Some(self.data[idx].as_scalar_ref())
            } else {
                None
            }
        })
    }

    unsafe fn get_unchecked(&self, idx: usize) -> Self::RefItem<'_> {
        self.data[idx].as_scalar_ref()
    }

    fn len(&self) -> usize {
        self.valid.len()
    }

    fn physical_type(&self) -> PhysicalType {
        PhysicalType::Any
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = AnyArrayBuilder::with_capacity(new_len);

        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl AnyArray {
    pub fn valid_map(&self) -> &BitVec {
        &self.valid
    }

    pub fn set_valid_map(&mut self, valid: BitVec) {
        self.valid = valid;
    }
}

#[derive(Debug)]
pub struct AnyArrayBuilder {
    data: Vec<ScalarValue>,
    valid: BitVec,
}

impl AnyArrayBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            valid: BitVec::with_capacity(capacity),
        }
    }

    pub fn push_n(&mut self, item: Option<ScalarRef<'_>>, repeat: usize) {
        if let Some(item) = item {
            self.data.extend(std::iter::repeat_n(item.to_owned_scalar(), repeat));
            self.valid.extend(std::iter::repeat_n(true, repeat));
        } else {
            self.data.extend(std::iter::repeat_n(ScalarValue::default(), repeat));
            self.valid.extend(std::iter::repeat_n(false, repeat));
        }
    }

    pub fn push(&mut self, item: Option<ScalarRef<'_>>) {
        self.push_n(item, 1);
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> AnyArray {
        AnyArray {
            data: self.data.into(),
            valid: self.valid,
        }
    }
}
