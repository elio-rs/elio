use async_stream::try_stream;
use futures::StreamExt;

use super::*;
use crate::execution::error::ExecError;
use crate::execution::executor::Executor;
use crate::execution::expr::Expression;

#[derive(Debug)]
pub struct FilterExecutor {
    pub input: SharedExecutor,
    pub filter: Arc<dyn Expression>,
    pub schema: Arc<Schema>,
}

// TODO(pgao): short circuit filter
impl Executor for FilterExecutor {
    fn open(&self, qctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError> {
        let filter = self.filter.clone();
        let input_stream = self.input.open(qctx.clone())?;

        let stream = try_stream! {
            let eval_ctx = qctx.derive_eval_ctx();

            for await chunk in input_stream {
                let mut chunk = chunk?;
                let res = filter.eval_batch(&chunk, &eval_ctx)?;
                let bool_array = res.as_bool().expect("filter should result in bool array");
                let mask = bool_array.to_filter_mask();
                let visibility = chunk.visibility_mut();
                *visibility &= mask;
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
        "Filter"
    }
}
