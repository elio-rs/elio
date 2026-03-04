use std::sync::Arc;

use async_stream::try_stream;
use elio_common::array::DataChunkBuilder;
use elio_common::scalar::ScalarRef;
use elio_common::schema::Schema;
use futures::StreamExt;

use crate::execution::QueryContext;
use crate::execution::error::ExecError;
use crate::execution::executor::{CHUNK_SIZE, DataChunkStream, Executor, SharedExecutor};
use crate::execution::expr::SharedExpression;

#[derive(Debug)]
pub struct UnwindExecutor {
    pub(crate) input: SharedExecutor,
    /// The list expression to unwind
    pub(crate) expr: SharedExpression,
    pub(crate) schema: Arc<Schema>,
}

impl Executor for UnwindExecutor {
    fn open(&self, qctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError> {
        let input = self.input.clone();
        let expr = self.expr.clone();
        let schema = self.schema.clone();
        let input_col_count = input.schema().columns().len();

        let stream = try_stream! {
            let input_stream = input.open(qctx.clone())?;
            let eval_ctx = qctx.derive_eval_ctx();
            let mut chunk_builder = DataChunkBuilder::new_from_schema(&schema, CHUNK_SIZE);

            for await chunk_result in input_stream {
                let chunk = chunk_result?;
                if chunk.visible_row_len() == 0 {
                    continue;
                }

                // Compact the chunk so visibility is all-true and we can index by row
                let chunk = chunk.compact();
                let list_col = expr.eval_batch(&chunk, &eval_ctx)?;
                let row_count = chunk.len();

                for row_idx in 0..row_count {
                    // Get the list value at this row
                    let list_scalar = list_col.get(row_idx);

                    let list_ref = match list_scalar {
                        Some(ScalarRef::List(l)) => l,
                        _ => {
                            // NULL or non-list: produce one row with NULL element
                            let mut row = Vec::with_capacity(input_col_count + 1);
                            for col in chunk.columns() {
                                row.push(col.get(row_idx));
                            }
                            row.push(None); // NULL element
                            if let Some(out_chunk) = chunk_builder.append_row(row) {
                                yield out_chunk;
                            }
                            continue;
                        }
                    };

                    if list_ref.len() == 0 {
                        // Empty list: skip this row (no output)
                        continue;
                    }

                    // For each element in the list, emit a row
                    for elem in list_ref.iter() {
                        let mut row = Vec::with_capacity(input_col_count + 1);
                        for col in chunk.columns() {
                            row.push(col.get(row_idx));
                        }
                        row.push(Some(elem));
                        if let Some(out_chunk) = chunk_builder.append_row(row) {
                            yield out_chunk;
                        }
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
        "Unwind"
    }
}
