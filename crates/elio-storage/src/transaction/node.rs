use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::TokenKind;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{Array, ArrayImpl, NodeArray, NodeArrayBuilder, VirtualNodeArray, VirtualNodeArrayBuilder};
use elio_common::scalar::{NodeValueRef, StructValue};

use crate::cf_property;
use crate::codec::NodeFormat;
use crate::error::GraphStoreError;
use crate::transaction::{DataChunkIterator, NodeScanOptions, TransactionImpl};

// props only accept the fowlling array types
// - StructArray
pub(crate) fn batch_node_create(
    tx: &TransactionImpl,
    labels: &[Arc<str>],
    props: &ArrayImpl,
) -> Result<NodeArray, GraphStoreError> {
    let len = props.len();

    // props
    let props = props
        .as_struct()
        .ok_or(GraphStoreError::type_mismatch("Expected struct array"))?;

    // create label id s
    let label_ids = labels
        .iter()
        .map(|l| tx.token.get_or_create_token(l, TokenKind::Label))
        .collect::<Result<Vec<_>, _>>()?;

    // create property key names
    let token_ids = props
        .fields()
        .iter()
        .map(|(k, _)| tx.token.get_or_create_token(k, TokenKind::PropertyKey))
        .collect::<Result<Vec<_>, _>>()?;

    // allocate node id for the batch
    let node_ids = tx.dict.batch_node_id(len)?;

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

    // construct batch
    let cf = tx.inner._db.cf_handle(cf_property::CF_NAME).unwrap();
    let mut guard = tx.write_state.lock().unwrap();
    for (k, v) in keys.iter().zip(values.iter()) {
        guard.batch.put_cf(&cf, k, v);
    }
    drop(guard);

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
    tx: &TransactionImpl,
    opts: NodeScanOptions,
) -> Result<Box<dyn DataChunkIterator + '_>, GraphStoreError> {
    let cf_handle = tx.inner._db.cf_handle(cf_property::CF_NAME).unwrap();
    let mut readopts = rocksdb::ReadOptions::default();
    // TODO(pgao): check the behavior of prefix scan
    readopts.set_prefix_same_as_start(true);
    let mode = rocksdb::IteratorMode::From(cf_property::NODE_KEY_PREFIX, rocksdb::Direction::Forward);
    let iter = tx.inner.snapshot.iterator_cf_opt(&cf_handle, readopts, mode);
    Ok(Box::new(NodeIterator { iter, opts }))
}

// null node id handling:
// two pass:
// 1. pass over the valid map of node id array, collect the valid node id
// 2. issue the rocksdb batch read for valid node id
// 3. pass over the valid map, if the node id is valid, then push the result from rocksdb to the builder, else push None
//
// Possible optimizations:
//  1. for dense array(all the data are valid, we should handle it separately)
pub(crate) fn batch_materialize_node(
    tx: &TransactionImpl,
    node_ids: &VirtualNodeArray,
    vis: &BitVec,
) -> Result<NodeArray, GraphStoreError> {
    let cf_handle = tx.inner._db.cf_handle(cf_property::CF_NAME).unwrap();
    let mut builder = NodeArrayBuilder::with_capacity(node_ids.len());

    let mut valid_node_keys = vec![];
    for (idx, node_id) in node_ids.iter().enumerate() {
        match (vis[idx], node_id) {
            (true, Some(node_id)) => valid_node_keys.push(NodeFormat::encode_node_key(node_id)),
            _ => continue,
        }
    }

    // rocksdb batch read
    let keys_cf = valid_node_keys.iter().map(|k| (&cf_handle, k));
    let batch = tx.inner.snapshot.multi_get_cf(keys_cf);
    let mut batch_iter = batch.into_iter();

    for (idx, node_id) in node_ids.iter().enumerate() {
        if !vis[idx] || node_id.is_none() {
            builder.push(None);
            continue;
        }

        let node_id = node_id.unwrap();
        // SAFETY: rocksdb will guarantee the length of batch eq to length of valid node_ids
        let val = batch_iter.next().unwrap()?;
        if let Some(val) = val {
            // deserialize
            // TODO(pgao): lazy deserialize
            let (label_ids, prop_map) = NodeFormat::decode_node_value(&val).map_err(GraphStoreError::internal)?;

            let label_strs = label_ids
                .iter()
                .map(|id| tx.token.get_token_val(id, TokenKind::Label))
                .collect::<Result<Vec<_>, _>>()?;

            let struct_value = {
                let mut fileds = vec![];
                for entry in prop_map.iter() {
                    let key = tx.token.get_token_val(entry.key(), TokenKind::PropertyKey)?;
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

pub struct NodeIterator<'a, D: rocksdb::DBAccess> {
    iter: rocksdb::DBIteratorWithThreadMode<'a, D>,
    opts: NodeScanOptions,
}

impl<'a, D: rocksdb::DBAccess> DataChunkIterator for NodeIterator<'a, D> {
    fn next_batch(&mut self) -> Result<Option<DataChunk>, GraphStoreError> {
        let mut builder = VirtualNodeArrayBuilder::with_capacity(self.opts.batch_size);
        for _ in 0..self.opts.batch_size {
            if let Some(item) = self.iter.next() {
                let (key, _val) = item.map_err(GraphStoreError::Rocksdb)?;
                if !key.starts_with(cf_property::NODE_KEY_PREFIX) {
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
