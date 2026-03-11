mod node;
mod relationship;

use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::array::{ArrayImpl, NodeArray, RelArray, StructArray, VirtualNodeArray};
use elio_common::{LabelId, NodeId, PropertyKeyId, SemanticDirection, TokenId};

pub use self::relationship::{NodeIdContainer, RelIterForNode};
use crate::codec::UniqueIndexCodec;
use crate::error::GraphStoreError;
use crate::id::IdStore;
use crate::kv::{CfKind, KvEngine};
use crate::token::TokenStore;
use crate::transaction::{DataChunkIterator, NodeScanOptions, Transaction};

pub struct GraphStore {
    db: Arc<KvEngine>,
    id_store: Arc<IdStore>,
    // token_store is not owned
    token_store: Arc<TokenStore>,
}

impl GraphStore {
    pub fn new(db: Arc<KvEngine>, token_store: Arc<TokenStore>) -> Result<Self, GraphStoreError> {
        let dict = Arc::new(IdStore::new(db.clone())?);
        Ok(Self {
            db,
            id_store: dict,
            token_store,
        })
    }

    pub fn db(&self) -> &Arc<KvEngine> {
        &self.db
    }

    pub fn dict(&self) -> &Arc<IdStore> {
        &self.id_store
    }

    pub fn token_store(&self) -> &Arc<TokenStore> {
        &self.token_store
    }

    // ==================== Graph Data Operations ====================
    // All operations take &Transaction as context

    pub fn node_create(
        &self,
        tx: &Transaction,
        labels: &[Arc<str>],
        props: &ArrayImpl,
    ) -> Result<NodeArray, GraphStoreError> {
        node::batch_node_create(tx, &self.id_store, &self.token_store, labels, props)
    }

    pub fn node_scan(
        &self,
        tx: &Transaction,
        opts: NodeScanOptions,
    ) -> Result<Box<dyn DataChunkIterator>, GraphStoreError> {
        node::batch_node_scan(tx, opts)
    }

    pub fn node_scan_by_label(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        opts: NodeScanOptions,
    ) -> Result<Box<dyn DataChunkIterator>, GraphStoreError> {
        node::batch_node_scan_by_label(tx, label_id, opts)
    }

    pub fn materialize_node(
        &self,
        tx: &Transaction,
        node_ids: &VirtualNodeArray,
        vis: &BitVec,
    ) -> Result<NodeArray, GraphStoreError> {
        node::batch_materialize_node(tx, &self.token_store, node_ids, vis)
    }

    pub fn rel_create<A, B>(
        &self,
        tx: &Transaction,
        rtype: &Arc<str>,
        start: &A,
        end: &B,
        prop: &StructArray,
    ) -> Result<RelArray, GraphStoreError>
    where
        A: NodeIdContainer,
        B: NodeIdContainer,
    {
        relationship::batch_rel_create(tx, &self.id_store, &self.token_store, rtype, start, end, prop)
    }

    pub fn rel_iter_for_node<'a>(
        &self,
        tx: &'a Transaction,
        node_id: NodeId,
        dir: SemanticDirection,
        rtypes: &[TokenId],
    ) -> Result<RelIterForNode<'a>, GraphStoreError> {
        relationship::rel_iter_for_node(tx, node_id, dir, rtypes)
    }

    // ==================== Graph Index Operations ====================
    /// Check if a unique index entry exists (reads from transaction)
    pub fn unique_index_exists(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<bool, GraphStoreError> {
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        Ok(tx.get(CfKind::IndexData, &key)?.is_some())
    }

    /// Get node_id from unique index (reads from transaction)
    pub fn get_unique_index(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<Option<NodeId>, GraphStoreError> {
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        match tx.get(CfKind::IndexData, &key)? {
            Some(value) => Ok(UniqueIndexCodec::decode_value(&value)),
            None => Ok(None),
        }
    }

    /// Put unique index entry (buffered in transaction write buffer)
    pub fn put_unique_index(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
        node_id: NodeId,
    ) -> Result<(), GraphStoreError> {
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        let value = UniqueIndexCodec::encode_value(node_id);
        tx.put(CfKind::IndexData, key.to_vec(), value.to_vec());
        Ok(())
    }

    /// Delete unique index entry (buffered in transaction write buffer)
    pub fn delete_unique_index(
        &self,
        tx: &Transaction,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<(), GraphStoreError> {
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        tx.delete(CfKind::IndexData, key.to_vec());
        Ok(())
    }
}
