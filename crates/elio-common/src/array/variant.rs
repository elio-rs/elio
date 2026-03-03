use std::sync::Arc;

use bitvec::prelude::*;

use super::*;
use crate::array::Array;

#[derive(Debug, Clone)]
pub struct VariantArray {
    data: Arc<[VariantValue]>,
    valid: BitVec,
    logical_type: Arc<LogicalType>,
}

impl Array for VariantArray {
    type RefItem<'a> = VariantRef<'a>;

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

    fn logical_type(&self) -> &LogicalType {
        &self.logical_type
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = VariantArrayBuilder::with_capacity(new_len, self.logical_type().clone());

        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl VariantArray {
    pub fn valid_map(&self) -> &BitVec {
        &self.valid
    }

    pub fn set_valid_map(&mut self, valid: BitVec) {
        self.valid = valid;
    }
}

#[derive(Debug)]
pub struct VariantArrayBuilder {
    data: Vec<VariantValue>,
    valid: BitVec,
    logical_type: Arc<LogicalType>,
}

impl VariantArrayBuilder {
    pub fn with_capacity(capacity: usize, logical_type: LogicalType) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            valid: BitVec::with_capacity(capacity),
            logical_type: Arc::new(logical_type),
        }
    }

    pub fn push_n(&mut self, item: Option<VariantRef<'_>>, repeat: usize) {
        if let Some(item) = item {
            self.data.extend(std::iter::repeat_n(item.to_owned_scalar(), repeat));
            self.valid.extend(std::iter::repeat_n(true, repeat));
        } else {
            self.data.extend(std::iter::repeat_n(VariantValue::default(), repeat));
            self.valid.extend(std::iter::repeat_n(false, repeat));
        }
    }

    pub fn push(&mut self, item: Option<VariantRef<'_>>) {
        self.push_n(item, 1);
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> VariantArray {
        VariantArray {
            data: self.data.into(),
            valid: self.valid,
            logical_type: self.logical_type,
        }
    }

    pub fn logical_type(&self) -> &LogicalType {
        &self.logical_type
    }
}
