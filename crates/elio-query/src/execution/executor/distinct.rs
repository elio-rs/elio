//! DistinctExecutor: Stream the input stream and remove duplicate rows.

use std::sync::Arc;

use async_stream::try_stream;
use educe::Educe;
use elio_common::array::{DataChunk, DataChunkBuilder};
use elio_common::scalar::Row;
use elio_common::schema::Schema;
use futures::StreamExt;
use hashbrown::HashSet;
use itertools::{self, Itertools};

use crate::execution::QueryContext;
use crate::execution::error::ExecError;
use crate::execution::executor::{CHUNK_SIZE, DataChunkStream, Executor, SharedExecutor};
use crate::execution::expr::SharedExpression;

#[derive(Educe)]
#[educe(Debug)]
pub struct DistinctExecutor {
    pub(crate) input: SharedExecutor,
    pub(crate) group_exprs: Vec<SharedExpression>,
    pub(crate) schema: Arc<Schema>,
}

#[derive(Default)]
struct DistinctState {
    // TODO(pgao): use hashbrown raw table
    seen: HashSet<Row>,
}

impl DistinctState {
    pub fn contains(&self, row: &Row) -> bool {
        self.seen.contains(row)
    }

    pub fn insert(&mut self, row: Row) {
        self.seen.insert(row);
    }
}

impl Executor for DistinctExecutor {
    fn open(&self, qctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError> {
        let input = self.input.clone();
        let group_exprs = self.group_exprs.clone();
        let schema = self.schema.clone();

        let stream = try_stream! {
            let input_stream = input.open(qctx.clone())?;

            let mut chunk_builder = DataChunkBuilder::new_from_schema(&schema, CHUNK_SIZE);
            let mut state = DistinctState::default();
            let eval_ctx = qctx.derive_eval_ctx();

            for await chunk in input_stream {
                let chunk = chunk?;
                if chunk.visible_row_len() == 0 {
                    continue;
                }

                // evaluate group_exprs
                let mut group_cols = vec![];
                for expr in group_exprs.iter() {
                    let column = expr.eval_batch(&chunk, &eval_ctx)?;
                    group_cols.push(column);
                }

                let group_chunk = DataChunk::new(group_cols, chunk.visibility().clone());
                for row_ref in group_chunk.iter() {
                    let row = row_ref.iter().map(|x| x.map(|x| x.to_owned_scalar())).collect_vec();
                    if state.contains(&row) {
                        continue;
                    }
                    state.insert(row);
                    if let Some(out_chunk) = chunk_builder.append_row(row_ref) {
                        yield out_chunk;
                    }
                }
            }

            if let Some(out_chunk) = chunk_builder.yield_chunk() {
                yield out_chunk;
            }
        }
        .boxed();
        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "Distinct"
    }
}
