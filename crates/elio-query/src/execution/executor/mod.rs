use std::pin::Pin;
use std::sync::Arc;

use elio_common::array::chunk::DataChunk;
use elio_common::schema::Schema;
use futures::Stream;
use tracing;

use crate::execution::QueryContext;
use crate::execution::error::ExecError;
use crate::execution::task::TaskExecContext;

pub mod all_node_scan;
pub mod apply;
pub mod argument;
pub mod black_hole;
pub mod constraint;
pub mod create_node;
pub mod create_rel;
pub mod cross_product;
pub mod distinct;
pub mod expand;
pub mod filter;
pub mod hash_aggregate;
pub mod load_csv;
pub mod node_index_seek;
pub mod pagination;
pub mod produce_result;
pub mod project;
pub mod relscan;
pub mod unit;
pub mod var_expand;

pub type DataChunkStream = Pin<Box<dyn Stream<Item = Result<DataChunk, ExecError>> + Send>>;

// maybe chunk size should be put to exec ctx as an configuration
pub const CHUNK_SIZE: usize = 4096;

pub trait Executor: Send + Sync + std::fmt::Debug {
    fn open(&self, _ctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError>;

    fn schema(&self) -> &Schema;

    fn into_shared(self) -> SharedExecutor
    where
        Self: Sized + 'static,
    {
        Arc::new(self)
    }

    fn name(&self) -> &'static str;
}

pub type SharedExecutor = Arc<dyn Executor>;
