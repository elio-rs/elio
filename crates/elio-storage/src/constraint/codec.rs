use std::sync::Arc;

use bytes::{BufMut, Bytes, BytesMut};
use elio_common::{LabelId, NodeId, PropertyKeyId};

use crate::cf_constraint;

/// Constraint entity type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EntityType {
    Node = 0,
    Relationship = 1,
}

impl EntityType {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(EntityType::Node),
            1 => Some(EntityType::Relationship),
            _ => None,
        }
    }
}

/// Constraint kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConstraintKind {
    Unique = 0,
    NodeKey = 1,
    NotNull = 2,
}

impl ConstraintKind {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(ConstraintKind::Unique),
            1 => Some(ConstraintKind::NodeKey),
            2 => Some(ConstraintKind::NotNull),
            _ => None,
        }
    }
}

/// Constraint metadata stored in the database
#[derive(Debug, Clone)]
pub struct ConstraintMeta {
    pub name: Arc<str>,
    pub entity_type: EntityType,
    pub label_id: LabelId,
    pub constraint_kind: ConstraintKind,
    pub property_key_ids: Vec<PropertyKeyId>,
}

/// Codec for constraint metadata
pub struct ConstraintCodec;

impl ConstraintCodec {
    /// Encode constraint metadata key
    /// Format: | prefix (1B) | name_len (2B) | name |
    pub fn encode_meta_key(name: &str) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(cf_constraint::CONSTRAINT_META_PREFIX);
        buf.put_u16_le(name.len() as u16);
        buf.put_slice(name.as_bytes());
        buf.freeze()
    }

    /// Decode constraint name from meta key
    pub fn decode_meta_key(buf: &[u8]) -> Option<String> {
        if buf.len() < 3 || buf[0] != cf_constraint::CONSTRAINT_META_PREFIX {
            return None;
        }
        let name_len = u16::from_le_bytes([buf[1], buf[2]]) as usize;
        if buf.len() < 3 + name_len {
            return None;
        }
        String::from_utf8(buf[3..3 + name_len].to_vec()).ok()
    }

    /// Encode constraint metadata value
    /// Format: | entity_type (1B) | label_id (2B) | kind (1B) | prop_count (2B) | prop_ids... |
    pub fn encode_meta_value(meta: &ConstraintMeta) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(meta.entity_type as u8);
        buf.put_u16_le(meta.label_id);
        buf.put_u8(meta.constraint_kind as u8);
        buf.put_u16_le(meta.property_key_ids.len() as u16);
        for prop_id in &meta.property_key_ids {
            buf.put_u16_le(*prop_id);
        }
        buf.freeze()
    }

    /// Decode constraint metadata value
    pub fn decode_meta_value(name: String, buf: &[u8]) -> Option<ConstraintMeta> {
        if buf.len() < 6 {
            return None;
        }
        let entity_type = EntityType::from_u8(buf[0])?;
        let label_id = u16::from_le_bytes([buf[1], buf[2]]);
        let constraint_kind = ConstraintKind::from_u8(buf[3])?;
        let prop_count = u16::from_le_bytes([buf[4], buf[5]]) as usize;

        if buf.len() < 6 + prop_count * 2 {
            return None;
        }

        let mut property_key_ids = Vec::with_capacity(prop_count);
        for i in 0..prop_count {
            let offset = 6 + i * 2;
            let prop_id = u16::from_le_bytes([buf[offset], buf[offset + 1]]);
            property_key_ids.push(prop_id);
        }

        Some(ConstraintMeta {
            name: name.into(),
            entity_type,
            label_id,
            constraint_kind,
            property_key_ids,
        })
    }

    /// Encode label-to-constraint mapping key
    /// Format: | prefix (1B) | label_id (2B) | name_len (2B) | name |
    pub fn encode_label_constraint_key(label_id: LabelId, name: &str) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(cf_constraint::LABEL_CONSTRAINT_PREFIX);
        buf.put_u16_le(label_id);
        buf.put_u16_le(name.len() as u16);
        buf.put_slice(name.as_bytes());
        buf.freeze()
    }

    /// Encode label-to-constraint prefix for iteration
    pub fn encode_label_constraint_prefix(label_id: LabelId) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(cf_constraint::LABEL_CONSTRAINT_PREFIX);
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
        buf.put_u8(cf_constraint::UNIQUE_INDEX_PREFIX);
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
