use std::ops::Deref;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bitvec::vec::BitVec;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{ArrayImpl, NodeArray, RelArray, StructArray, VirtualNodeArray};
use elio_common::{LabelId, NodeId, PropertyKeyId, SemanticDirection, TokenId};

use crate::constraint::{ConstraintCodec, ConstraintMeta, UniqueIndexCodec};
use crate::dict::IdStore;
use crate::error::GraphStoreError;
use crate::token::TokenStore;
use crate::transaction::node::{batch_materialize_node, batch_node_create, batch_node_scan};
use crate::transaction::relationship::{NodeIdContainer, RelIterForNode, batch_rel_create, rel_iter_for_node};
use crate::{KvEngine, cf_constraint};

mod node;
mod relationship;

// global transaction lock guard, this is used to enforce single-writer
pub enum TxGuard {
    Read(parking_lot::RwLockReadGuard<'static, ()>),
    Write(parking_lot::RwLockWriteGuard<'static, ()>),
}

// #safety
//   the underlying lock is held by the guard and lives as long as TxGuard
unsafe impl Send for TxGuard {}
unsafe impl Sync for TxGuard {}

pub struct RelScanOptions {}
pub struct NodeScanOptions {
    pub batch_size: usize,
}

#[async_trait]
pub trait DataChunkIterator: Send {
    fn next_batch(&mut self) -> Result<Option<DataChunk>, GraphStoreError>;
}

// Simple transaction implementation with snapshot and write batch buffer
pub struct TransactionImpl {
    pub(crate) inner: OwnedSnapshot,
    dict: Arc<IdStore>,
    token: Arc<TokenStore>,
    // write buffer
    write_state: Mutex<WriteState>,
    // holds global tx lock guard to enforce single-writer
    // RAII, lock will release when dropped
    #[allow(unused)]
    tx_guard: TxGuard,
}

#[derive(Default)]
pub struct WriteState {
    // TODO(pgao): should we use transaction db?
    pub(crate) batch: rocksdb::WriteBatchWithTransaction<false>,
    // TODO(pgao): local buffer
    // local_cache: HashMap<Vec<u8>, Option<Vec<u8>>>,
}

impl TransactionImpl {
    pub fn new(db: Arc<KvEngine>, dict: Arc<IdStore>, token: Arc<TokenStore>, tx_guard: TxGuard) -> Self {
        Self {
            inner: OwnedSnapshot::new(db),
            dict,
            token,
            write_state: WriteState::default().into(),
            tx_guard,
        }
    }
}

// impl Transaction for TransactionImpl {
impl TransactionImpl {
    pub fn rel_scan(&self, _opts: &RelScanOptions) -> Result<Box<dyn DataChunkIterator>, GraphStoreError> {
        todo!()
    }

    pub fn node_scan(&self, opts: NodeScanOptions) -> Result<Box<dyn DataChunkIterator + '_>, GraphStoreError> {
        batch_node_scan(self, opts)
    }

    pub fn materialize_node(&self, node_ids: &VirtualNodeArray, vis: &BitVec) -> Result<NodeArray, GraphStoreError> {
        batch_materialize_node(self, node_ids, vis)
    }

    pub fn node_create(&self, label: &[Arc<str>], prop: &ArrayImpl) -> Result<NodeArray, GraphStoreError> {
        batch_node_create(self, label, prop)
    }

    pub fn relationship_create<A, B>(
        &self,
        rtype: &Arc<str>,
        start: &A,          // VirtualNode/Node
        end: &B,            // VirtualNode/Node
        prop: &StructArray, // Struct or Any
    ) -> Result<RelArray, GraphStoreError>
    where
        A: NodeIdContainer,
        B: NodeIdContainer,
    {
        batch_rel_create(self, rtype, start, end, prop)
    }

    pub fn node_delete(&self, _node: &DataChunk) -> Result<(), GraphStoreError> {
        todo!()
    }

    pub fn relationship_delete(&self, _rel: &DataChunk) -> Result<(), GraphStoreError> {
        todo!()
    }

    pub fn rel_iter_for_node(
        &self,
        node_id: NodeId,
        dir: SemanticDirection,
        rtypes: &[TokenId],
    ) -> Result<RelIterForNode<'_>, GraphStoreError> {
        rel_iter_for_node(self, node_id, dir, rtypes)
    }

    pub fn commit(&self) -> Result<(), GraphStoreError> {
        let mut state = self.write_state.lock().unwrap();
        let batch = std::mem::take(&mut state.batch);
        self.inner._db.write(batch)?;
        Ok(())
    }

