pub mod codec;
mod snapshot;

use std::sync::Arc;

use arc_swap::ArcSwap;
use elio_common::catalog::{ConstraintKind, IndexCatalogEntry, IndexKind};
use tracing::warn;

pub use self::codec::{ConstraintCodec, IndexCodec};
pub(crate) use self::snapshot::{CatalogChange, DurableCatalogSnapshot};
use crate::error::GraphStoreError;
use crate::kv::{KvEngine, cf_catalog};

/// In-memory catalog state, kept as a singleton in the database instance.
/// Manages the current `DurableCatalogSnapshot` — loads it on startup,
/// hands it to new transactions, and advances it on commit.
pub struct CatalogState {
    // in memory snapshot for fast read
    snapshot: ArcSwap<DurableCatalogSnapshot>,
}

impl CatalogState {
    pub fn new(db: Arc<KvEngine>) -> Result<Self, GraphStoreError> {
        let snapshot = Arc::new(Self::load_snapshot_from_db(db.as_ref())?);
        Ok(Self {
            snapshot: ArcSwap::from(snapshot),
        })
    }

    /// Returns the current catalog snapshot for use in transaction isolation.
    pub(crate) fn current_snapshot(&self) -> Arc<DurableCatalogSnapshot> {
        self.snapshot.load_full()
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
