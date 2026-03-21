use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::TokenKind;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{Array, ArrayImpl, NodeArray, NodeArrayBuilder, VirtualNodeArray, VirtualNodeArrayBuilder};
use elio_common::scalar::{NodeValueRef, StructValue};

use crate::codec::{AdaptiveNodeCodec, GraphKeyCodec, LabelIndexCodec};
use crate::error::GraphStoreError;
use crate::id::IdStore;
use crate::kv::graph_keys;
use crate::token::TokenStore;
use crate::transaction::{DataChunkIterator, NodeScanOptions, Transaction};

pub(crate) fn batch_node_create(
    tx: &Transaction,
    dict: &IdStore,
    token: &TokenStore,
    labels: &[Arc<str>],
    props: &ArrayImpl,
) -> Result<NodeArray, GraphStoreError> {
    let len = props.len();

    let props = props
        .as_struct()
        .ok_or(GraphStoreError::type_mismatch("Expected struct array"))?;

    let label_ids = labels
        .iter()
        .map(|l| token.get_or_create_token(l, TokenKind::Label))
        .collect::<Result<Vec<_>, _>>()?;

    let token_ids = props
        .fields()
        .iter()
        .map(|(k, _)| token.get_or_create_token(k, TokenKind::PropertyKey))
        .collect::<Result<Vec<_>, _>>()?;

    let node_ids = dict.batch_node_id(len)?;

    // Write to graph env via RwTxn
    tx.with_rw_txn_mut(|txn| {
        let engine = &tx._engine;
        let graph_db = engine.graph.graph;
        let index_db = engine.graph.index;

        for (node_id, prop) in node_ids.iter().zip(props.iter()) {
            let prop = prop.unwrap();
            let prop_bytes =
                AdaptiveNodeCodec::encode_property_value(&token_ids, prop).map_err(GraphStoreError::internal)?;

            // New node has 0 edges → always packed
            let value = AdaptiveNodeCodec::try_encode_packed(&label_ids, &prop_bytes, &[], &[])
                .expect("new node with 0 edges should always fit in packed mode");

            let key = GraphKeyCodec::encode_header_key(*node_id);
            graph_db.put(txn, &key, &value).map_err(GraphStoreError::Heed)?;

            // write label index entries
            for label_id in &label_ids {
                let label_key = LabelIndexCodec::encode_key(*label_id, *node_id);
                index_db.put(txn, &label_key, &[]).map_err(GraphStoreError::Heed)?;
            }
        }
        Ok::<_, GraphStoreError>(())
    })?;

    // create node array
    let mut builder = NodeArrayBuilder::with_capacity(len);

    for (i, node_id) in node_ids.iter().enumerate() {
        let node_ref = NodeValueRef {
            id: *node_id,
            labels,
            props: props.get(i).unwrap(),
        };
        builder.push(Some(node_ref));
    }

    Ok(builder.finish())
}

pub(crate) fn batch_node_scan(
    tx: &Transaction,
    opts: NodeScanOptions,
) -> Result<Box<dyn DataChunkIterator + '_>, GraphStoreError> {
    let node_ids = tx.with_rw_txn(|txn| {
        let graph_db = tx._engine.graph.graph;

        let mut node_ids = Vec::new();
        let iter = graph_db.iter(txn).map_err(GraphStoreError::Heed)?;
        for result in iter {
            let (key, _) = result.map_err(GraphStoreError::Heed)?;
            if key.len() < 9 {
                continue;
            }
            let (node_id, segment) = GraphKeyCodec::decode(key);
            if segment == graph_keys::SEGMENT_HEADER {
                node_ids.push(node_id);
            }
        }
        Ok::<_, GraphStoreError>(node_ids)
    })?;

    Ok(Box::new(NodeIterator { node_ids, pos: 0, opts }))
}

