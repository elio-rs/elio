pub mod codec;

use std::sync::Arc;

use elio_common::catalog::{ConstraintCatalogEntry, ConstraintKind, IndexHint};
use elio_common::{LabelId, NodeId, PropertyKeyId};

pub use self::codec::{ConstraintCodec, UniqueIndexCodec};
use crate::error::GraphStoreError;
use crate::kv::{KvEngine, cf_catalog};
use crate::transaction::Transaction;

/// Catalog store:
///  - constraints
pub struct CatalogStore {
    db: Arc<KvEngine>,
}

impl CatalogStore {
    pub fn new(db: Arc<KvEngine>) -> Self {
        Self { db }
    }

    // ==================== Constraint Metadata ====================

    /// Check if a constraint exists (reads from transaction snapshot)
    pub fn constraint_exists(&self, tx: &Transaction, name: &str) -> Result<bool, GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();
        let key = ConstraintCodec::encode_meta_key(name);
        Ok(tx.snapshot.get_cf(&cf, &key)?.is_some())
    }

    /// Get constraint metadata by name (reads from transaction snapshot)
    pub fn get_constraint(
        &self,
        tx: &Transaction,
        name: &str,
    ) -> Result<Option<ConstraintCatalogEntry>, GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();
        let key = ConstraintCodec::encode_meta_key(name);
        match tx.snapshot.get_cf(&cf, &key)? {
            Some(value) => Ok(ConstraintCodec::decode_meta_value(name.to_string(), &value)),
            None => Ok(None),
        }
    }

    /// Get all constraints for a label (reads from transaction snapshot)
    pub fn get_constraints_for_label(
        &self,
        tx: &Transaction,
        label_id: LabelId,
    ) -> Result<Vec<ConstraintCatalogEntry>, GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();
        let prefix = ConstraintCodec::encode_label_constraint_prefix(label_id);

        let mut constraints = Vec::new();
        let mut readopts = rocksdb::ReadOptions::default();
        readopts.set_prefix_same_as_start(true);
        let mode = rocksdb::IteratorMode::From(&prefix, rocksdb::Direction::Forward);
        let iter = tx.snapshot.iterator_cf_opt(&cf, readopts, mode);

        for item in iter {
            let (key, _) = item?;
            if !key.starts_with(&prefix) {
                break;
            }

            // Extract constraint name from the key
            let name_len_offset = 3; // prefix (1) + label_id (2)
            if key.len() < name_len_offset + 2 {
                continue;
            }
            let name_len = u16::from_le_bytes([key[name_len_offset], key[name_len_offset + 1]]) as usize;
            if key.len() < name_len_offset + 2 + name_len {
                continue;
            }
            let name = String::from_utf8_lossy(&key[name_len_offset + 2..name_len_offset + 2 + name_len]).to_string();

            // Get the full constraint metadata
            if let Some(meta) = self.get_constraint(tx, &name)? {
                constraints.push(meta);
            }
        }

        Ok(constraints)
    }

    /// Store a constraint (buffered in transaction write batch)
    pub fn put_constraint(&self, tx: &Transaction, meta: &ConstraintCatalogEntry) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();
        let mut guard = tx.write_state.lock().unwrap();

        // Store metadata
        let meta_key = ConstraintCodec::encode_meta_key(&meta.name);
        let meta_value = ConstraintCodec::encode_meta_value(meta);
        guard.batch.put_cf(&cf, &meta_key, &meta_value);

        // Store label-to-constraint mapping
        let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_or_rel_id, &meta.name);
        guard.batch.put_cf(&cf, &label_key, []);

        Ok(())
    }

    /// Delete a constraint (buffered in transaction write batch)
    pub fn delete_constraint(&self, tx: &Transaction, name: &str) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();

        // Get the constraint first to find the label_id
        if let Some(meta) = self.get_constraint(tx, name)? {
            let mut guard = tx.write_state.lock().unwrap();

            // Delete label-to-constraint mapping
            let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_or_rel_id, name);
            guard.batch.delete_cf(&cf, &label_key);

            // Delete metadata
            let meta_key = ConstraintCodec::encode_meta_key(name);
            guard.batch.delete_cf(&cf, &meta_key);
        }

        Ok(())
    }

    pub fn find_unique_index(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        property_key_ids: &[PropertyKeyId],
    ) -> Result<Option<IndexHint>, GraphStoreError> {
        let constraints = self.get_constraints_for_label(tx, label_id)?;
        for constraint in constraints {
            if matches!(
                constraint.constraint_kind,
                ConstraintKind::Unique | ConstraintKind::NodeKey
            ) {
                // Check if all constraint properties are in the requested set
                // The constraint properties must be a subset of the filter properties
                // and the filter must have all constraint properties
                if constraint.property_key_ids.len() <= property_key_ids.len()
                    && constraint.property_key_ids.iter().all(|p| property_key_ids.contains(p))
                {
                    return Ok(Some(IndexHint {
                        constraint_name: constraint.name,
                        label_id: constraint.label_or_rel_id,
                        property_key_ids: constraint.property_key_ids,
                    }));
                }
            }
        }
        Ok(None)
    }

    // ==================== Unique Index Operations ====================

    /// Check if a unique index entry exists (reads from transaction snapshot)
    pub fn unique_index_exists(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<bool, GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        Ok(tx.snapshot.get_cf(&cf, &key)?.is_some())
    }

    /// Get node_id from unique index (reads from transaction snapshot)
    pub fn get_unique_index(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<Option<NodeId>, GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        match tx.snapshot.get_cf(&cf, &key)? {
            Some(value) => Ok(UniqueIndexCodec::decode_value(&value)),
            None => Ok(None),
        }
    }

    /// Put unique index entry (buffered in transaction write batch)
    pub fn put_unique_index(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
        node_id: NodeId,
    ) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        let value = UniqueIndexCodec::encode_value(node_id);

        let mut guard = tx.write_state.lock().unwrap();
        guard.batch.put_cf(&cf, &key, &value);
        Ok(())
    }

    /// Delete unique index entry (buffered in transaction write batch)
    pub fn delete_unique_index(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_catalog::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);

        let mut guard = tx.write_state.lock().unwrap();
        guard.batch.delete_cf(&cf, &key);
        Ok(())
    }
}
