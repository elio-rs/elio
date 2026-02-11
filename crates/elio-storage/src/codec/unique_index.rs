use bytes::{BufMut, Bytes, BytesMut};
use elio_common::{LabelId, NodeId, PropertyKeyId};

use crate::kv::cf_indexdata;

/// Codec for unique index
pub struct UniqueIndexCodec;

impl UniqueIndexCodec {
    /// Encode unique index key
    /// Format: | prefix (1B) | label_id (2B) | prop_key_id (2B) | prop_value_len (4B) | prop_value |
    ///
    /// For composite keys, prop_key_ids and values are concatenated
    pub fn encode_key(label_id: LabelId, prop_key_ids: &[PropertyKeyId], prop_values: &[&[u8]]) -> Bytes {
        assert_eq!(prop_key_ids.len(), prop_values.len());

        let mut buf = BytesMut::new();
        buf.put_u8(cf_indexdata::UNIQUE_INDEX_PREFIX);
        buf.put_u16_le(label_id);

        for (prop_key_id, prop_value) in prop_key_ids.iter().zip(prop_values.iter()) {
            buf.put_u16_le(*prop_key_id);
            buf.put_u32_le(prop_value.len() as u32);
            buf.put_slice(prop_value);
        }

        buf.freeze()
    }

    /// Encode unique index value (just the node_id)
    pub fn encode_value(node_id: NodeId) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u64_le(*node_id);
        buf.freeze()
    }

    /// Decode unique index value to node_id
    pub fn decode_value(buf: &[u8]) -> Option<NodeId> {
        if buf.len() < 8 {
            return None;
        }
        Some(NodeId::from_le_bytes(buf[0..8].try_into().ok()?))
    }
}
