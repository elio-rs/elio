use async_stream::try_stream;
use elio_common::array::ArrayImpl;
use futures::StreamExt;

use super::*;
use crate::execution::expr::SharedExpression;

// input: Schema
// output: Schema + Node
#[derive(Debug)]
pub struct CreateRelExectuor {
    pub input: SharedExecutor,
    pub schema: Arc<Schema>,
    pub items: Vec<CreateRelItem>,
}

#[derive(Debug, Clone)]
pub struct CreateRelItem {
    pub rtype: Arc<str>,
    pub start: usize, // index of start node
    pub end: usize,   // index of end node
    // the return type should be struct
    pub properties: SharedExpression,
}

impl Executor for CreateRelExectuor {
    fn open(&self, qctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError> {
        let items = self.items.clone();
        let input_stream = self.input.open(qctx.clone())?;

        let stream = try_stream! {
            let eval_ctx = qctx.derive_eval_ctx();

            // execute the stream
            for await chunk in input_stream {
                let chunk = chunk?;
                // TODO(pgao): do not eager compact the chunk
                let mut chunk = chunk.compact();
                // for each variable execute create node
                for (i, item) in items.iter().enumerate() {
                    let prop = item.properties.eval_batch(&chunk, &eval_ctx)?;
                    let prop = prop.as_ref().as_struct().ok_or_else(||
                        ExecError::type_mismatch(
                            format!("create rel item {}", i),
                            "struct",
                            prop.logical_type().clone(),
                        ))?;

                    let start = chunk.column(item.start);
                    let end = chunk.column(item.end);

                    let output = match (start.as_ref(), end.as_ref()) {
                        (ArrayImpl::VirtualNode(start), ArrayImpl::VirtualNode(end)) => {
                            qctx.graph_store().rel_create(qctx.txn().as_ref(), &item.rtype, start, end, prop).map_err(|e| e.into())
                        }
                        (ArrayImpl::Node(start), ArrayImpl::Node(end)) => {
                            qctx.graph_store().rel_create(qctx.txn().as_ref(), &item.rtype, start, end, prop).map_err(|e| e.into())
                        }
                        (ArrayImpl::VirtualNode(start), ArrayImpl::Node(end)) => {
                            qctx.graph_store().rel_create(qctx.txn().as_ref(), &item.rtype, start, end, prop).map_err(|e| e.into())
                        }
                        (ArrayImpl::Node(start), ArrayImpl::VirtualNode(end)) => {
                            qctx.graph_store().rel_create(qctx.txn().as_ref(), &item.rtype, start, end, prop).map_err(|e| e.into())
                        }
                        (s, e) => {
                            let pt = if !matches!(s, ArrayImpl::Node(_) | ArrayImpl::VirtualNode(_)) {
                                s.logical_type().clone()
                            } else {
                                e.logical_type().clone()
                            };
                            Err(ExecError::type_mismatch(
                                format!("create rel item {} node", i),
                                "node or virtual node",
                                pt,
                            ))
                        }
                    }?;
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
        "CreateRel"
    }
}
