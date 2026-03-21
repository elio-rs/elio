pub mod manager;

use std::sync::Arc;

use elio_common::catalog::{ConstraintCatalogEntry, ConstraintKind, IndexCatalogEntry, IndexHint, IndexKind};
use elio_common::{LabelId, PropertyKeyId};

use crate::catalog::codec::{ConstraintCodec, IndexCodec};
use crate::catalog::{CatalogChange, DurableCatalogSnapshot};
use crate::error::GraphStoreError;

/// Callback to publish catalog changes on commit.
pub(crate) type CatalogCommitFn = Arc<dyn Fn(Vec<CatalogChange>) + Send + Sync>;

/// The inner transaction handle — either a read-only or read-write LMDB transaction.
///
/// - `RoTxn<'static, WithoutTls>` is `Send` (heed provides this when the env is opened
///   with `read_txn_without_tls`).
/// - `RwTxn<'static>` is `Send` in heed 0.22.
/// - Both are `!Sync`, so we wrap in `Mutex` to serialize access from multiple threads.
pub(crate) enum TxnInner {
    ReadOnly(heed::RoTxn<'static, heed::WithoutTls>),
    ReadWrite(heed::RwTxn<'static>),
}

/// Transaction wrapping a graph env RoTxn or RwTxn.
/// Meta env (tokens/IDs) uses independent short-lived transactions.
pub struct Transaction {
    pub(crate) txn: std::sync::Mutex<Option<TxnInner>>,
    /// Keep the KvEngine alive for the duration of the transaction
    pub(crate) _engine: Arc<crate::kv::KvEngine>,
    pub(crate) catalog_snapshot: Arc<DurableCatalogSnapshot>,
    pub(crate) catalog_changes: std::sync::Mutex<Vec<CatalogChange>>,
    pub(crate) on_catalog_commit: CatalogCommitFn,
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
        let catalog_changes = {
            let mut changes = self.catalog_changes.lock().unwrap();
            std::mem::take(&mut *changes)
        };

        let txn = {
            let mut guard = self.txn.lock().unwrap();
            guard.take().expect("transaction already committed or aborted")
        };
        match txn {
            TxnInner::ReadOnly(ro) => ro.commit().map_err(GraphStoreError::Heed)?,
            TxnInner::ReadWrite(rw) => rw.commit().map_err(GraphStoreError::Heed)?,
        }

        (self.on_catalog_commit)(catalog_changes);
        Ok(())
    }

    pub fn abort(&self) -> Result<(), GraphStoreError> {
        let txn = {
            let mut guard = self.txn.lock().unwrap();
            guard.take().expect("transaction already committed or aborted")
        };
        match txn {
            TxnInner::ReadOnly(_ro) => { /* RoTxn aborts on drop */ }
            TxnInner::ReadWrite(rw) => rw.abort(),
        }
        Ok(())
    }

    /// Access the transaction for reading. Works with both RoTxn and RwTxn (via Deref).
    /// Panics if the transaction has already been committed/aborted.
    pub(crate) fn with_rw_txn<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&heed::RoTxn<'_>) -> R,
    {
        let guard = self.txn.lock().unwrap();
        let txn = guard.as_ref().expect("transaction already committed or aborted");
        match txn {
            TxnInner::ReadOnly(ro) => f(ro),
            TxnInner::ReadWrite(rw) => f(rw),
        }
    }

    /// Access the RwTxn mutably for writes.
    /// Panics if the transaction is read-only or already committed/aborted.
    pub(crate) fn with_rw_txn_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut heed::RwTxn<'_>) -> R,
    {
        let mut guard = self.txn.lock().unwrap();
        let txn = guard.as_mut().expect("transaction already committed or aborted");
        match txn {
            TxnInner::ReadWrite(rw) => f(rw),
            TxnInner::ReadOnly(_) => panic!("cannot write in a read-only transaction"),
        }
    }

    // ==================== Catalog snapshot reads ====================

    pub fn constraint_exists(&self, name: &str) -> bool {
        self.catalog_snapshot.constraint_exists(name)
    }

    pub fn get_constraint(&self, name: &str) -> Option<ConstraintCatalogEntry> {
        self.catalog_snapshot.get_constraint(name)
    }

    pub fn get_constraints_for_label(&self, label_id: LabelId) -> Vec<ConstraintCatalogEntry> {
        self.catalog_snapshot.get_constraints_for_label(label_id)
    }

    pub fn find_unique_index(&self, label_id: LabelId, property_key_ids: &[PropertyKeyId]) -> Option<IndexHint> {
        self.catalog_snapshot.find_unique_index(label_id, property_key_ids)
    }

    // ==================== Catalog writes ====================

    pub fn put_constraint(&self, meta: &ConstraintCatalogEntry) -> Result<(), GraphStoreError> {
        self.with_rw_txn_mut(|txn| {
            let engine = &self._engine;
            let catalog_db = engine.graph.catalog;

            let mut meta = meta.clone();
            if matches!(meta.constraint_kind, ConstraintKind::Unique | ConstraintKind::NodeKey) {
                let index_name: Arc<str> = Arc::from(format!("idx_{}", meta.name));
                let index_entry = IndexCatalogEntry {
                    name: index_name.clone(),
                    label_id: meta.label_or_rel_id,
                    index_kind: IndexKind::Unique,
                    property_key_ids: meta.property_key_ids.clone(),
                };

                let index_key = IndexCodec::encode_meta_key(&index_entry.name);
                let index_value = IndexCodec::encode_meta_value(&index_entry);
                catalog_db
                    .put(txn, &index_key, &index_value)
                    .map_err(GraphStoreError::Heed)?;

                let mut changes = self.catalog_changes.lock().unwrap();
                changes.push(CatalogChange::UpsertIndex(index_entry));

                meta.backing_index = Some(index_name);
            }

            let meta_key = ConstraintCodec::encode_meta_key(&meta.name);
            let meta_value = ConstraintCodec::encode_meta_value(&meta);
            catalog_db
                .put(txn, &meta_key, &meta_value)
                .map_err(GraphStoreError::Heed)?;

            let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_or_rel_id, &meta.name);
            catalog_db.put(txn, &label_key, &[]).map_err(GraphStoreError::Heed)?;

            let mut changes = self.catalog_changes.lock().unwrap();
            changes.push(CatalogChange::UpsertConstraint(meta));

            Ok(())
        })
    }

    pub fn delete_constraint(&self, name: &str) -> Result<(), GraphStoreError> {
        if let Some(meta) = self.catalog_snapshot.get_constraint(name) {
            self.with_rw_txn_mut(|txn| {
                let catalog_db = self._engine.graph.catalog;

                if let Some(ref index_name) = meta.backing_index {
                    let index_key = IndexCodec::encode_meta_key(index_name);
                    catalog_db.delete(txn, &index_key).map_err(GraphStoreError::Heed)?;
                    let mut changes = self.catalog_changes.lock().unwrap();
                    changes.push(CatalogChange::DeleteIndex {
                        name: index_name.clone(),
                        label_id: meta.label_or_rel_id,
                    });
                }

                let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_or_rel_id, name);
                catalog_db.delete(txn, &label_key).map_err(GraphStoreError::Heed)?;

                let meta_key = ConstraintCodec::encode_meta_key(name);
                catalog_db.delete(txn, &meta_key).map_err(GraphStoreError::Heed)?;

                let mut changes = self.catalog_changes.lock().unwrap();
                changes.push(CatalogChange::DeleteConstraint {
                    name: meta.name,
                    label_id: meta.label_or_rel_id,
                });

                Ok(())
            })
        } else {
            Ok(())
        }
    }
}
