use async_stream::try_stream;
use elio_common::schema::Schema;
use futures::StreamExt;

use super::*;
use crate::executor::Executor;

#[derive(Debug, Default)]
pub struct UnitExecutor {
    schema: Schema,
}

impl Executor for UnitExecutor {
    fn open(&self, _ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let stream = try_stream! {
            // return one empty row
            yield DataChunk::unit();
        }
        .boxed();

        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "Unit"
    }
}
