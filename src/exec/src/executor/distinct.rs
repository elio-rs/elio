//! DistinctExecutor: Stream the input stream and remove duplicate rows.

use std::sync::Arc;

use async_stream::try_stream;
use educe::Educe;
use elio_common::array::{DataChunk, DataChunkBuilder};
use elio_common::schema::Schema;
use elio_expr::impl_::SharedExpression;
use futures::StreamExt;
use hashbrown::HashSet;
use itertools::{self, Itertools};

use crate::executor::{CHUNK_SIZE, Executor, SharedExecutor};

#[derive(Educe)]
#[educe(Debug)]
pub struct DistinctExecutor {
    pub(crate) input: SharedExecutor,
    pub(crate) group_exprs: Vec<SharedExpression>,
    pub(crate) schema: Arc<Schema>,
}

impl Executor for DistinctExecutor {
    fn open(&self, ctx: Arc<crate::task::TaskExecContext>) -> Result<super::DataChunkStream, crate::error::ExecError> {
        let input = self.input.clone();
        let group_exprs = self.group_exprs.clone();
        let schema = self.schema.clone();

        let stream = try_stream! {
            let input_stream = input.open(ctx.clone())?;

            let mut chunk_builder = DataChunkBuilder::new_from_schema(&schema, CHUNK_SIZE);
            let eval_ctx = ctx.derive_eval_ctx();

            for await chunk in input_stream {
                let chunk = chunk?;
                if chunk.visible_row_len() == 0 {
                    continue;
                }

                // distict state: set of seen group keys
                // TODO(pgao): use hashbrow raw set
                let mut seen = HashSet::new();

                // evaluate group_exprs
                let mut group_cols = vec![];
                for expr in group_exprs.iter() {
                    let column = expr.eval_batch(&chunk, &eval_ctx)?;
                    group_cols.push(column);
                }

                let group_chunk = DataChunk::new(group_cols, chunk.visibility().clone());
                for row_ref in group_chunk.iter() {

                    let row = row_ref.iter().map(|x| x.unwrap().to_owned_scalar()).collect_vec();
                    if seen.contains(&row) {
                        continue;
                    }
                    seen.insert(row);
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
