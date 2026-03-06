use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::Instant;

use elio_common::array::chunk::DataChunk;
use elio_common::schema::Schema;
use futures::Stream;
use pretty_xmlish::{Pretty, PrettyConfig, XmlNode};

use crate::execution::QueryContext;
use crate::execution::error::ExecError;
use crate::execution::executor::{DataChunkStream, Executor, SharedExecutor};

/// Runtime statistics collected for a single operator.
#[derive(Debug)]
pub struct OperatorMetrics {
    pub name: &'static str,
    /// Total rows output by this operator.
    pub rows: AtomicU64,
    /// Wall-clock time in nanoseconds spent inside this operator's `poll_next`,
    /// **including** time spent in children. Self-time is computed at display time
    /// as `elapsed_ns - sum(children.elapsed_ns)`.
    pub elapsed_ns: AtomicU64,
    pub children: Vec<Arc<OperatorMetrics>>,
}

impl OperatorMetrics {
    pub fn new(name: &'static str, children: Vec<Arc<OperatorMetrics>>) -> Self {
        Self {
            name,
            rows: AtomicU64::new(0),
            elapsed_ns: AtomicU64::new(0),
            children,
        }
    }

    /// Format the profile tree as a pretty-printed string using `pretty_xmlish`.
    pub fn format_tree(&self) -> String {
        let xml = self.to_xmlnode();
        let record = Pretty::Record(xml);
        let mut config = PrettyConfig {
            indent: 3,
            width: 2048,
            need_boundaries: false,
            reduced_spaces: true,
        };
        let mut output = String::with_capacity(2048);
        config.unicode(&mut output, &record);
        output
    }

    fn to_xmlnode(&self) -> XmlNode<'_> {
        let total_ns = self.elapsed_ns.load(Ordering::Relaxed);
        let children_ns: u64 = self.children.iter().map(|c| c.elapsed_ns.load(Ordering::Relaxed)).sum();
        let self_ns = total_ns.saturating_sub(children_ns);
        let rows = self.rows.load(Ordering::Relaxed);

        let fields: Vec<(&str, Pretty<'_>)> = vec![
            ("rows", Pretty::display(&rows)),
            ("time", Pretty::display(&format_duration_ns(self_ns))),
        ];

        let children: Vec<Pretty<'_>> = self.children.iter().map(|c| Pretty::Record(c.to_xmlnode())).collect();

        XmlNode::simple_record(self.name, fields, children)
    }
}

fn format_duration_ns(ns: u64) -> String {
    format!("{:.3}ms", ns as f64 / 1_000_000.0)
}

/// A decorator executor that collects profiling metrics.
#[derive(Debug)]
pub struct ProfiledExecutor {
    pub inner: SharedExecutor,
    pub metrics: Arc<OperatorMetrics>,
}

impl Executor for ProfiledExecutor {
    fn open(&self, qctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError> {
        let inner_stream = self.inner.open(qctx)?;
        let metrics = self.metrics.clone();
        Ok(Box::pin(ProfiledStream {
            inner: inner_stream,
            metrics,
        }))
    }

    fn schema(&self) -> &Schema {
        self.inner.schema()
    }

    fn name(&self) -> &'static str {
        self.inner.name()
    }
}

/// A stream wrapper that measures time spent in `poll_next` and counts rows.
struct ProfiledStream {
    inner: DataChunkStream,
    metrics: Arc<OperatorMetrics>,
}

impl Stream for ProfiledStream {
    type Item = Result<DataChunk, ExecError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let start = Instant::now();
        // SAFETY: we never move inner out of self
        let inner = unsafe { self.as_mut().map_unchecked_mut(|s| &mut s.inner) };
        let result = inner.poll_next(cx);
        let elapsed = start.elapsed().as_nanos() as u64;

        if let Poll::Ready(Some(Ok(ref chunk))) = result {
            self.metrics
                .rows
                .fetch_add(chunk.visible_row_len() as u64, Ordering::Relaxed);
            self.metrics.elapsed_ns.fetch_add(elapsed, Ordering::Relaxed);
        } else if let Poll::Ready(None) = result {
            // stream done, accumulate final time
            self.metrics.elapsed_ns.fetch_add(elapsed, Ordering::Relaxed);
        }

        result
    }
}
