pub mod manager;

use std::sync::{Arc, Mutex};

use elio_common::catalog::{ConstraintCatalogEntry, ConstraintKind, IndexCatalogEntry, IndexHint, IndexKind};
use elio_common::{LabelId, PropertyKeyId};

use crate::catalog::codec::{ConstraintCodec, IndexCodec};
use crate::catalog::{CatalogChange, DurableCatalogSnapshot};
use crate::error::GraphStoreError;
use crate::kv::cf_catalog;
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

    // ==================== Catalog snapshot writes ====================

    /// Store a constraint (buffered in write batch, applied on commit).
    /// For Unique/NodeKey constraints, a backing index is automatically created.
    pub fn put_constraint(&self, meta: &ConstraintCatalogEntry) -> Result<(), GraphStoreError> {
        let cf = self.snapshot.db().cf_handle(cf_catalog::CF_NAME).unwrap();
        let mut guard = self.write_state.lock().unwrap();

        // Create backing index for Unique/NodeKey constraints
        let mut meta = meta.clone();
        if matches!(meta.constraint_kind, ConstraintKind::Unique | ConstraintKind::NodeKey) {
            let index_name: Arc<str> = Arc::from(format!("idx_{}", meta.name));
            let index_entry = IndexCatalogEntry {
                name: index_name.clone(),
                label_id: meta.label_or_rel_id,
                index_kind: IndexKind::Unique,
                property_key_ids: meta.property_key_ids.clone(),
            };

            // Persist index to RocksDB
            let index_key = IndexCodec::encode_meta_key(&index_entry.name);
            let index_value = IndexCodec::encode_meta_value(&index_entry);
            guard.batch.put_cf(&cf, &index_key, &index_value);
            guard.catalog_changes.push(CatalogChange::UpsertIndex(index_entry));

            meta.backing_index = Some(index_name);
        }

        // Store constraint metadata
        let meta_key = ConstraintCodec::encode_meta_key(&meta.name);
        let meta_value = ConstraintCodec::encode_meta_value(&meta);
        guard.batch.put_cf(&cf, &meta_key, &meta_value);

        // Store label-to-constraint mapping
        let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_or_rel_id, &meta.name);
        guard.batch.put_cf(&cf, &label_key, []);
        guard.catalog_changes.push(CatalogChange::UpsertConstraint(meta));

        Ok(())
    }

    /// Delete a constraint (buffered in write batch, applied on commit).
    /// Also deletes the backing index if present.
    pub fn delete_constraint(&self, name: &str) -> Result<(), GraphStoreError> {
        let cf = self.snapshot.db().cf_handle(cf_catalog::CF_NAME).unwrap();

        // Get the constraint from transaction's catalog snapshot
        if let Some(meta) = self.catalog_snapshot.get_constraint(name) {
            let mut guard = self.write_state.lock().unwrap();

            // Delete backing index if present
            if let Some(ref index_name) = meta.backing_index {
                let index_key = IndexCodec::encode_meta_key(index_name);
                guard.batch.delete_cf(&cf, &index_key);
                guard.catalog_changes.push(CatalogChange::DeleteIndex {
                    name: index_name.clone(),
                    label_id: meta.label_or_rel_id,
                });
            }

            // Delete label-to-constraint mapping
            let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_or_rel_id, name);
            guard.batch.delete_cf(&cf, &label_key);

            // Delete constraint metadata
            let meta_key = ConstraintCodec::encode_meta_key(name);
            guard.batch.delete_cf(&cf, &meta_key);
            guard.catalog_changes.push(CatalogChange::DeleteConstraint {
                name: meta.name,
                label_id: meta.label_or_rel_id,
            });
        }

        Ok(())
    }
}
