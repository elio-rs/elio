use async_stream::try_stream;
use futures::StreamExt;

use super::*;
use crate::execution::executor::Executor;

#[derive(Debug)]
pub struct PaginationExecutor {
    pub input: SharedExecutor,
    pub offset: u64,
    pub limit: u64,
    pub schema: Arc<Schema>,
}

impl Executor for PaginationExecutor {
    fn open(&self, ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let input_stream = self.input.open(ctx)?;
        let offset = self.offset as usize;
        let mut remaining = if self.limit == u64::MAX {
            None
        } else {
            Some(self.limit as usize)
        };

        let stream = try_stream! {
            let mut skipped = 0usize;
            futures::pin_mut!(input_stream);

            for await chunk_result in input_stream {
                let mut chunk = chunk_result?.compact();
                let chunk_len = chunk.len();

                if chunk_len == 0 {
                    continue;
                }

                if skipped < offset {
                    let to_skip = offset - skipped;
                    if to_skip >= chunk_len {
                        skipped += chunk_len;
                        continue;
                    }

                    let visibility = chunk.visibility_mut();
                    for i in 0..to_skip {
                        visibility.set(i, false);
                    }
                    skipped += to_skip;
                }

                if let Some(ref mut rem) = remaining {
                    if *rem == 0 {
                        break;
                    }

                    let mut visible = chunk.visible_row_len();
                    if visible > *rem {
                        let visibility = chunk.visibility_mut();
                        let mut kept = 0usize;
                        for i in 0..visibility.len() {
                            if visibility[i] {
                                kept += 1;
                                if kept > *rem {
                                    visibility.set(i, false);
                                }
                            }
                        }
                        visible = *rem;
                    }
                    *rem -= visible;
                }

                if chunk.visible_row_len() == 0 {
                    continue;
                }

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
        "Pagination"
    }
}
