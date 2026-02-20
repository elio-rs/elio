pub mod manager;

use std::sync::{Arc, Mutex};

use crate::catalog::{CatalogChange, DurableCatalogSnapshot};
use crate::error::GraphStoreError;
use crate::transaction::manager::{OwnedSnapshot, TxGuard};

/// Callback to publish catalog changes on commit.
pub(crate) type CatalogCommitFn = Arc<dyn Fn(Vec<CatalogChange>) + Send + Sync>;

/// Thin transaction — isolation context only.
/// Contains snapshot for reads, write batch for writes, and lock guard for concurrency.
/// NO data access methods. Data operations are on GraphStore/ConstraintStore.
pub struct Transaction {
    pub(crate) snapshot: OwnedSnapshot,
    pub(crate) catalog_snapshot: Arc<DurableCatalogSnapshot>,
    pub(crate) write_state: Mutex<WriteState>,
    // used to update catalog in-memory cache
    pub(crate) on_catalog_commit: CatalogCommitFn,
    // holds global tx lock guard to enforce single-writer
    // RAII, lock will release when dropped
    #[allow(unused)]
    pub(crate) tx_guard: TxGuard,
}

#[derive(Default)]
pub struct WriteState {
    pub(crate) batch: rocksdb::WriteBatchWithTransaction<false>,
    pub(crate) catalog_changes: Vec<CatalogChange>,
}

pub struct NodeScanOptions {
    pub batch_size: usize,
}

pub struct RelScanOptions {}

#[async_trait::async_trait]
pub trait DataChunkIterator: Send {
    fn next_batch(&mut self) -> Result<Option<elio_common::array::chunk::DataChunk>, GraphStoreError>;
}

impl Transaction {
    pub fn commit(&self) -> Result<(), GraphStoreError> {
        let mut state = self.write_state.lock().unwrap();
        let batch = std::mem::take(&mut state.batch);
        let catalog_changes = std::mem::take(&mut state.catalog_changes);
        drop(state);

        self.snapshot.db().write(batch)?;
        (self.on_catalog_commit)(catalog_changes);
        Ok(())
    }

    pub fn abort(&self) -> Result<(), GraphStoreError> {
        let mut state = self.write_state.lock().unwrap();
        state.batch.clear();
        state.catalog_changes.clear();
        Ok(())
    }
}
