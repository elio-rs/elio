use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{Array, ArrayImpl, NodeArray, NodeArrayBuilder, VirtualNodeArray, VirtualNodeArrayBuilder};
use elio_common::scalar::{NodeValueRef, StructValue};
use elio_common::TokenKind;

use crate::codec::{LabelIndexCodec, NodeFormat};
use crate::error::GraphStoreError;
use crate::id::IdStore;
use crate::kv::{cf_data, CfKind};
use crate::token::TokenStore;
use crate::transaction::{DataChunkIterator, NodeScanOptions, Transaction};

// props only accept the following array types
// - StructArray
pub(crate) fn batch_node_create(
    tx: &Transaction,
    dict: &IdStore,
    token: &TokenStore,
    labels: &[Arc<str>],
    props: &ArrayImpl,
) -> Result<NodeArray, GraphStoreError> {
    let len = props.len();

    // props
    let props = props
        .as_struct()
        .ok_or(GraphStoreError::type_mismatch("Expected struct array"))?;

    // create label ids
    let label_ids = labels
        .iter()
        .map(|l| token.get_or_create_token(l, TokenKind::Label))
        .collect::<Result<Vec<_>, _>>()?;

    // create property key names
    let token_ids = props
        .fields()
        .iter()
        .map(|(k, _)| token.get_or_create_token(k, TokenKind::PropertyKey))
        .collect::<Result<Vec<_>, _>>()?;

    // allocate node id for the batch
    let node_ids = dict.batch_node_id(len)?;

    // create node fields for the batch
    let mut keys = Vec::with_capacity(len);
    let mut values = Vec::with_capacity(len);

    for (node_id, prop) in node_ids.iter().zip(props.iter()) {
        let key = NodeFormat::encode_node_key(*node_id);
        keys.push(key);
        // labels and props must not be null
        let prop = prop.unwrap();
        let value = NodeFormat::encode_node_value(&label_ids, &token_ids, prop).map_err(GraphStoreError::internal)?;
        values.push(value);
    }

    // buffer writes via transaction
    for (k, v) in keys.iter().zip(values.iter()) {
        tx.put(CfKind::Data, k.to_vec(), v.to_vec());
    }
    // write label index entries
    for node_id in node_ids.iter() {
        for label_id in label_ids.iter() {
            let label_key = LabelIndexCodec::encode_key(*label_id, *node_id);
            tx.put(
                CfKind::Data,
                label_key.to_vec(),
                crate::kv::EMPTY_VALUE_SENTINEL.to_vec(),
            );
        }
    }

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
) -> Result<Box<dyn DataChunkIterator>, GraphStoreError> {
    let entries = tx.prefix_scan(CfKind::Data, cf_data::NODE_KEY_PREFIX);
    Ok(Box::new(VecNodeIterator {
        entries: entries.into_iter(),
        opts,
    }))
}

pub(crate) fn batch_node_scan_by_label(
    tx: &Transaction,
    label_id: elio_common::LabelId,
    opts: NodeScanOptions,
) -> Result<Box<dyn DataChunkIterator>, GraphStoreError> {
    let prefix = LabelIndexCodec::encode_prefix(label_id);
    let entries = tx.prefix_scan(CfKind::Data, &prefix);
    Ok(Box::new(VecLabelNodeIterator {
        entries: entries.into_iter(),
        prefix,
        opts,
    }))
}

// null node id handling:
// two pass:
// 1. pass over the valid map of node id array, collect the valid node id
// 2. issue the batch read for valid node id
// 3. pass over the valid map, if the node id is valid, then push the result to the builder, else push None
pub(crate) fn batch_materialize_node(
    tx: &Transaction,
    token: &TokenStore,
    node_ids: &VirtualNodeArray,
    vis: &BitVec,
) -> Result<NodeArray, GraphStoreError> {
    let mut builder = NodeArrayBuilder::with_capacity(node_ids.len());

    // Collect valid node keys and read them individually
    let mut valid_entries: Vec<(usize, Option<Vec<u8>>)> = Vec::new();
    for (idx, node_id) in node_ids.iter().enumerate() {
        match (vis[idx], node_id) {
            (true, Some(node_id)) => {
                let key = NodeFormat::encode_node_key(node_id);
                let val = tx.get(CfKind::Data, &key)?;
                valid_entries.push((idx, val));
            }
            _ => continue,
        }
    }

    let mut entry_iter = valid_entries.into_iter();
    let mut next_entry = entry_iter.next();

    for (idx, node_id) in node_ids.iter().enumerate() {
        if !vis[idx] || node_id.is_none() {
            builder.push(None);
            continue;
        }

        let node_id = node_id.unwrap();
        // Consume the next valid entry
        let (_, val) = next_entry.take().unwrap();
        next_entry = entry_iter.next();

        if let Some(val) = val {
            let (label_ids, prop_map) = NodeFormat::decode_node_value(&val).map_err(GraphStoreError::internal)?;

            let label_strs = label_ids
                .iter()
                .map(|id| token.get_token_val(id, TokenKind::Label))
                .collect::<Result<Vec<_>, _>>()?;

            let struct_value = {
                let mut fileds = vec![];
                for entry in prop_map.iter() {
                    let key = token.get_token_val(entry.key(), TokenKind::PropertyKey)?;
                    fileds.push((key, entry.value().to_owned_scalar()));
                }
                StructValue::new(fileds)
            };

            let node_ref = NodeValueRef {
                id: node_id,
                labels: &label_strs,
                props: struct_value.as_scalar_ref(),
            };
            builder.push(Some(node_ref));
        } else {
            // if val does not exists, then push None
            tracing::warn!("node id {} not found", node_id);
            builder.push(None);
        }
    }

    Ok(builder.finish())
}

/// Iterator over eagerly-collected node scan results.
pub struct VecNodeIterator {
    entries: std::vec::IntoIter<(Vec<u8>, Vec<u8>)>,
    opts: NodeScanOptions,
}

impl DataChunkIterator for VecNodeIterator {
    fn next_batch(&mut self) -> Result<Option<DataChunk>, GraphStoreError> {
        let mut builder = VirtualNodeArrayBuilder::with_capacity(self.opts.batch_size);
        for _ in 0..self.opts.batch_size {
            if let Some((key, _val)) = self.entries.next() {
                if !key.starts_with(cf_data::NODE_KEY_PREFIX) {
                    break;
                }
                let node_id = NodeFormat::decode_node_key(&key);
                builder.push(Some(node_id));
            } else {
                break;
            }
        }

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

/// Iterator over eagerly-collected label-scan results.
pub struct VecLabelNodeIterator {
    entries: std::vec::IntoIter<(Vec<u8>, Vec<u8>)>,
    prefix: bytes::Bytes,
    opts: NodeScanOptions,
}

impl DataChunkIterator for VecLabelNodeIterator {
    fn next_batch(&mut self) -> Result<Option<DataChunk>, GraphStoreError> {
        let mut builder = VirtualNodeArrayBuilder::with_capacity(self.opts.batch_size);
        for _ in 0..self.opts.batch_size {
            if let Some((key, _val)) = self.entries.next() {
                if !key.starts_with(&self.prefix) {
                    break;
                }
                let (_label_id, node_id) = LabelIndexCodec::decode_key(&key);
                builder.push(Some(node_id));
            } else {
                break;
            }
        }

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
