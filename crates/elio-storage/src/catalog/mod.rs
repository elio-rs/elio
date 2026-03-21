pub mod codec;
mod snapshot;

use std::sync::Arc;

use arc_swap::ArcSwap;
use elio_common::catalog::{ConstraintKind, IndexCatalogEntry, IndexKind};
use tracing::warn;

pub use self::codec::{ConstraintCodec, IndexCodec};
pub(crate) use self::snapshot::{CatalogChange, DurableCatalogSnapshot};
use crate::error::GraphStoreError;
use crate::kv::{KvEngine, graph_keys};

/// In-memory catalog state, kept as a singleton in the database instance.
pub struct CatalogState {
    snapshot: ArcSwap<DurableCatalogSnapshot>,
}

impl CatalogState {
    pub fn new(engine: Arc<KvEngine>) -> Result<Self, GraphStoreError> {
        let snapshot = Arc::new(Self::load_snapshot_from_db(&engine)?);
        Ok(Self {
            snapshot: ArcSwap::from(snapshot),
        })
    }

    pub(crate) fn current_snapshot(&self) -> Arc<DurableCatalogSnapshot> {
        self.snapshot.load_full()
    }

    pub(crate) fn publish_changes(&self, changes: Vec<CatalogChange>) {
        if changes.is_empty() {
            return;
        }

        self.snapshot.rcu(|old| Arc::new(old.apply_changes(&changes)));
    }

    fn load_snapshot_from_db(engine: &KvEngine) -> Result<DurableCatalogSnapshot, GraphStoreError> {
        let rtxn = engine.graph.env.read_txn().map_err(GraphStoreError::Heed)?;
        let catalog_db = engine.graph.catalog;

        // Load constraints
        let mut constraints = Vec::new();
        let constraint_prefix = [graph_keys::CONSTRAINT_META_PREFIX];
        let iter = catalog_db.iter(&rtxn).map_err(GraphStoreError::Heed)?;
        for item in iter {
            let (key, value) = item.map_err(GraphStoreError::Heed)?;
            if !key.starts_with(&constraint_prefix) {
                continue;
            }

            let Some(name) = ConstraintCodec::decode_meta_key(key) else {
                warn!("skip invalid constraint meta key while loading snapshot");
                continue;
            };

            let Some(entry) = ConstraintCodec::decode_meta_value(name.clone(), value) else {
                warn!(constraint_name = %name, "skip invalid constraint meta value while loading snapshot");
                continue;
            };

            constraints.push(entry);
        }

        // Load indexes
        let mut indexes = Vec::new();
        let index_prefix = [graph_keys::INDEX_META_PREFIX];
        let iter = catalog_db.iter(&rtxn).map_err(GraphStoreError::Heed)?;
        for item in iter {
            let (key, value) = item.map_err(GraphStoreError::Heed)?;
            if !key.starts_with(&index_prefix) {
                continue;
            }

            let Some(name) = IndexCodec::decode_meta_key(key) else {
                warn!("skip invalid index meta key while loading snapshot");
                continue;
            };

            let Some(entry) = IndexCodec::decode_meta_value(name.clone(), value) else {
                warn!(index_name = %name, "skip invalid index meta value while loading snapshot");
                continue;
            };

            indexes.push(entry);
        }

        // Backward compatibility: synthesize missing index entries
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
