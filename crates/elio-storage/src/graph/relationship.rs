use std::collections::HashMap;
use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::array::{Array, NodeArray, RelArray, RelArrayBuilder, StructArray, VirtualNodeArray};
use elio_common::scalar::{RelValueRef, StructValue};
use elio_common::store_types::RelDirection;
use elio_common::{NodeId, RelationshipId, SemanticDirection, TokenId, TokenKind};

use crate::codec::{AdaptiveNodeCodec, EdgeEntry, GraphKeyCodec};
use crate::error::GraphStoreError;
use crate::id::IdStore;
use crate::token::TokenStore;
use crate::transaction::Transaction;

pub(crate) fn batch_rel_create<A, B>(
    tx: &Transaction,
    dict: &IdStore,
    token: &TokenStore,
    rtype: &Arc<str>,
    start: &A,
    end: &B,
    prop: &StructArray,
) -> Result<RelArray, GraphStoreError>
where
    A: NodeIdContainer,
    B: NodeIdContainer,
{
    if !start.valid_map().all() || !end.valid_map().all() {
        return Err(GraphStoreError::internal("start/end should not be null".to_string()));
    }

    assert_eq!(start.len(), end.len());
    assert_eq!(prop.len(), start.len());

    let rtype_id = token.get_or_create_token(rtype, TokenKind::RelationshipType)?;
    let rel_ids = dict.batch_rel_id(start.len())?;
    let prop_key_ids = prop
        .field_names()
        .map(|k| token.get_or_create_token(k, TokenKind::PropertyKey))
        .collect::<Result<Vec<_>, _>>()?;
    let len = start.len();

    // Encode property bytes for each relationship
    let empty_prop = StructValue::default();
    let empty_prop_ref = empty_prop.as_scalar_ref();
    let mut rel_prop_bytes = Vec::with_capacity(len);
    for i in 0..len {
        let p = prop.get(i).unwrap_or(empty_prop_ref);
        let prop_bytes = AdaptiveNodeCodec::encode_property_value(&prop_key_ids, p)
            .map_err(|e| GraphStoreError::internal(e.to_string()))?;
        rel_prop_bytes.push(prop_bytes);
    }

    // Group edges by affected node_id
    struct PendingEdge {
        edge: EdgeEntry,
        is_out: bool,
    }

    let mut node_edges: HashMap<NodeId, Vec<PendingEdge>> = HashMap::new();

    for (i, rel_id) in rel_ids.iter().enumerate() {
        let start_id = start.get_unchecked(i);
        let end_id = end.get_unchecked(i);

        node_edges.entry(start_id).or_default().push(PendingEdge {
            edge: EdgeEntry {
                rel_id: *rel_id,
                other_node_id: end_id,
                rel_type_id: rtype_id,
                prop_bytes: rel_prop_bytes[i].clone(),
            },
            is_out: true,
        });

        node_edges.entry(end_id).or_default().push(PendingEdge {
            edge: EdgeEntry {
                rel_id: *rel_id,
                other_node_id: start_id,
                rel_type_id: rtype_id,
                prop_bytes: rel_prop_bytes[i].clone(),
            },
            is_out: false,
        });
    }

    // Read-modify-write for each affected node
    tx.with_rw_txn_mut(|txn| {
        let graph_db = tx._engine.graph.graph;

        for (node_id, pending) in &node_edges {
            let header_key = GraphKeyCodec::encode_header_key(*node_id);

            let header_val = graph_db
                .get(txn, &header_key)
                .map_err(GraphStoreError::Heed)?
                .ok_or_else(|| GraphStoreError::internal(format!("node {} not found for rel create", node_id)))?;

            let (is_packed, label_ref, prop_bytes, remaining) =
                AdaptiveNodeCodec::decode_header(header_val).map_err(GraphStoreError::internal)?;

            let labels: Vec<u16> = label_ref.iter().collect();
            let prop_bytes_owned = prop_bytes.to_vec();

            let (mut out_edges, mut in_edges) = if is_packed {
                AdaptiveNodeCodec::decode_packed_edges(remaining).map_err(GraphStoreError::internal)?
            } else {
                let out_key = GraphKeyCodec::encode_out_edges_key(*node_id);
                let in_key = GraphKeyCodec::encode_in_edges_key(*node_id);

                let out = match graph_db.get(txn, &out_key).map_err(GraphStoreError::Heed)? {
                    Some(data) => AdaptiveNodeCodec::decode_edge_list(data).map_err(GraphStoreError::internal)?,
                    None => Vec::new(),
                };
                let inc = match graph_db.get(txn, &in_key).map_err(GraphStoreError::Heed)? {
                    Some(data) => AdaptiveNodeCodec::decode_edge_list(data).map_err(GraphStoreError::internal)?,
                    None => Vec::new(),
                };
                (out, inc)
            };

            for p in pending {
                if p.is_out {
                    out_edges.push(p.edge.clone());
                } else {
                    in_edges.push(p.edge.clone());
                }
            }

            if let Some(packed) =
                AdaptiveNodeCodec::try_encode_packed(&labels, &prop_bytes_owned, &out_edges, &in_edges)
            {
                graph_db.put(txn, &header_key, &packed).map_err(GraphStoreError::Heed)?;
                if !is_packed {
                    let out_key = GraphKeyCodec::encode_out_edges_key(*node_id);
                    let in_key = GraphKeyCodec::encode_in_edges_key(*node_id);
                    let _ = graph_db.delete(txn, &out_key);
                    let _ = graph_db.delete(txn, &in_key);
                }
            } else {
                let header = AdaptiveNodeCodec::encode_header(&labels, &prop_bytes_owned);
                let out_data = AdaptiveNodeCodec::encode_edge_list(&out_edges);
                let in_data = AdaptiveNodeCodec::encode_edge_list(&in_edges);

                graph_db.put(txn, &header_key, &header).map_err(GraphStoreError::Heed)?;
                let out_key = GraphKeyCodec::encode_out_edges_key(*node_id);
                let in_key = GraphKeyCodec::encode_in_edges_key(*node_id);
                graph_db.put(txn, &out_key, &out_data).map_err(GraphStoreError::Heed)?;
                graph_db.put(txn, &in_key, &in_data).map_err(GraphStoreError::Heed)?;
            }
        }
        Ok::<_, GraphStoreError>(())
    })?;

    // create rel array
    let mut builder = RelArrayBuilder::with_capacity(len);

    for (i, rel_id) in rel_ids.iter().enumerate() {
        let rel_ref = RelValueRef {
            id: *rel_id,
            reltype: rtype,
            start_id: start.get_unchecked(i),
            end_id: end.get_unchecked(i),
            props: prop.get(i).unwrap_or(empty_prop_ref),
        };
        builder.push(Some(rel_ref));
    }

    Ok(builder.finish())
}

