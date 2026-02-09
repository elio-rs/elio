use async_stream::try_stream;
use elio_expr::impl_::Expression;
use futures::StreamExt;

use super::*;
use crate::error::ExecError;
use crate::executor::Executor;

#[derive(Debug)]
pub struct FilterExecutor {
    pub input: SharedExecutor,
    pub filter: Arc<dyn Expression>,
    pub schema: Arc<Schema>,
}

// TODO(pgao): short circuit filter
impl Executor for FilterExecutor {
    fn open(&self, ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let filter = self.filter.clone();
        let input_stream = self.input.open(ctx.clone())?;

        let stream = try_stream! {
            let eval_ctx = ctx.derive_eval_ctx();

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
