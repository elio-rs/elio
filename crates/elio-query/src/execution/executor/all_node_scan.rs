use std::sync::Arc;

use async_stream::try_stream;
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
pub struct AllNodeScanExectuor {
    pub schema: Arc<Schema>,
    pub input: Option<SharedExecutor>,
    pub output_mapping: Option<Vec<OutputColumnSource>>,
}

impl AllNodeScanExectuor {
    pub fn new(schema: Arc<Schema>) -> Self {
        Self {
            schema,
            input: None,
            output_mapping: None,
        }
    }

    pub fn with_input(schema: Arc<Schema>, input: SharedExecutor, output_mapping: Vec<OutputColumnSource>) -> Self {
        Self {
            schema,
            input: Some(input),
            output_mapping: Some(output_mapping),
        }
    }
}

/// Scan all nodes from storage, returns a receiver for node chunks
fn scan_nodes(ctx: &Arc<TaskExecContext>) -> mpsc::Receiver<Result<DataChunk, ExecError>> {
    let (tx, rx) = mpsc::channel::<Result<DataChunk, ExecError>>(CHANNEL_BUFFER_SIZE);
    let txn = ctx.tx().clone();
    let graph_store = ctx.graph_store().clone();

    tokio::task::spawn_blocking(move || {
        let opts = NodeScanOptions { batch_size: 1024 };
        let mut iter = match graph_store.node_scan(&txn, opts) {
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

impl Executor for AllNodeScanExectuor {
    fn open(&self, ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let schema = self.schema.clone();
        let input = self.input.clone();
        let output_mapping = self.output_mapping.clone();

        let stream = try_stream! {
            if let Some(input) = input {
                // with input: for each input chunk, rescan all nodes
                let mapping = output_mapping.unwrap();
                let mut out_builder = DataChunkBuilder::new(
                    schema.columns().iter().map(|col| col.typ.physical_type()),
                    CHUNK_SIZE
                );

                let input_stream = input.open(ctx.clone())?;
                futures::pin_mut!(input_stream);

                while let Some(input_chunk_result) = input_stream.next().await {
                    let input_chunk = input_chunk_result?.compact();

                    // rescan nodes for this input chunk
                    let mut rx = scan_nodes(&ctx);
                    while let Some(node_chunk_result) = rx.recv().await {
                        let node_chunk = node_chunk_result?.compact();

                        // cross product: input_chunk × node_chunk
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
                // no input: just scan and yield
                let mut rx = scan_nodes(&ctx);
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
        "AllNodeScan"
    }
}
