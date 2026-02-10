pub mod manager;

use std::sync::Mutex;

use crate::error::GraphStoreError;
use crate::transaction::manager::{OwnedSnapshot, TxGuard};

/// Thin transaction — isolation context only.
/// Contains snapshot for reads, write batch for writes, and lock guard for concurrency.
/// NO data access methods. Data operations are on GraphStore/ConstraintStore.
pub struct Transaction {
    pub(crate) snapshot: OwnedSnapshot,
    pub(crate) write_state: Mutex<WriteState>,
    // holds global tx lock guard to enforce single-writer
    // RAII, lock will release when dropped
    #[allow(unused)]
    pub(crate) tx_guard: TxGuard,
}

#[derive(Default)]
pub struct WriteState {
    pub(crate) batch: rocksdb::WriteBatchWithTransaction<false>,
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
        self.snapshot.db().write(batch)?;
        Ok(())
    }

    pub fn abort(&self) -> Result<(), GraphStoreError> {
        let mut state = self.write_state.lock().unwrap();
        state.batch.clear();
        Ok(())
    }
}
