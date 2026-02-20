use std::sync::Arc;

use parking_lot::RwLock;

use crate::catalog::CatalogStore;
use crate::kv::KvEngine;
use crate::transaction::{CatalogCommitFn, Transaction, WriteState};

pub enum TransactionMode {
    ReadOnly,
    ReadWrite,
}

// global transaction lock guard, this is used to enforce single-writer
// Fields are held for RAII — lock releases when guard drops
#[allow(dead_code)]
pub(crate) enum TxGuard {
    Read(parking_lot::RwLockReadGuard<'static, ()>),
    Write(parking_lot::RwLockWriteGuard<'static, ()>),
}

// #safety
//   the underlying lock is held by the guard and lives as long as TxGuard
unsafe impl Send for TxGuard {}
unsafe impl Sync for TxGuard {}

pub(crate) struct OwnedSnapshot {
    // snapshot is static, so we need to keep the db alive
    pub(crate) _db: Arc<KvEngine>,
    pub(crate) snapshot: rocksdb::Snapshot<'static>,
}

impl OwnedSnapshot {
    pub fn new(db: Arc<KvEngine>) -> Self {
        unsafe {
            let snapshot = db.snapshot();
            let static_snapshot: rocksdb::Snapshot<'static> = std::mem::transmute(snapshot);
            Self {
                _db: db,
                snapshot: static_snapshot,
            }
        }
    }

    pub fn db(&self) -> &Arc<KvEngine> {
        &self._db
    }
}

impl std::ops::Deref for OwnedSnapshot {
    type Target = rocksdb::Snapshot<'static>;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

pub struct TransactionManager {
    db: Arc<KvEngine>,
    // used to apply changes to inmemory catalog snapshot
    catalog_store: Arc<CatalogStore>,
    // the global transaction lock, this is used to enforce single-writer
    // TODO(pgao): we should have a mutext here, reads should not hold locks.
    tx_lock: RwLock<()>,
}

impl TransactionManager {
    pub fn new(db: Arc<KvEngine>, catalog_store: Arc<CatalogStore>) -> Self {
        Self {
            db,
            catalog_store,
            tx_lock: RwLock::new(()),
        }
    }

    pub fn begin(&self, mode: TransactionMode) -> Arc<Transaction> {
        let tx_guard = match mode {
            TransactionMode::ReadOnly => {
                let guard = self.tx_lock.read();
                // SAFETY:
                //    tx_lock will live longer than the guard
                let guard: parking_lot::RwLockReadGuard<'static, ()> = unsafe { std::mem::transmute(guard) };
                TxGuard::Read(guard)
            }
            TransactionMode::ReadWrite => {
                let guard = self.tx_lock.write();
                // SAFETY:
                //    tx_lock will live longer than the guard
                let guard: parking_lot::RwLockWriteGuard<'static, ()> = unsafe { std::mem::transmute(guard) };
                TxGuard::Write(guard)
            }
        };
        let catalog_snapshot = self.catalog_store.current_snapshot();
        let catalog_store = self.catalog_store.clone();
        let on_catalog_commit: CatalogCommitFn = Arc::new(move |changes| catalog_store.publish_changes(changes));
        Arc::new(Transaction {
            snapshot: OwnedSnapshot::new(self.db.clone()),
            catalog_snapshot,
            write_state: std::sync::Mutex::new(WriteState::default()),
            on_catalog_commit,
            tx_guard,
        })
    }
}
