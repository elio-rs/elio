//! LoadCsvExecutor is a leaf executor that loads data from a CSV file.

use std::fs::File;
use std::io::BufReader;

use async_stream::try_stream;
use elio_common::array::chunk::DataChunkBuilder;
use elio_common::scalar::{ListValue, ScalarValue, StructValue};
use elio_cypher::ir::query_project::CsvLoadFormat;
use futures::StreamExt;
use tokio::sync::mpsc;

use super::*;
use crate::error::ExecError;
use crate::executor::Executor;

const CHANNEL_BUFFER_SIZE: usize = 1024;

#[derive(Debug, Clone)]
pub struct LoadCsvExecutor {
    pub source_url: Arc<str>,
    pub format: CsvLoadFormat,
    pub schema: Arc<Schema>,
}

impl Executor for LoadCsvExecutor {
    fn open(&self, _ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let (tx, mut rx) = mpsc::channel::<Result<DataChunk, ExecError>>(CHANNEL_BUFFER_SIZE);
        let source_url = self.source_url.clone();
        let format = self.format.clone();

        let handle = tokio::task::spawn_blocking(move || {
            if let Err(e) = load_csv_blocking(&source_url, &format, &tx) {
                let _ = tx.blocking_send(Err(e));
            }
        });

        let stream = try_stream! {
            while let Some(item) = rx.recv().await {
                yield item?;
            }
            // Check if the blocking task panicked
            if let Err(e) = handle.await
                && e.is_panic() {
                    Err(ExecError::io_error("load csv task panicked"))?;
                }
        }
        .boxed();

        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "LoadCsv"
    }
}

// NB: this is an blocking implementation, since we are reading from local files.
// If we're going to support remote files via http, we need to use an async implementation.
fn load_csv_blocking(
    source_url: &str,
    format: &CsvLoadFormat,
    tx: &mpsc::Sender<Result<DataChunk, ExecError>>,
) -> Result<(), ExecError> {
    let file = File::open(source_url).map_err(|e| ExecError::io_error(e.to_string()))?;
    let reader = BufReader::new(file);

    let mut csv_reader = csv::ReaderBuilder::new()
        .delimiter(format.delimiter() as u8)
        .has_headers(format.header())
        .from_reader(reader);

    // Get headers if format.header() is true
    let headers: Option<Vec<Arc<str>>> = if format.header() {
        let headers = csv_reader.headers().map_err(|e| ExecError::io_error(e.to_string()))?;
        Some(headers.iter().map(Arc::from).collect())
    } else {
        None
    };

    // only at runtime, we can know the physical type, so, here we just output an Any Array.
    // TODO(pgao): this can be optimized.
    let mut builder = DataChunkBuilder::new(std::iter::once(format.output_type().physical_type()), CHUNK_SIZE);

    for result in csv_reader.records() {
        let record = result.map_err(|e| ExecError::io_error(e.to_string()))?;

        // TODO(pgao): we can avoid the match here
        let value = match &headers {
            Some(h) => build_struct_value(h, &record),
            None => build_list_value(&record),
        };

        if let Some(chunk) = builder.append_row(vec![Some(value.as_scalar_ref())])
            && tx.blocking_send(Ok(chunk)).is_err()
        {
            // Receiver dropped, stop processing
            tracing::warn!("recv dropped, could not send load csv result.");
            return Ok(());
        }
    }

    // Flush remaining rows
    if let Some(chunk) = builder.yield_chunk() {
        let _ = tx.blocking_send(Ok(chunk));
    }

    Ok(())
}

// if csv have header, it will return struct
fn build_struct_value(headers: &[Arc<str>], record: &csv::StringRecord) -> ScalarValue {
    let fields: Vec<(Arc<str>, ScalarValue)> = headers
        .iter()
        .zip(record.iter())
        .map(|(k, v)| (k.clone(), ScalarValue::String(v.to_string())))
        .collect();
    ScalarValue::Struct(Box::new(StructValue::new(fields)))
}

// if csv does not have header, it will return list
fn build_list_value(record: &csv::StringRecord) -> ScalarValue {
    let items: Vec<ScalarValue> = record.iter().map(|v| ScalarValue::String(v.to_string())).collect();
    ScalarValue::List(Box::new(ListValue::new(items)))
}
