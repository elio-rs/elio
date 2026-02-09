//! BlackHoleExecutor will eagerly consume the input stream and return an empty stream.

use async_stream::try_stream;
use futures::StreamExt;

use super::*;

#[derive(Debug)]
pub struct BlackHoleExecutor {
    pub input: SharedExecutor,
    pub schema: Arc<Schema>,
}

impl Executor for BlackHoleExecutor {
    fn open(&self, ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let input_stream = self.input.open(ctx.clone())?;
        let schema = self.schema.clone();

        let stream = try_stream! {
            // consume the input stream and return an empty stream
            for await chunk in input_stream {
                let _ = chunk?;
            }
            // return an empty stream
            yield DataChunk::empty(&schema);
        }
        .boxed();
        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "BlackHole"
    }
}
