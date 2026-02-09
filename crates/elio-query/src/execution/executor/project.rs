use async_stream::try_stream;
use futures::StreamExt;

use super::*;
use crate::execution::expr::SharedExpression;

#[derive(Debug)]
pub struct ProjectExecutor {
    pub(crate) input: SharedExecutor,
    pub(crate) exprs: Vec<SharedExpression>,
    pub(crate) schema: Arc<Schema>,
}

impl Executor for ProjectExecutor {
    fn open(&self, ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let exprs = self.exprs.clone();

        let input_stream = self.input.open(ctx.clone())?;

        let stream = try_stream! {
            let eval_ctx = ctx.derive_eval_ctx();

            let mut input_stream = input_stream;
            while let Some(chunk) = input_stream.next().await {
                let chunk = chunk?;
                let mut cols = vec![];
                for expr in exprs.iter() {
                    let column = expr.eval_batch(&chunk, &eval_ctx)?;
                    cols.push(column);
                }
                let output = DataChunk::new(cols, chunk.visibility().clone());
                yield output;
           }
        }
        .boxed();
        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "Project"
    }
}
