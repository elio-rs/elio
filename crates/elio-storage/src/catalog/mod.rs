pub mod codec;
mod snapshot;

use std::sync::Arc;

use arc_swap::ArcSwap;
use elio_common::catalog::{ConstraintCatalogEntry, ConstraintKind, IndexCatalogEntry, IndexHint, IndexKind};
use elio_common::{LabelId, PropertyKeyId};
use tracing::warn;

pub use self::codec::{ConstraintCodec, IndexCodec};
pub(crate) use self::snapshot::{CatalogChange, DurableCatalogSnapshot};
use crate::error::GraphStoreError;
use crate::kv::{KvEngine, cf_catalog};
use crate::transaction::Transaction;

/// Catalog store:
///  - constraints
/// Note: CatalogStore have states: i.e. the global latest snapshot
/// So catalogstore should be kept as a singleton in database instance.
pub struct CatalogStore {
    // rocksdb handle
    db: Arc<KvEngine>,
    // in memory snapshot for fast read
    snapshot: ArcSwap<DurableCatalogSnapshot>,
}

impl CatalogStore {
    pub fn new(db: Arc<KvEngine>) -> Result<Self, GraphStoreError> {
        let snapshot = Arc::new(Self::load_snapshot_from_db(db.as_ref())?);
        Ok(Self {
            db,
            snapshot: ArcSwap::from(snapshot),
        })
    }

    /// Returns the current catalog snapshot for use in transaction isolation.
    pub(crate) fn current_snapshot(&self) -> Arc<DurableCatalogSnapshot> {
        self.snapshot.load_full()
    }

    // ==================== Constraint Metadata ====================

    /// Check if a constraint exists (reads from transaction's catalog snapshot)
    pub fn constraint_exists(&self, tx: &Transaction, name: &str) -> Result<bool, GraphStoreError> {
        Ok(tx.catalog_snapshot.constraint_exists(name))
    }

    /// Get constraint metadata by name (reads from transaction's catalog snapshot)
    pub fn get_constraint(
        &self,
        tx: &Transaction,
        name: &str,
    ) -> Result<Option<ConstraintCatalogEntry>, GraphStoreError> {
        Ok(tx.catalog_snapshot.get_constraint(name))
    }

    /// Get all constraints for a label (reads from transaction's catalog snapshot)
    pub fn get_constraints_for_label(
        &self,
        tx: &Transaction,
        label_id: LabelId,
    ) -> Result<Vec<ConstraintCatalogEntry>, GraphStoreError> {
        Ok(tx.catalog_snapshot.get_constraints_for_label(label_id))
    }

    /// Get index metadata by name (reads from transaction's catalog snapshot)
    pub fn get_index(&self, tx: &Transaction, name: &str) -> Result<Option<IndexCatalogEntry>, GraphStoreError> {
        Ok(tx.catalog_snapshot.get_index(name))
    }

    /// Get all indexes for a label (reads from transaction's catalog snapshot)
    pub fn get_indexes_for_label(
        &self,
        tx: &Transaction,
        label_id: LabelId,
    ) -> Result<Vec<IndexCatalogEntry>, GraphStoreError> {
        Ok(tx.catalog_snapshot.get_indexes_for_label(label_id))
    }

    /// Store a constraint (buffered in transaction write batch).
    /// For Unique/NodeKey constraints, a backing index is automatically created.
    pub fn put_constraint(&self, tx: &Transaction, meta: &ConstraintCatalogEntry) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();
        let mut guard = tx.write_state.lock().unwrap();

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

    /// Delete a constraint (buffered in transaction write batch).
    /// Also deletes the backing index if present.
    pub fn delete_constraint(&self, tx: &Transaction, name: &str) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();

        // Get the constraint from transaction's catalog snapshot
        if let Some(meta) = tx.catalog_snapshot.get_constraint(name) {
            let mut guard = tx.write_state.lock().unwrap();

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

    pub fn find_unique_index(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        property_key_ids: &[PropertyKeyId],
    ) -> Result<Option<IndexHint>, GraphStoreError> {
        Ok(tx.catalog_snapshot.find_unique_index(label_id, property_key_ids))
    }

    pub(crate) fn publish_changes(&self, changes: Vec<CatalogChange>) {
        if changes.is_empty() {
            return;
        }

        self.snapshot.rcu(|old| Arc::new(old.apply_changes(&changes)));
    }

    fn load_snapshot_from_db(db: &KvEngine) -> Result<DurableCatalogSnapshot, GraphStoreError> {
        let cf = db.cf_handle(cf_catalog::CF_NAME).unwrap();

        // Load constraints
        let mut constraints = Vec::new();
        let constraint_prefix = [cf_catalog::CONSTRAINT_META_PREFIX];
        let constraint_iter = db.prefix_iterator_cf(&cf, constraint_prefix);
        for item in constraint_iter {
            let (key, value) = item?;
            if !key.starts_with(&constraint_prefix) {
                break;
            }

            let Some(name) = ConstraintCodec::decode_meta_key(&key) else {
                warn!("skip invalid constraint meta key while loading snapshot");
                continue;
            };

            let Some(entry) = ConstraintCodec::decode_meta_value(name.clone(), &value) else {
                warn!(constraint_name = %name, "skip invalid constraint meta value while loading snapshot");
                continue;
            };

            constraints.push(entry);
        }

        // Load indexes
        let mut indexes = Vec::new();
        let index_prefix = [cf_catalog::INDEX_META_PREFIX];
        let index_iter = db.prefix_iterator_cf(&cf, index_prefix);
        for item in index_iter {
            let (key, value) = item?;
            if !key.starts_with(&index_prefix) {
                break;
            }

            let Some(name) = IndexCodec::decode_meta_key(&key) else {
                warn!("skip invalid index meta key while loading snapshot");
                continue;
            };

            let Some(entry) = IndexCodec::decode_meta_value(name.clone(), &value) else {
                warn!(index_name = %name, "skip invalid index meta value while loading snapshot");
                continue;
            };

            indexes.push(entry);
        }

        // Backward compatibility: older constraints may not have backing_index set.
        // Synthesize missing index entries and patch constraints with backing_index names.
        let existing_index_names: std::collections::HashSet<Arc<str>> =
            indexes.iter().map(|entry| entry.name.clone()).collect();
        for constraint in &mut constraints {
            if !matches!(
                constraint.constraint_kind,
                ConstraintKind::Unique | ConstraintKind::NodeKey
            ) {
                continue;
            }
            let index_name: Arc<str> = match &constraint.backing_index {
                Some(name) => name.clone(),
                None => {
                    // Legacy constraint without backing_index — assign the new naming convention
                    let name: Arc<str> = Arc::from(format!("idx_{}", constraint.name));
                    constraint.backing_index = Some(name.clone());
                    name
                }
            };
            if !existing_index_names.contains(&index_name) {
                indexes.push(IndexCatalogEntry {
                    name: index_name,
                    label_id: constraint.label_or_rel_id,
                    index_kind: IndexKind::Unique,
                    property_key_ids: constraint.property_key_ids.clone(),
                });
            }
        }

        Ok(DurableCatalogSnapshot::from_entries(constraints, indexes))
    }
}
