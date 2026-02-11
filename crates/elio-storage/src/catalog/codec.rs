use bytes::{BufMut, Bytes, BytesMut};
use elio_common::catalog::ConstraintCatalogEntry;
use elio_common::{LabelId, NodeId, PropertyKeyId};

use crate::kv::{cf_catalog, cf_indexdata};

/// Codec for constraint metadata
pub struct ConstraintCodec;

impl ConstraintCodec {
    /// Encode constraint metadata key
    /// Format: | prefix (1B) | name_len (2B) | name |
    pub fn encode_meta_key(name: &str) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(cf_catalog::CONSTRAINT_META_PREFIX);
        buf.put_u16_le(name.len() as u16);
        buf.put_slice(name.as_bytes());
        buf.freeze()
    }

    /// Decode constraint name from meta key
    pub fn decode_meta_key(buf: &[u8]) -> Option<String> {
        if buf.len() < 3 || buf[0] != cf_catalog::CONSTRAINT_META_PREFIX {
            return None;
        }
        let name_len = u16::from_le_bytes([buf[1], buf[2]]) as usize;
        if buf.len() < 3 + name_len {
            return None;
        }
        String::from_utf8(buf[3..3 + name_len].to_vec()).ok()
    }

    /// Encode constraint metadata value
    /// Format: Json
    pub fn encode_meta_value(entry: &ConstraintCatalogEntry) -> Bytes {
        let bytes = serde_json::to_vec(entry).expect("ConstraintCatalogEntry serialization to JSON should never fail");
        Bytes::from(bytes)
    }

    /// Decode constraint metadata value
    pub fn decode_meta_value(_name: String, buf: &[u8]) -> Option<ConstraintCatalogEntry> {
        serde_json::from_slice(buf).ok()
    }

    /// Encode label-to-constraint mapping key
    /// Format: | prefix (1B) | label_id (2B) | name_len (2B) | name |
    pub fn encode_label_constraint_key(label_id: LabelId, name: &str) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(cf_catalog::LABEL_CONSTRAINT_PREFIX);
        buf.put_u16_le(label_id);
        buf.put_u16_le(name.len() as u16);
        buf.put_slice(name.as_bytes());
        buf.freeze()
    }

    /// Encode label-to-constraint prefix for iteration
    pub fn encode_label_constraint_prefix(label_id: LabelId) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(cf_catalog::LABEL_CONSTRAINT_PREFIX);
        buf.put_u16_le(label_id);
        buf.freeze()
    }
}

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
