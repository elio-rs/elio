use bitvec::vec::BitVec;

use crate::array::{Array, PhysicalType};

#[derive(Debug, Clone)]
pub struct BoolArray {
    data: BitVec,
    valid: BitVec,
}

impl Array for BoolArray {
    type RefItem<'a> = bool;

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
        PhysicalType::Bool
    }

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
        let mut builder = BoolArrayBuilder::with_capacity(new_len);

        for idx in visibility.iter_ones() {
            builder.push(self.get(idx));
        }

        builder.finish()
    }
}

impl BoolArray {
    pub fn from_parts(data: BitVec, valid: BitVec) -> Self {
        Self { data, valid }
    }

    pub fn valid_map(&self) -> &BitVec {
        &self.valid
    }

    pub fn set_valid_map(&mut self, valid: BitVec) {
        self.valid = valid;
    }

    pub fn to_filter_mask(&self) -> BitVec {
        self.data.clone() & self.valid.clone()
    }
}

#[derive(Debug)]
pub struct BoolArrayBuilder {
    data: BitVec,
    valid: BitVec,
}

impl BoolArrayBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: BitVec::with_capacity(capacity),
            valid: BitVec::with_capacity(capacity),
        }
    }

    pub fn push_n(&mut self, value: Option<bool>, repeat: usize) {
        if let Some(value) = value {
            self.data.extend(std::iter::repeat_n(value, repeat));
            self.valid.extend(std::iter::repeat_n(true, repeat));
        } else {
            self.data.extend(std::iter::repeat_n(false, repeat));
            self.valid.extend(std::iter::repeat_n(false, repeat));
        }
    }

    pub fn push(&mut self, value: Option<bool>) {
        self.push_n(value, 1);
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.valid.len()
    }

    pub fn finish(self) -> BoolArray {
        let data = self.data;
        let valid = self.valid;
        BoolArray { data, valid }
    }
}

#[cfg(test)]
mod tests {
    use bitvec::prelude::*;

    use super::*;

    #[test]
    fn test_bool_array_builder() {
        let mut builder = BoolArrayBuilder::with_capacity(5);
        builder.push(Some(true));
        builder.push(Some(false));
        builder.push(None);
        builder.push_n(Some(true), 2);
        let arr = builder.finish();

        assert_eq!(arr.len(), 5);
        assert_eq!(arr.get(0), Some(true));
        assert_eq!(arr.get(1), Some(false));
        assert_eq!(arr.get(2), None);
        assert_eq!(arr.get(3), Some(true));
        assert_eq!(arr.get(4), Some(true));
    }

    #[test]
    fn test_bool_array_len() {
        let mut builder = BoolArrayBuilder::with_capacity(0);
        builder.push(Some(true));
        builder.push(None);
        let arr = builder.finish();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_bool_array_physical_type() {
        let mut builder = BoolArrayBuilder::with_capacity(0);
        builder.push(Some(true));
        let arr = builder.finish();
        assert_eq!(arr.physical_type(), PhysicalType::Bool);
    }

    #[test]
    fn test_bool_array_valid_map() {
        let mut builder = BoolArrayBuilder::with_capacity(3);
        builder.push(Some(true));
        builder.push(None);
        builder.push(Some(false));
        let arr = builder.finish();

        let expected_valid_map = bitvec![1, 0, 1];
        assert_eq!(arr.valid_map(), &expected_valid_map);
    }

    #[test]
    fn test_bool_array_set_valid_map() {
        let mut builder = BoolArrayBuilder::with_capacity(2);
        builder.push(Some(true));
        builder.push(Some(false));
        let mut arr = builder.finish();

        let new_valid_map = bitvec![0, 1];
        arr.set_valid_map(new_valid_map.clone());

        assert_eq!(arr.valid_map(), &new_valid_map);
        assert_eq!(arr.get(0), None);
        assert_eq!(arr.get(1), Some(false));
    }

    #[test]
    fn test_bool_array_to_filter_mask() {
        let mut builder = BoolArrayBuilder::with_capacity(5);
        builder.push(Some(true));
        builder.push(Some(false));
        builder.push(None);
        builder.push_n(Some(true), 2);
        let arr = builder.finish();

        // data: 1 0 0 1 1
        // valid: 1 1 0 1 1
        // filter_mask: 1 0 0 1 1
        let expected_filter_mask = bitvec![1, 0, 0, 1, 1];
        assert_eq!(arr.to_filter_mask(), expected_filter_mask);
    }
}
