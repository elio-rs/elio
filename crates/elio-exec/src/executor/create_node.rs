use async_stream::try_stream;
use elio_common::IrToken;
use elio_common::schema::Variable;
use elio_expr::impl_::SharedExpression;
use futures::StreamExt;

use super::constraint::{check_unique_constraints, fetch_constraints_for_labels, update_unique_indexes};
use super::*;

// input: Schema
// output: Schema + Node
#[derive(Debug)]
pub struct CreateNodeExectuor {
    pub input: SharedExecutor,
    pub schema: Arc<Schema>,
    pub items: Vec<CreateNodeItem>,
}

#[derive(Debug, Clone)]
pub struct CreateNodeItem {
    pub labels: Vec<IrToken>,
    // the return type should be struct
    pub properties: SharedExpression,
    pub variable: Variable,
}

impl Executor for CreateNodeExectuor {
    fn open(&self, ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let items = self.items.clone();
        let mut input_stream = self.input.open(ctx.clone())?;

        let stream = try_stream! {

            let eval_ctx = ctx.derive_eval_ctx();

            // Prepare labels for each item
            let label_vec: Vec<Vec<Arc<str>>> = items
                .iter()
                .map(|item| item.labels.iter().map(|label| label.name().clone()).collect())
                .collect();

            // Pre-fetch constraints for all labels
            let label_constraints: Vec<_> = label_vec
                .iter()
                .map(|labels| fetch_constraints_for_labels(ctx.store(), ctx.tx(), labels))
                .collect::<Result<_, _>>()?;

            // Acquire read locks for all labels (to prevent concurrent CREATE CONSTRAINT)
            let all_label_ids: Vec<_> = label_vec
                .iter()
                .flat_map(|labels| labels.iter().filter_map(|l| ctx.store().token_store().get_label_id(l)))
                .collect();
            let _locks = ctx.store().acquire_labels_read(&all_label_ids);

            // Execute the stream
            while let Some(chunk) = input_stream.next().await {
                let chunk = chunk?;
                let mut chunk = chunk.compact();

                // For each CREATE item, create nodes with constraint checking
                for (i, item) in items.iter().enumerate() {
                    let prop = item.properties.eval_batch(&chunk, &eval_ctx)?;
                    let prop_struct = prop.as_struct().ok_or_else(|| ExecError::type_mismatch(
                        "create_node",
                        "struct",
                        prop.physical_type(),
                    ))?;

                    // Check constraints before creating nodes
                    check_unique_constraints(ctx.store(), ctx.tx(), &label_constraints[i], prop_struct)?;

                    // Create the nodes
                    let output = ctx.tx().node_create(&label_vec[i], &prop)?;

                    // Update unique indexes for the created nodes
                    update_unique_indexes(ctx.store(), ctx.tx(), &label_constraints[i], prop_struct, &output)?;

                    chunk.add_column(Arc::new(output.into()));
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
        "CreateNode"
    }
}
