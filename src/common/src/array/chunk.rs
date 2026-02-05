use std::sync::Arc;

use bitvec::vec::BitVec;
use itertools::Itertools;

use super::*;
use crate::array::{ArrayBuilderImpl, ArrayImpl, ArrayRef, PhysicalType};
use crate::schema::Schema;

#[derive(Clone)]
pub struct DataChunk {
    columns: Vec<Arc<ArrayImpl>>,
    visibility: BitVec,
}

impl DataChunk {
    pub fn unit() -> Self {
        Self {
            columns: vec![],
            visibility: BitVec::repeat(true, 1),
        }
    }

    /// Return an chunk with all columns empty
    pub fn empty(schema: &Schema) -> Self {
        let columns = schema
            .columns()
            .iter()
            .map(|col| col.typ.physical_type().array_builder(0))
            .map(|b| Arc::new(b.finish()))
            .collect_vec();
        Self {
            columns,
            visibility: BitVec::repeat(true, 0),
        }
    }

    pub fn new(columns: Vec<Arc<ArrayImpl>>, visibility: BitVec) -> Self {
        let len = visibility.len();

        if cfg!(debug_assertions) {
            assert!(
                columns.iter().all(|c| c.len() == len),
                "All columns in a DataChunk must have the same length"
            );
        }
        Self { columns, visibility }
    }

    pub fn add_column(&mut self, column: Arc<ArrayImpl>) {
        assert_eq!(column.len(), self.len());
        self.columns.push(column);
    }

    pub fn visibility_mut(&mut self) -> &mut BitVec {
        &mut self.visibility
    }

    pub fn column(&self, idx: usize) -> ArrayRef {
        self.columns[idx].clone()
    }

    pub fn columns(&self) -> &[Arc<ArrayImpl>] {
        &self.columns
    }

    pub fn visibility(&self) -> &BitVec {
        &self.visibility
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.visibility.len()
    }

    // valid row len
    pub fn visible_row_len(&self) -> usize {
        self.visibility.count_ones()
    }

    pub fn iter(&self) -> ChunkIter<'_> {
        ChunkIter {
            chunk: self,
            idx: 0,
            len: self.visibility.len(),
        }
    }

    pub fn compact(&self) -> DataChunk {
        // if all true,just clone
        if self.visibility.all() {
            return self.clone();
        }
        let card = self.visible_row_len();
        let mut new_columns = vec![];

        for col in &self.columns {
            new_columns.push(Arc::new(col.compact(&self.visibility, card)));
        }

        DataChunk {
            columns: new_columns,
            visibility: BitVec::repeat(true, card),
        }
    }
}

pub struct ChunkIter<'a> {
    chunk: &'a DataChunk,
    idx: usize,
    // visbility map len
    len: usize,
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = Vec<Option<ScalarRef<'a>>>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.idx < self.len {
            // TODO(pgao): optimize visibility map
            if self.chunk.visibility[self.idx] {
                let mut row = vec![];
                for col in self.chunk.columns.iter() {
                    row.push(col.get(self.idx));
                }
                self.idx += 1;
                return Some(row);
            }
            self.idx += 1;
        }
        None
    }
}

pub struct DataChunkBuilder {
    // array data types
    types: Vec<PhysicalType>,
    // chunk capacity, if full build the data chunk
    capacity: usize,

    columns: Vec<ArrayBuilderImpl>,
    len: usize,
}

impl DataChunkBuilder {
    pub fn new(types: impl Iterator<Item = PhysicalType>, capacity: usize) -> Self {
        let types = types.collect_vec();
        let columns = types.iter().map(|t| t.array_builder(capacity)).collect_vec();
        Self {
            types,
            capacity,
            columns,
            len: 0,
        }
    }

    pub fn new_from_schema(schema: &Schema, capacity: usize) -> Self {
        let types = schema.columns().iter().map(|c| c.typ.physical_type());
        Self::new(types, capacity)
    }

    pub fn append_row(&mut self, row: Vec<Option<ScalarRef<'_>>>) -> Option<DataChunk> {
        assert_eq!(row.len(), self.columns.len());
        for (col, item) in self.columns.iter_mut().zip_eq(row) {
            col.push(item);
        }
        self.len += 1;
        if self.len >= self.capacity {
            Some(self.build_chunk())
        } else {
            None
        }
    }

    pub fn yield_chunk(&mut self) -> Option<DataChunk> {
        (self.len > 0).then(|| self.build_chunk())
    }

    // build the data chunk and reset the builder
    fn build_chunk(&mut self) -> DataChunk {
        let builders = std::mem::take(&mut self.columns);
        let columns = builders.into_iter().map(|b| Arc::new(b.finish())).collect_vec();
        let chunk = DataChunk::new(columns, BitVec::repeat(true, self.len));
        self.reset();
        chunk
    }

    fn reset(&mut self) {
        self.len = 0;
        self.columns.clear();
        for dtype in self.types.iter() {
            self.columns.push(dtype.array_builder(self.capacity));
        }
    }
}
