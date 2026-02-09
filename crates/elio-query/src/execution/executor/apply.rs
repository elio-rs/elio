use std::sync::{Arc, RwLock};

use async_stream::try_stream;
use elio_common::array::chunk::DataChunkBuilder;
use elio_common::scalar::ScalarValue;
use futures::StreamExt;

use super::*;

/// Shared context for passing values from Apply to Argument
#[derive(Debug, Clone, Default)]
pub struct ArgumentContext {
    // TODO(pgao): currently we are single thread executor, the rwlock is not necessary.
    inner: Arc<RwLock<Option<Vec<ScalarValue>>>>,
}

impl ArgumentContext {
    pub fn set_row(&self, values: Vec<ScalarValue>) {
        *self.inner.write().unwrap() = Some(values);
    }

    pub fn get_value(&self, idx: usize) -> Option<ScalarValue> {
        self.inner.read().unwrap().as_ref()?.get(idx).cloned()
    }
}

/// Specifies where an output column comes from
#[derive(Debug, Clone, Copy)]
pub enum OutputColumnSource {
    Left(usize),
    Right(usize),
}

#[derive(Debug)]
pub struct ApplyExecutor {
    pub left: SharedExecutor,
    pub right: SharedExecutor,
    pub argument_ctx: ArgumentContext,
    pub schema: Arc<Schema>,
    /// Maps argument variable indices to left column indices
    pub argument_mapping: Vec<usize>,
    /// Maps each output column to its source (left or right) and index
    pub output_mapping: Vec<OutputColumnSource>,
}

impl Executor for ApplyExecutor {
    fn open(&self, ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        // Clone Arc-wrapped data to move into the async block
        let left = self.left.clone();
        let right = self.right.clone();
        let argument_ctx = self.argument_ctx.clone();
        let schema = self.schema.clone();
        let argument_mapping = self.argument_mapping.clone();
        let output_mapping = self.output_mapping.clone();

        let left_stream = left.open(ctx.clone())?;

        let stream = try_stream! {
            let mut out_builder = DataChunkBuilder::new(
                schema.columns().iter().map(|col| col.typ.physical_type()),
                CHUNK_SIZE
            );

            for await left_chunk_result in left_stream {
                let left_chunk = left_chunk_result?.compact();

                for left_row in left_chunk.iter() {
                    // Extract argument values from left row and update shared context
                    let arg_values: Vec<ScalarValue> = argument_mapping
                        .iter()
                        .map(|&col_idx| {
                            left_row.get(col_idx)
                                .and_then(|v| v.map(|r| r.to_owned_scalar()))
                                .unwrap_or(ScalarValue::Unknown)
                        })
                        .collect();
                    argument_ctx.set_row(arg_values);

                    // Open a new stream from the right executor (reuses executor, just new stream)
                    let mut right_stream = right.open(ctx.clone())?;

                    while let Some(right_chunk_result) = right_stream.next().await {
                        let right_chunk = right_chunk_result?.compact();

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

                if let Some(chunk) = out_builder.yield_chunk() {
                    yield chunk;
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
        "Apply"
    }
}
