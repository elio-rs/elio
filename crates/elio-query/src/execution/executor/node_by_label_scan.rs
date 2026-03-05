use std::sync::Arc;

use async_stream::try_stream;
use elio_common::IrToken;
use elio_common::array::chunk::{DataChunk, DataChunkBuilder};
use elio_common::schema::Schema;
use elio_storage::transaction::NodeScanOptions;
use futures::StreamExt;
use tokio::sync::mpsc;

use super::*;
use crate::execution::executor::Executor;
use crate::execution::executor::apply::OutputColumnSource;

const CHANNEL_BUFFER_SIZE: usize = 128;

#[derive(Debug)]
pub struct NodeByLabelScanExecutor {
    pub schema: Arc<Schema>,
    pub label: IrToken,
    pub input: Option<SharedExecutor>,
    pub output_mapping: Option<Vec<OutputColumnSource>>,
}

impl NodeByLabelScanExecutor {
    pub fn new(schema: Arc<Schema>, label: IrToken) -> Self {
        Self {
            schema,
            label,
            input: None,
            output_mapping: None,
        }
    }

    pub fn with_input(
        schema: Arc<Schema>,
        label: IrToken,
        input: SharedExecutor,
        output_mapping: Vec<OutputColumnSource>,
    ) -> Self {
        Self {
            schema,
            label,
            input: Some(input),
            output_mapping: Some(output_mapping),
        }
    }
}

fn scan_nodes_by_label(
    qctx: &Arc<QueryContext>,
    label_id: elio_common::TokenId,
) -> mpsc::Receiver<Result<DataChunk, ExecError>> {
    let (tx, rx) = mpsc::channel::<Result<DataChunk, ExecError>>(CHANNEL_BUFFER_SIZE);
    let txn = qctx.tx.clone();
    let graph_store = qctx.db.graph_store().clone();

    tokio::task::spawn_blocking(move || {
        let opts = NodeScanOptions { batch_size: CHUNK_SIZE };
        let mut iter = match graph_store.node_scan_by_label(&txn, label_id, opts) {
            Ok(iter) => iter,
            Err(e) => {
                let _ = tx.blocking_send(Err(e.into()));
                return;
            }
        };
        loop {
            match iter.next_batch() {
                Ok(Some(chunk)) => {
                    if tx.blocking_send(Ok(chunk)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    let _ = tx.blocking_send(Err(e.into()));
                    break;
                }
            }
        }
    });

    rx
}

impl Executor for NodeByLabelScanExecutor {
    fn open(&self, ctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError> {
        let schema = self.schema.clone();
        let label = self.label.clone();
        let input = self.input.clone();
        let output_mapping = self.output_mapping.clone();

        let stream = try_stream! {
            // For unresolved labels, no nodes can match — yield nothing
            let label_id = match &label {
                IrToken::Resolved { token, .. } => *token,
                IrToken::Unresolved(_) => return,
            };

            if let Some(input) = input {
                let mapping = output_mapping.unwrap();
                let mut out_builder = DataChunkBuilder::new(
                    schema.columns().iter().map(|col| col.typ.clone()),
                    CHUNK_SIZE
                );

                let input_stream = input.open(ctx.clone())?;
                futures::pin_mut!(input_stream);

                while let Some(input_chunk_result) = input_stream.next().await {
                    let input_chunk = input_chunk_result?.compact();

                    let mut rx = scan_nodes_by_label(&ctx, label_id);
                    while let Some(node_chunk_result) = rx.recv().await {
                        let node_chunk = node_chunk_result?.compact();

                        for input_row in input_chunk.iter() {
                            for node_row in node_chunk.iter() {
                                let mut output_row = Vec::with_capacity(mapping.len());
                                for source in &mapping {
                                    let val = match source {
                                        OutputColumnSource::Left(idx) => input_row.get(*idx).cloned().flatten(),
                                        OutputColumnSource::Right(idx) => node_row.get(*idx).cloned().flatten(),
                                    };
                                    output_row.push(val);
                                }
                                if let Some(chunk) = out_builder.append_row(output_row) {
                                    yield chunk;
                                }
                            }
                        }
                    }
                }

                if let Some(chunk) = out_builder.yield_chunk() {
                    yield chunk;
                }
            } else {
                let mut rx = scan_nodes_by_label(&ctx, label_id);
                while let Some(chunk_result) = rx.recv().await {
                    yield chunk_result?;
                }
            }
        }
        .boxed();

        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "NodeByLabelScan"
    }
}
