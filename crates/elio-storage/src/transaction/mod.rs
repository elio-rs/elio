pub mod manager;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use elio_common::catalog::{ConstraintCatalogEntry, ConstraintKind, IndexCatalogEntry, IndexHint, IndexKind};
use elio_common::{LabelId, PropertyKeyId};

use crate::catalog::codec::{ConstraintCodec, IndexCodec};
use crate::catalog::{CatalogChange, DurableCatalogSnapshot};
use crate::error::GraphStoreError;
use crate::kv::{self, CfKind, KvEngine};
use crate::transaction::manager::TxGuard;

/// Callback to publish catalog changes on commit.
pub(crate) type CatalogCommitFn = Arc<dyn Fn(Vec<CatalogChange>) + Send + Sync>;

/// Buffered write operation.
#[derive(Debug, Clone)]
pub(crate) enum WriteOp {
    Put(Vec<u8>),
    Delete,
}

/// Thin transaction — isolation context only.
/// Contains write buffer for reads/writes and lock guard for concurrency.
pub struct Transaction {
    pub(crate) db: Arc<KvEngine>,
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
    pub(crate) buffer: BTreeMap<(CfKind, Vec<u8>), WriteOp>,
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
    /// Read a key from the given column family, checking the write buffer first.
    pub(crate) fn get(&self, cf: CfKind, key: &[u8]) -> Result<Option<Vec<u8>>, GraphStoreError> {
        // Check write buffer first
        let guard = self.write_state.lock().unwrap();
        if let Some(op) = guard.buffer.get(&(cf, key.to_vec())) {
            return match op {
                WriteOp::Put(val) => Ok(Some(val.clone())),
                WriteOp::Delete => Ok(None),
            };
        }
        drop(guard);

        // Fall through to BfTree
        kv::bf_read(self.db.tree(cf), key)
    }

    /// Buffer a put operation.
    pub(crate) fn put(&self, cf: CfKind, key: Vec<u8>, value: Vec<u8>) {
        let mut guard = self.write_state.lock().unwrap();
        guard.buffer.insert((cf, key), WriteOp::Put(value));
    }

    /// Buffer a delete operation.
    pub(crate) fn delete(&self, cf: CfKind, key: Vec<u8>) {
        let mut guard = self.write_state.lock().unwrap();
        guard.buffer.insert((cf, key), WriteOp::Delete);
    }

    /// Prefix scan over BfTree snapshot only (no write buffer merge).
    #[allow(dead_code)]
    pub(crate) fn prefix_scan_snapshot(&self, cf: CfKind, prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        kv::bf_prefix_scan(self.db.tree(cf), prefix)
    }

