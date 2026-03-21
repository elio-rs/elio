use std::sync::Arc;

use crate::catalog::CatalogState;
use crate::kv::KvEngine;
use crate::transaction::{CatalogCommitFn, TxnInner, Transaction};

pub enum TransactionMode {
    ReadOnly,
    ReadWrite,
}

pub struct TransactionManager {
    engine: Arc<KvEngine>,
    catalog_state: Arc<CatalogState>,
}

impl TransactionManager {
    pub fn new(engine: Arc<KvEngine>, catalog_state: Arc<CatalogState>) -> Self {
        Self { engine, catalog_state }
    }

    /// Begin a transaction on the graph env.
    ///
    /// - `ReadOnly`: uses `RoTxn` — multiple concurrent readers allowed, no env-level blocking.
    /// - `ReadWrite`: uses `RwTxn` — single-writer, blocks other writers (but not readers).
    ///
    /// If the map is full on write, automatically resizes and retries.
    pub fn begin(&self, mode: TransactionMode) -> Arc<Transaction> {
        let engine = self.engine.clone();

        // SAFETY: We transmute the txn lifetime to 'static. The KvEngine (and thus the Env)
        // is kept alive by the Arc<KvEngine> stored in the Transaction.
        let txn_inner = match mode {
            TransactionMode::ReadOnly => {
                let ro = engine.graph.env.read_txn().expect("failed to begin read txn");
                // SAFETY: Env outlives the transaction via Arc<KvEngine> in Transaction.
                let static_ro = unsafe {
                    std::mem::transmute::<
                        heed::RoTxn<'_, heed::WithoutTls>,
                        heed::RoTxn<'static, heed::WithoutTls>,
                    >(ro)
                };
                TxnInner::ReadOnly(static_ro)
            }
            TransactionMode::ReadWrite => unsafe {
                match engine.graph.env.write_txn() {
                    Ok(txn) => {
                        TxnInner::ReadWrite(std::mem::transmute::<heed::RwTxn<'_>, heed::RwTxn<'static>>(txn))
                    }
                    Err(heed::Error::Mdb(heed::MdbError::MapFull)) => {
                        engine.graph.try_resize().expect("failed to resize graph env");
                        let txn = engine.graph.env.write_txn().expect("failed to begin write txn after resize");
                        TxnInner::ReadWrite(std::mem::transmute::<heed::RwTxn<'_>, heed::RwTxn<'static>>(txn))
                    }
                    Err(e) => panic!("failed to begin write txn: {e}"),
                }
            },
        };

        let catalog_snapshot = self.catalog_state.current_snapshot();
        let catalog_state = self.catalog_state.clone();
        let on_catalog_commit: CatalogCommitFn = Arc::new(move |changes| catalog_state.publish_changes(changes));

        Arc::new(Transaction {
            txn: std::sync::Mutex::new(Some(txn_inner)),
            _engine: engine,
            catalog_snapshot,
            catalog_changes: std::sync::Mutex::new(Vec::new()),
            on_catalog_commit,
        })
    }
}