pub(crate) fn batch_node_scan_by_label(
    tx: &Transaction,
    label_id: elio_common::LabelId,
    opts: NodeScanOptions,
) -> Result<Box<dyn DataChunkIterator + '_>, GraphStoreError> {
    let prefix = LabelIndexCodec::encode_prefix(label_id);

    let node_ids = tx.with_rw_txn(|txn| {
        let index_db = tx._engine.graph.index;

        let mut node_ids = Vec::new();
        let iter = index_db.iter(txn).map_err(GraphStoreError::Heed)?;
        for result in iter {
            let (key, _) = result.map_err(GraphStoreError::Heed)?;
            if key.starts_with(&prefix) {
                let (_label_id, node_id) = LabelIndexCodec::decode_key(key);
                node_ids.push(node_id);
            } else if key > prefix.as_ref() {
                break;
            }
        }
        Ok::<_, GraphStoreError>(node_ids)
    })?;

    Ok(Box::new(NodeIterator { node_ids, pos: 0, opts }))
}

pub(crate) fn batch_materialize_node(
    tx: &Transaction,
    token: &TokenStore,
    node_ids: &VirtualNodeArray,
    vis: &BitVec,
) -> Result<NodeArray, GraphStoreError> {
    let mut builder = NodeArrayBuilder::with_capacity(node_ids.len());

    // Collect data from LMDB within the txn lock
    struct NodeData {
        label_strs: Vec<Arc<str>>,
        struct_value: StructValue,
    }

    let mut results: Vec<Option<NodeData>> = Vec::with_capacity(node_ids.len());

    tx.with_rw_txn(|txn| {
        let graph_db = tx._engine.graph.graph;

        for (idx, node_id) in node_ids.iter().enumerate() {
            if !vis[idx] || node_id.is_none() {
                results.push(None);
                continue;
            }

            let node_id = node_id.unwrap();
            let key = GraphKeyCodec::encode_header_key(node_id);
            let val = graph_db.get(txn, &key).map_err(GraphStoreError::Heed)?;

            if let Some(val) = val {
                let (_is_packed, label_ids_ref, prop_bytes, _remaining) =
                    AdaptiveNodeCodec::decode_header(val).map_err(GraphStoreError::internal)?;

                let label_strs = label_ids_ref
                    .iter()
                    .map(|id| token.get_token_val(id, TokenKind::Label))
                    .collect::<Result<Vec<_>, _>>()?;

                let prop_map = AdaptiveNodeCodec::decode_property_map(prop_bytes);
                let struct_value = {
                    let mut fields = vec![];
                    for entry in prop_map.iter() {
                        let key = token.get_token_val(entry.key(), TokenKind::PropertyKey)?;
                        fields.push((key, entry.value().to_owned_scalar()));
                    }
                    StructValue::new(fields)
                };

                results.push(Some(NodeData {
                    label_strs,
                    struct_value,
                }));
            } else {
                tracing::warn!("node id {} not found", node_id);
                results.push(None);
            }
        }
        Ok::<_, GraphStoreError>(())
    })?;

    // Build the array outside the txn lock
    for (idx, node_id) in node_ids.iter().enumerate() {
        if !vis[idx] || node_id.is_none() {
            builder.push(None);
            continue;
        }

        let node_id = node_id.unwrap();
        match &results[idx] {
            Some(data) => {
                let node_ref = NodeValueRef {
                    id: node_id,
                    labels: &data.label_strs,
                    props: data.struct_value.as_scalar_ref(),
                };
                builder.push(Some(node_ref));
            }
            None => {
                builder.push(None);
            }
        }
    }

    Ok(builder.finish())
}

/// Iterator that yields batches of node IDs from a pre-collected Vec
pub struct NodeIterator {
    node_ids: Vec<elio_common::NodeId>,
    pos: usize,
    opts: NodeScanOptions,
}

impl DataChunkIterator for NodeIterator {
    fn next_batch(&mut self) -> Result<Option<DataChunk>, GraphStoreError> {
        if self.pos >= self.node_ids.len() {
            return Ok(None);
        }

        let end = (self.pos + self.opts.batch_size).min(self.node_ids.len());
        let mut builder = VirtualNodeArrayBuilder::with_capacity(self.opts.batch_size);

        for i in self.pos..end {
            builder.push(Some(self.node_ids[i]));
        }
        self.pos = end;

        let array = builder.finish();
        if array.is_empty() {
            Ok(None)
        } else {
            let vis = BitVec::repeat(true, array.len());
            let chunk = DataChunk::new(vec![Arc::new(array.into())], vis);
            Ok(Some(chunk))
        }
    }
}
