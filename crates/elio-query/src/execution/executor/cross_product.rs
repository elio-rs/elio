//! CrossProductExecutor, the left and right may have overlapping variables.

use std::sync::Arc;

use async_stream::try_stream;
use elio_common::array::chunk::DataChunkBuilder;
use elio_common::schema::Schema;
use futures::StreamExt;

use super::apply::OutputColumnSource;
use super::*;

#[derive(Debug)]
pub struct CrossProductExecutor {
    pub left: SharedExecutor,
    pub right: SharedExecutor,
    pub schema: Arc<Schema>,
    /// Maps each output column to its source (left or right) and index
    pub output_mapping: Vec<OutputColumnSource>,
}

impl Executor for CrossProductExecutor {
    fn open(&self, qctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError> {
        let left = self.left.clone();
        let right = self.right.clone();
        let schema = self.schema.clone();
        let output_mapping = self.output_mapping.clone();

        let stream = try_stream! {
            // materialize the right side into memory as data chunks
            let mut right_chunks = Vec::new();
            let right_stream = right.open(qctx.clone())?;
            futures::pin_mut!(right_stream);

            while let Some(chunk_result) = right_stream.next().await {
                let chunk = chunk_result?.compact();
                right_chunks.push(chunk);
            }

            // if right side is empty, no output
            if right_chunks.iter().all(|x| x.visible_row_len() == 0) {
                return;
            }

            // iterate over left side and produce cartesian product
            let mut out_builder = DataChunkBuilder::new(
                schema.columns().iter().map(|col| col.typ.clone()),
                CHUNK_SIZE
            );

            let left_stream = left.open(qctx.clone())?;
            futures::pin_mut!(left_stream);

            while let Some(left_chunk_result) = left_stream.next().await {
                let left_chunk = left_chunk_result?.compact();

                for left_row in left_chunk.iter() {
                    // Cross product: for each right row, produce output
                    for right_chunk in &right_chunks {
                        for right_row in right_chunk.iter() {
                            let mut output_row = Vec::with_capacity(output_mapping.len());
                            for source in &output_mapping {
                                let val = match source {
                                    OutputColumnSource::Left(idx) => left_row.get(*idx).cloned().flatten(),
                                    OutputColumnSource::Right(idx) => right_row.get(*idx).cloned().flatten(),
                                };
                                output_row.push(val);
                            }
                            if let Some(chunk) = out_builder.append_row(output_row) {
                                yield chunk;
                            }
                        }
                    }
                }
            }

            if let Some(chunk) = out_builder.yield_chunk() {
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
        "CrossProduct"
    }
}
