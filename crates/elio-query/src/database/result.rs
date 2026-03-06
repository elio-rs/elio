use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use async_stream::stream;
use elio_common::array::DataChunk;
use elio_common::scalar::{Row, ScalarValue};
use futures::Stream;
use futures::stream::BoxStream;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::database::error::Error;
use crate::execution::error::ExecError;
use crate::execution::profile::OperatorMetrics;

/// Query Result Handle.
/// Result handle contains three parts:
/// 1. meta data: query static info(static, resolved after planning):
///     - query execution kind
///     - columns
/// 2. meta data: query execution info(dynamic, resolved after execution):
///     - query statistics
///     - execution plan
///     - gql status object
///     - notifications
/// 3. data: query result data(if any)
/// 4. listeners
///     - on query failed
///     - on query finished
///     - ...
///
/// ResultHandle communicate with execution engine with QueryExecutionHandle object
pub trait ResultHandle: Stream<Item = Result<Row, Error>> + Send {
    fn columns(&self) -> &[String];

    /// Returns profile information after the data stream is fully consumed.
    /// Only available for PROFILE statements.
    fn profile(&self) -> Option<String> {
        None
    }
}

pub struct TaskHandleBridge {
    pub stream: BoxStream<'static, Result<Row, Error>>,
    pub columns: Vec<String>,
}

impl TaskHandleBridge {
    pub fn new(columns: Vec<String>, mut data: UnboundedReceiver<Result<DataChunk, ExecError>>) -> Self {
        let s = Box::pin(stream! {
            while let Some(msg) = data.recv().await {
                match msg {
                    Ok(chunk) => {
                        for row_ref in chunk.iter() {
                            let row =
                            row_ref.into_iter().map(|x| x.map(|y| y.to_owned_scalar())).collect::<Row>();
                            yield Ok(row)
                        }
                    }
                    Err(e) =>{
                        yield Err(e.into())
                    }
                }
            }
        });

        Self { stream: s, columns }
    }
}

impl Stream for TaskHandleBridge {
    type Item = Result<Row, Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.stream.as_mut().poll_next(cx)
    }
}

impl ResultHandle for TaskHandleBridge {
    fn columns(&self) -> &[String] {
        &self.columns
    }
}

/// Empty result handle for DDL statements
pub struct EmptyResultHandle {
    columns: Vec<String>,
    done: bool,
}

impl EmptyResultHandle {
    pub fn new(columns: Vec<String>) -> Self {
        Self { columns, done: false }
    }
}

impl Stream for EmptyResultHandle {
    type Item = Result<Row, Error>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            Poll::Ready(None)
        } else {
            self.done = true;
            // Return a single row with "OK" message
            let row = vec![Some(ScalarValue::String("Constraint created/dropped".into()))];
            Poll::Ready(Some(Ok(row)))
        }
    }
}

impl ResultHandle for EmptyResultHandle {
    fn columns(&self) -> &[String] {
        &self.columns
    }
}

/// Result handle for EXPLAIN statements
pub struct ExplainResultHandle {
    columns: Vec<String>,
    explain: String,
    done: bool,
}

impl ExplainResultHandle {
    pub fn new(explain: String) -> Self {
        Self {
            columns: vec!["plan".to_string()],
            explain,
            done: false,
        }
    }
}

impl Stream for ExplainResultHandle {
    type Item = Result<Row, Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.done {
            std::task::Poll::Ready(None)
        } else {
            self.done = true;
            let row = vec![Some(ScalarValue::String(std::mem::take(&mut self.explain)))];
            std::task::Poll::Ready(Some(Ok(row)))
        }
    }
}

impl ResultHandle for ExplainResultHandle {
    fn columns(&self) -> &[String] {
        &self.columns
    }
}

/// Result handle for PROFILE statements.
/// Streams normal query results; profile statistics are available as metadata
/// via `ResultHandle::profile()` after the data stream is consumed.
pub struct ProfileResultHandle {
    inner: TaskHandleBridge,
    metrics: Arc<OperatorMetrics>,
}

impl ProfileResultHandle {
    pub fn new(
        columns: Vec<String>,
        data: UnboundedReceiver<Result<DataChunk, ExecError>>,
        metrics: Arc<OperatorMetrics>,
    ) -> Self {
        Self {
            inner: TaskHandleBridge::new(columns, data),
            metrics,
        }
    }
}

impl Stream for ProfileResultHandle {
    type Item = Result<Row, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.stream.as_mut().poll_next(cx)
    }
}

impl ResultHandle for ProfileResultHandle {
    fn columns(&self) -> &[String] {
        &self.inner.columns
    }

    fn profile(&self) -> Option<String> {
        Some(self.metrics.format_tree())
    }
}
