mod node;
mod relationship;

use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::array::{ArrayImpl, NodeArray, RelArray, StructArray, VirtualNodeArray};
use elio_common::{NodeId, SemanticDirection, TokenId};

pub use self::relationship::{NodeIdContainer, RelIterForNode};
use crate::catalog::CatalogStore;
use crate::error::GraphStoreError;
use crate::id::IdStore;
use crate::kv::KvEngine;
use crate::token::TokenStore;
use crate::transaction::{DataChunkIterator, NodeScanOptions, Transaction};

pub struct GraphStore {
    db: Arc<KvEngine>,
    dict: Arc<IdStore>,
    token: Arc<TokenStore>,
    constraint: Arc<CatalogStore>,
}

impl GraphStore {
    pub fn new(db: Arc<KvEngine>) -> Result<Self, GraphStoreError> {
        let dict = Arc::new(IdStore::new(db.clone())?);
        let token = Arc::new(TokenStore::new(db.clone())?);
        let constraint = Arc::new(CatalogStore::new(db.clone()));
        Ok(Self {
            db,
            dict,
            token,
            constraint,
        })
    }

    pub fn db(&self) -> &Arc<KvEngine> {
        &self.db
    }

    pub fn dict(&self) -> &Arc<IdStore> {
        &self.dict
    }

    pub fn token_store(&self) -> &Arc<TokenStore> {
        &self.token
    }

    pub fn constraint_store(&self) -> &Arc<CatalogStore> {
        &self.constraint
    }

    // ==================== Graph Data Operations ====================
    // All operations take &Transaction as context

    pub fn node_create(
        &self,
        tx: &Transaction,
        labels: &[Arc<str>],
        props: &ArrayImpl,
    ) -> Result<NodeArray, GraphStoreError> {
        node::batch_node_create(tx, &self.dict, &self.token, labels, props)
    }

    pub fn node_scan<'a>(
        &'a self,
        tx: &'a Transaction,
        opts: NodeScanOptions,
    ) -> Result<Box<dyn DataChunkIterator + 'a>, GraphStoreError> {
        node::batch_node_scan(tx, opts)
    }

    pub fn materialize_node(
        &self,
        tx: &Transaction,
        node_ids: &VirtualNodeArray,
        vis: &BitVec,
    ) -> Result<NodeArray, GraphStoreError> {
        node::batch_materialize_node(tx, &self.token, node_ids, vis)
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
        relationship::batch_rel_create(tx, &self.dict, &self.token, rtype, start, end, prop)
    }

    pub fn rel_iter_for_node<'a>(
        &'a self,
        tx: &'a Transaction,
        node_id: NodeId,
        dir: SemanticDirection,
        rtypes: &[TokenId],
    ) -> Result<RelIterForNode<'a>, GraphStoreError> {
        relationship::rel_iter_for_node(tx, node_id, dir, rtypes)
    }
}