    pub fn abort(&self) -> Result<(), GraphStoreError> {
        let mut state = self.write_state.lock().unwrap();
        state.batch.clear();
        Ok(())
    }

    // ==================== Constraint Operations ====================

    /// Check if a constraint exists (reads from snapshot)
    pub fn constraint_exists(&self, name: &str) -> Result<bool, GraphStoreError> {
        let cf = self.inner._db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = ConstraintCodec::encode_meta_key(name);
        Ok(self.inner.snapshot.get_cf(&cf, &key)?.is_some())
    }

    /// Get constraint metadata by name (reads from snapshot)
    pub fn get_constraint(&self, name: &str) -> Result<Option<ConstraintMeta>, GraphStoreError> {
        let cf = self.inner._db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = ConstraintCodec::encode_meta_key(name);
        match self.inner.snapshot.get_cf(&cf, &key)? {
            Some(value) => Ok(ConstraintCodec::decode_meta_value(name.to_string(), &value)),
            None => Ok(None),
        }
    }

    /// Get all constraints for a label (reads from snapshot)
    pub fn get_constraints_for_label(&self, label_id: LabelId) -> Result<Vec<ConstraintMeta>, GraphStoreError> {
        let cf = self.inner._db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let prefix = ConstraintCodec::encode_label_constraint_prefix(label_id);

        let mut constraints = Vec::new();
        let mut readopts = rocksdb::ReadOptions::default();
        readopts.set_prefix_same_as_start(true);
        let mode = rocksdb::IteratorMode::From(&prefix, rocksdb::Direction::Forward);
        let iter = self.inner.snapshot.iterator_cf_opt(&cf, readopts, mode);

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
            if let Some(meta) = self.get_constraint(&name)? {
                constraints.push(meta);
            }
        }

        Ok(constraints)
    }

    /// Store a constraint (buffered in write batch)
    pub fn put_constraint(&self, meta: &ConstraintMeta) -> Result<(), GraphStoreError> {
        let cf = self.inner._db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let mut guard = self.write_state.lock().unwrap();

        // Store metadata
        let meta_key = ConstraintCodec::encode_meta_key(&meta.name);
        let meta_value = ConstraintCodec::encode_meta_value(meta);
        guard.batch.put_cf(&cf, &meta_key, &meta_value);

        // Store label-to-constraint mapping
        let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_id, &meta.name);
        guard.batch.put_cf(&cf, &label_key, []);

        Ok(())
    }

    /// Delete a constraint (buffered in write batch)
    pub fn delete_constraint(&self, name: &str) -> Result<(), GraphStoreError> {
        let cf = self.inner._db.cf_handle(cf_constraint::CF_NAME).unwrap();

        // Get the constraint first to find the label_id
        if let Some(meta) = self.get_constraint(name)? {
            let mut guard = self.write_state.lock().unwrap();

            // Delete label-to-constraint mapping
            let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_id, name);
            guard.batch.delete_cf(&cf, &label_key);

            // Delete metadata
            let meta_key = ConstraintCodec::encode_meta_key(name);
            guard.batch.delete_cf(&cf, &meta_key);
        }

        Ok(())
    }

    // ==================== Unique Index Operations ====================

    /// Check if a unique index entry exists (reads from snapshot)
    pub fn unique_index_exists(
        &self,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<bool, GraphStoreError> {
        let cf = self.inner._db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        Ok(self.inner.snapshot.get_cf(&cf, &key)?.is_some())
    }

    /// Get node_id from unique index (reads from snapshot)
    pub fn get_unique_index(
        &self,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<Option<NodeId>, GraphStoreError> {
        let cf = self.inner._db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        match self.inner.snapshot.get_cf(&cf, &key)? {
            Some(value) => Ok(UniqueIndexCodec::decode_value(&value)),
            None => Ok(None),
        }
    }

    /// Put unique index entry (buffered in write batch)
    pub fn put_unique_index(
        &self,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
        node_id: NodeId,
    ) -> Result<(), GraphStoreError> {
        let cf = self.inner._db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        let value = UniqueIndexCodec::encode_value(node_id);

        let mut guard = self.write_state.lock().unwrap();
        guard.batch.put_cf(&cf, &key, &value);
        Ok(())
    }

    /// Delete unique index entry (buffered in write batch)
    pub fn delete_unique_index(
        &self,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<(), GraphStoreError> {
        let cf = self.inner._db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);

        let mut guard = self.write_state.lock().unwrap();
        guard.batch.delete_cf(&cf, &key);
        Ok(())
    }
}

struct OwnedSnapshot {
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
}

impl Deref for OwnedSnapshot {
    type Target = rocksdb::Snapshot<'static>;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}