pub(crate) fn rel_iter_for_node(
    tx: &Transaction,
    node_id: NodeId,
    dir: SemanticDirection,
    rtypes: &[TokenId],
) -> Result<RelIterForNode, GraphStoreError> {
    let entries = tx.with_rw_txn(|txn| {
        let graph_db = tx._engine.graph.graph;

        let header_key = GraphKeyCodec::encode_header_key(node_id);
        let header_val = match graph_db.get(txn, &header_key).map_err(GraphStoreError::Heed)? {
            Some(v) => v,
            None => {
                return Ok(Vec::new());
            }
        };

        let (is_packed, _labels, _prop_bytes, remaining) =
            AdaptiveNodeCodec::decode_header(header_val).map_err(GraphStoreError::internal)?;

        let (out_edges, in_edges) = if is_packed {
            AdaptiveNodeCodec::decode_packed_edges(remaining).map_err(GraphStoreError::internal)?
        } else {
            let out_key = GraphKeyCodec::encode_out_edges_key(node_id);
            let in_key = GraphKeyCodec::encode_in_edges_key(node_id);

            let out = match graph_db.get(txn, &out_key).map_err(GraphStoreError::Heed)? {
                Some(data) => AdaptiveNodeCodec::decode_edge_list(data).map_err(GraphStoreError::internal)?,
                None => Vec::new(),
            };
            let inc = match graph_db.get(txn, &in_key).map_err(GraphStoreError::Heed)? {
                Some(data) => AdaptiveNodeCodec::decode_edge_list(data).map_err(GraphStoreError::internal)?,
                None => Vec::new(),
            };
            (out, inc)
        };

        let mut entries = Vec::new();
        match dir {
            SemanticDirection::Outgoing => {
                for e in out_edges {
                    entries.push((e, RelDirection::Out));
                }
            }
            SemanticDirection::Incoming => {
                for e in in_edges {
                    entries.push((e, RelDirection::In));
                }
            }
            SemanticDirection::Both => {
                for e in out_edges {
                    entries.push((e, RelDirection::Out));
                }
                for e in in_edges {
                    entries.push((e, RelDirection::In));
                }
            }
        }
        Ok::<_, GraphStoreError>(entries)
    })?;

    Ok(RelIterForNode {
        entries,
        pos: 0,
        from_id: node_id,
        rtypes: rtypes.into(),
    })
}

pub trait NodeIdContainer: Sized {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn get_unchecked(&self, index: usize) -> NodeId;
    fn valid_map(&self) -> &BitVec;
}

impl NodeIdContainer for NodeArray {
    fn len(&self) -> usize {
        <Self as Array>::len(self)
    }

    fn get_unchecked(&self, index: usize) -> NodeId {
        <Self as Array>::get(self, index).unwrap().id
    }

    fn valid_map(&self) -> &BitVec {
        self.valid_map()
    }
}

impl NodeIdContainer for VirtualNodeArray {
    fn len(&self) -> usize {
        <Self as Array>::len(self)
    }

    fn get_unchecked(&self, index: usize) -> NodeId {
        <Self as Array>::get(self, index).unwrap()
    }

    fn valid_map(&self) -> &BitVec {
        self.valid_map()
    }
}

pub struct RelIterForNode {
    entries: Vec<(EdgeEntry, RelDirection)>,
    pos: usize,
    from_id: NodeId,
    rtypes: Box<[TokenId]>,
}

impl Iterator for RelIterForNode {
    // from, dir, reltype, to, rel_id, property bytes
    #[allow(clippy::type_complexity)]
    type Item = Result<(NodeId, RelDirection, TokenId, NodeId, RelationshipId, Box<[u8]>), GraphStoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.entries.len() {
            let (entry, dir) = &self.entries[self.pos];
            self.pos += 1;

            if !self.rtypes.is_empty() && !self.rtypes.contains(&entry.rel_type_id) {
                continue;
            }

            // Return (from_id, dir, reltype, to_id, rel_id, props)
            // where from_id = the node we're iterating, to_id = the other node.
            // The caller (expand executor) computes the relationship's actual
            // start_id/end_id based on direction.
            return Some(Ok((
                self.from_id,
                *dir,
                entry.rel_type_id,
                entry.other_node_id,
                entry.rel_id,
                entry.prop_bytes.to_vec().into_boxed_slice(),
            )));
        }
        None
    }
}
