use bytes::{BufMut, Bytes, BytesMut};
use elio_common::LabelId;
use elio_common::catalog::ConstraintCatalogEntry;

use crate::kv::cf_catalog;

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