    pub(crate) fn prefix_scan_snapshot_iter<'a>(
        &'a self,
        cf: CfKind,
        prefix: &[u8],
    ) -> Result<kv::BfPrefixScanIter<'a>, GraphStoreError> {
        kv::bf_prefix_scan_iter(self.db.tree(cf), prefix)
    }

    /// Prefix scan merging write buffer with BfTree results.
    pub(crate) fn prefix_scan(&self, cf: CfKind, prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        // Collect from BfTree
        let tree_results = kv::bf_prefix_scan(self.db.tree(cf), prefix);

        // Collect from write buffer
        let guard = self.write_state.lock().unwrap();
        let buf_start = (cf, prefix.to_vec());
        let buf_end = match kv::prefix_successor(prefix) {
            Some(succ) => (cf, succ),
            None => {
                // No upper bound — scan to end of this cf's entries
                // Use the next CfKind variant as bound
                match cf {
                    CfKind::Catalog => (CfKind::Meta, vec![]),
                    CfKind::Meta => (CfKind::Topology, vec![]),
                    CfKind::Topology => (CfKind::Data, vec![]),
                    CfKind::Data => (CfKind::IndexData, vec![]),
                    CfKind::IndexData => {
                        // Last variant — collect all remaining
                        let buf_entries: Vec<_> = guard
                            .buffer
                            .range((cf, prefix.to_vec())..)
                            .filter(|((c, k), _)| *c == cf && k.starts_with(prefix))
                            .filter_map(|((_, k), op)| match op {
                                WriteOp::Put(v) => Some((k.clone(), v.clone())),
                                WriteOp::Delete => None,
                            })
                            .collect();
                        drop(guard);
                        return merge_scan_results(tree_results, buf_entries);
                    }
                }
            }
        };

        let buf_entries: Vec<_> = guard
            .buffer
            .range(buf_start..buf_end)
            .filter(|((c, k), _)| *c == cf && k.starts_with(prefix))
            .filter_map(|((_, k), op)| match op {
                WriteOp::Put(v) => Some((k.clone(), v.clone())),
                WriteOp::Delete => None,
            })
            .collect();
        // Collect deleted keys to filter out from tree results
        let deleted_keys: std::collections::HashSet<Vec<u8>> = guard
            .buffer
            .range((cf, prefix.to_vec())..)
            .take_while(|((c, k), _)| *c == cf && k.starts_with(prefix))
            .filter_map(|((_, k), op)| match op {
                WriteOp::Delete => Some(k.clone()),
                _ => None,
            })
            .collect();
        drop(guard);

        // Filter tree results to remove deleted entries
        let tree_results: Vec<_> = tree_results
            .into_iter()
            .filter(|(k, _)| !deleted_keys.contains(k))
            .collect();

        merge_scan_results(tree_results, buf_entries)
    }

    pub fn commit(&self) -> Result<(), GraphStoreError> {
        let mut state = self.write_state.lock().unwrap();
        let buffer = std::mem::take(&mut state.buffer);
        let catalog_changes = std::mem::take(&mut state.catalog_changes);
        drop(state);

        // Apply all buffered writes to BfTree instances
        for ((cf, key), op) in buffer {
            let tree = self.db.tree(cf);
            match op {
                WriteOp::Put(value) => {
                    kv::bf_insert(tree, &key, &value)?;
                }
                WriteOp::Delete => {
                    tree.delete(&key);
                }
            }
        }

        (self.on_catalog_commit)(catalog_changes);
        Ok(())
    }

    pub fn abort(&self) -> Result<(), GraphStoreError> {
        let mut state = self.write_state.lock().unwrap();
        state.buffer.clear();
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

    /// Store a constraint (buffered in write buffer, applied on commit).
    /// For Unique/NodeKey constraints, a backing index is automatically created.
    pub fn put_constraint(&self, meta: &ConstraintCatalogEntry) -> Result<(), GraphStoreError> {
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

            // Persist index
            let index_key = IndexCodec::encode_meta_key(&index_entry.name);
            let index_value = IndexCodec::encode_meta_value(&index_entry);
            guard.buffer.insert(
                (CfKind::Catalog, index_key.to_vec()),
                WriteOp::Put(index_value.to_vec()),
            );
            guard.catalog_changes.push(CatalogChange::UpsertIndex(index_entry));

            meta.backing_index = Some(index_name);
        }

        // Store constraint metadata
        let meta_key = ConstraintCodec::encode_meta_key(&meta.name);
        let meta_value = ConstraintCodec::encode_meta_value(&meta);
        guard
            .buffer
            .insert((CfKind::Catalog, meta_key.to_vec()), WriteOp::Put(meta_value.to_vec()));

        // Store label-to-constraint mapping
        let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_or_rel_id, &meta.name);
        guard.buffer.insert(
            (CfKind::Catalog, label_key.to_vec()),
            WriteOp::Put(kv::EMPTY_VALUE_SENTINEL.to_vec()),
        );
        guard.catalog_changes.push(CatalogChange::UpsertConstraint(meta));

        Ok(())
    }

    /// Delete a constraint (buffered in write buffer, applied on commit).
    /// Also deletes the backing index if present.
    pub fn delete_constraint(&self, name: &str) -> Result<(), GraphStoreError> {
        // Get the constraint from transaction's catalog snapshot
        if let Some(meta) = self.catalog_snapshot.get_constraint(name) {
            let mut guard = self.write_state.lock().unwrap();

            // Delete backing index if present
            if let Some(ref index_name) = meta.backing_index {
                let index_key = IndexCodec::encode_meta_key(index_name);
                guard
                    .buffer
                    .insert((CfKind::Catalog, index_key.to_vec()), WriteOp::Delete);
                guard.catalog_changes.push(CatalogChange::DeleteIndex {
                    name: index_name.clone(),
                    label_id: meta.label_or_rel_id,
                });
            }

            // Delete label-to-constraint mapping
            let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_or_rel_id, name);
            guard
                .buffer
                .insert((CfKind::Catalog, label_key.to_vec()), WriteOp::Delete);

            // Delete constraint metadata
            let meta_key = ConstraintCodec::encode_meta_key(name);
            guard
                .buffer
                .insert((CfKind::Catalog, meta_key.to_vec()), WriteOp::Delete);
            guard.catalog_changes.push(CatalogChange::DeleteConstraint {
                name: meta.name,
                label_id: meta.label_or_rel_id,
            });
        }

        Ok(())
    }
}

/// Merge two sorted key-value Vecs, preferring buf_entries on key conflict.
fn merge_scan_results(
    tree_results: Vec<(Vec<u8>, Vec<u8>)>,
    buf_entries: Vec<(Vec<u8>, Vec<u8>)>,
) -> Vec<(Vec<u8>, Vec<u8>)> {
    if buf_entries.is_empty() {
        return tree_results;
    }
    if tree_results.is_empty() {
        return buf_entries;
    }

    let mut merged = Vec::with_capacity(tree_results.len() + buf_entries.len());
    let mut ti = tree_results.into_iter().peekable();
    let mut bi = buf_entries.into_iter().peekable();

    loop {
        match (ti.peek(), bi.peek()) {
            (Some((tk, _)), Some((bk, _))) => match tk.cmp(bk) {
                std::cmp::Ordering::Less => merged.push(ti.next().unwrap()),
                std::cmp::Ordering::Greater => merged.push(bi.next().unwrap()),
                std::cmp::Ordering::Equal => {
                    ti.next(); // skip tree entry, buf wins
                    merged.push(bi.next().unwrap());
                }
            },
            (Some(_), None) => merged.push(ti.next().unwrap()),
            (None, Some(_)) => merged.push(bi.next().unwrap()),
            (None, None) => break,
        }
    }
    merged
}
