//! this implement is row-wise hash aggregate.
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use async_stream::try_stream;
use educe::Educe;
use elio_common::array::chunk::{DataChunk, DataChunkBuilder};
use elio_common::scalar::{Datum, DatumRef};
use futures::StreamExt;
use hashbrown::raw::RawTable;

use super::*;
use crate::execution::error::EvalError;
use crate::execution::executor::apply::OutputColumnSource;
use crate::execution::expr::agg_call::{AggFuncImpl, AggFuncState};

/// HashAggregate inputs must be column references.
#[derive(Educe)]
#[educe(Debug)]
pub struct HashAggregateExecutor {
    pub(crate) input: SharedExecutor,
    // group by column indexes in the input chunk
    pub(crate) group_by: Vec<usize>,
    // argument column indexes for each aggregate, parallel to `aggs`
    pub(crate) agg_args: Vec<Vec<usize>>,
    /// mapping from output columns to either group key idx (Left) or agg idx (Right)
    pub(crate) output_mapping: Vec<OutputColumnSource>,
    #[educe(Debug(ignore))]
    pub(crate) aggs: Vec<Arc<dyn AggFuncImpl>>,
    pub(crate) schema: Arc<Schema>,
}

// for each group, we have an RowContainer, which contains the group_keys and aggregate states.
#[derive(Educe)]
#[educe(Debug)]
pub struct RowContainer {
    pub keys: Vec<Datum>,
    #[educe(Debug(ignore))]
    pub states: Vec<Box<dyn AggFuncState>>,
}

type RowIdx = usize;

// holds an set of RowContainers, which contains the group_keys and aggregate states.
pub struct GroupingSet {
    pub agg_args: Vec<Vec<usize>>,
    pub aggs: Vec<Arc<dyn AggFuncImpl>>,
    pub rows: Vec<RowContainer>,
    // hash table to map the group key to the row index in rows.
    pub index: RawTable<RowIdx>,
}

impl GroupingSet {
    pub fn new(agg_args: Vec<Vec<usize>>, aggs: Vec<Arc<dyn AggFuncImpl>>) -> Self {
        Self {
            agg_args,
            aggs,
            rows: Vec::new(),
            index: RawTable::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn lookup(&self, key: &[Datum]) -> Option<RowIdx> {
        let hash = Self::hash_key(key);
        self.index
            // TODO(pgao): we should customize the eq here.
            // in cypher 1.0 = 1.
            .get(hash, |&idx| self.rows[idx].keys.as_slice() == key)
            .copied()
    }

    /// get states by given row index
    #[inline]
    pub fn states_mut(&mut self, row_idx: RowIdx) -> &mut [Box<dyn AggFuncState>] {
        &mut self.rows[row_idx].states
    }

    /// accumulate a row into the grouping set
    /// if the key is not found, a new row will be created
    /// if the key is found, the states will be updated
    /// return the row index of the affected group
    pub fn accumulate_row(&mut self, key: &[Datum], input_row: &[DatumRef]) -> Result<RowIdx, EvalError> {
        let row_idx = if let Some(idx) = self.lookup(key) {
            idx
        } else {
            let states = self.aggs.iter().map(|agg| agg.create_state()).collect();
            let row_idx = self.rows.len();
            self.rows.push(RowContainer {
                keys: key.to_vec(),
                states,
            });
            let hash = Self::hash_key(key);
            self.index
                .insert(hash, row_idx, |&idx| Self::hash_key(&self.rows[idx].keys));
            row_idx
        };

        let aggs = self.aggs.clone();
        let arg_idx_list = self.agg_args.clone();
        let states = self.states_mut(row_idx);
        for ((state, agg), arg_idx) in states.iter_mut().zip(aggs.iter()).zip(arg_idx_list.iter()) {
            let args: Vec<DatumRef> = arg_idx
                .iter()
                .map(|idx| input_row.get(*idx).copied().unwrap_or(None))
                .collect();
            agg.update_state(state, &args)?;
        }
        Ok(row_idx)
    }

    /// finalize all groups and produce (group_keys, agg_results) per group.
    /// finalize into data chunks using provided schema/order.
    /// TODO(pgao): the result be an iterator
    pub fn finalize_chunks(
        &mut self,
        output_mapping: &[OutputColumnSource],
        schema: &Schema,
        chunk_size: usize,
    ) -> Vec<DataChunk> {
        let mut builder = DataChunkBuilder::new(schema.columns().iter().map(|c| c.typ.physical_type()), chunk_size);
        let mut chunks = Vec::new();

        for rc in self.rows.iter_mut() {
            let agg_vals: Vec<Datum> = rc
                .states
                .iter_mut()
                .zip(self.aggs.iter())
                .map(|(state, agg)| agg.extract_value(state))
                .collect();

            let mut out_row = Vec::with_capacity(schema.columns().len());
            for source in output_mapping.iter() {
                let val = match source {
                    OutputColumnSource::Left(idx) => {
                        rc.keys.get(*idx).and_then(|d| d.as_ref()).map(|v| v.as_scalar_ref())
                    }
                    OutputColumnSource::Right(idx) => {
                        agg_vals.get(*idx).and_then(|d| d.as_ref()).map(|v| v.as_scalar_ref())
                    }
                };
                out_row.push(val);
            }
            if let Some(chunk) = builder.append_row(out_row) {
                chunks.push(chunk);
            }
        }
        if let Some(chunk) = builder.yield_chunk() {
            chunks.push(chunk);
        }
        chunks
    }

    // TODO(pgao): use an costomized hash function. since we cusotmized the compare eq function in lookup_or_insert
    fn hash_key(key: &[Datum]) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

impl Executor for HashAggregateExecutor {
    fn open(&self, qctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError> {
        let input = self.input.clone();
        let group_idx = self.group_by.clone();
        let agg_arg_idx = self.agg_args.clone();
        let aggs = self.aggs.clone();
        let schema = self.schema.clone();
        let output_mapping = self.output_mapping.clone();

        let stream = try_stream! {
            let mut grouping = GroupingSet::new(agg_arg_idx.clone(), aggs.clone());

            let input_stream = input.open(qctx.clone())?;
            for await chunk_res in input_stream {
                let chunk = chunk_res?;
                if chunk.visible_row_len() == 0 {
                    continue;
                }

                // row-wise accumulation using column refs only
                for row_idx in chunk.visibility().iter_ones() {
                    // build group key from columns
                    let mut key = Vec::with_capacity(group_idx.len());
                    for &col_idx in group_idx.iter() {
                        let datum = chunk.column(col_idx).get(row_idx).map(|sr| sr.to_owned_scalar());
                        key.push(datum);
                    }

                    // build row buffer for all input columns so aggregation args can reuse
                    let col_count = chunk.columns().len();
                    let mut row_vals: Vec<Datum> = Vec::with_capacity(col_count);
                    for col_idx in 0..col_count {
                        let datum: Datum = chunk.column(col_idx).get(row_idx).map(|sr| sr.to_owned_scalar());
                        row_vals.push(datum);
                    }
                    let row_refs: Vec<DatumRef> = row_vals.iter().map(|d| d.as_ref()).collect();

                    let _ = grouping.accumulate_row(&key, &row_refs)?;
                }
            }

            // finalize and emit output chunks
            for chunk in grouping.finalize_chunks(&output_mapping, &schema, CHUNK_SIZE) {
                yield chunk;
            }
        }
        .boxed();

        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "HashAggregate"
    }
}
