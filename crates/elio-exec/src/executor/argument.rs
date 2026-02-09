use async_stream::try_stream;
use bitvec::vec::BitVec;
use elio_common::array::ArrayImpl;
use elio_common::array::chunk::DataChunk;
use futures::StreamExt;

use super::apply::ArgumentContext;
use super::*;

#[derive(Debug)]
pub struct ArgumentExecutor {
    pub schema: Arc<Schema>,
    pub argument_ctx: ArgumentContext,
}

impl Executor for ArgumentExecutor {
    fn open(&self, _ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let schema = self.schema.clone();
        let argument_ctx = self.argument_ctx.clone();

        let stream = try_stream! {
            let mut columns: Vec<Arc<ArrayImpl>> = Vec::with_capacity(schema.len());

            for (idx, field) in schema.columns().iter().enumerate() {
                let value = argument_ctx.get_value(idx);
                let physical_type = field.typ.physical_type();
                let mut builder = physical_type.array_builder(1);
                builder.push(value.as_ref().map(|v| v.as_scalar_ref()));
                columns.push(Arc::new(builder.finish()));
            }

            let mut visibility = BitVec::with_capacity(1);
            visibility.push(true);
            let chunk = DataChunk::new(columns, visibility);
            yield chunk;
        }
        .boxed();
        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "Argument"
    }
}
