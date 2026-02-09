//! Constraint metadata and operations
//!
//! This module provides:
//! - Constraint metadata storage and retrieval
//! - Unique index operations for constraint enforcement

use std::sync::Arc;

use bytes::{BufMut, Bytes, BytesMut};
use elio_common::{LabelId, NodeId, PropertyKeyId};

use crate::error::GraphStoreError;
use crate::{KvEngine, cf_constraint};

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

/// Constraint store operations
pub struct ConstraintStore {
    db: Arc<KvEngine>,
}

impl ConstraintStore {
    pub fn new(db: Arc<KvEngine>) -> Self {
        Self { db }
    }

    /// Check if a constraint with the given name exists
    pub fn constraint_exists(&self, name: &str) -> Result<bool, GraphStoreError> {
        let cf = self.db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = ConstraintCodec::encode_meta_key(name);
        Ok(self.db.get_cf(&cf, &key)?.is_some())
    }

    /// Get constraint metadata by name
    pub fn get_constraint(&self, name: &str) -> Result<Option<ConstraintMeta>, GraphStoreError> {
        let cf = self.db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = ConstraintCodec::encode_meta_key(name);
        match self.db.get_cf(&cf, &key)? {
            Some(value) => Ok(ConstraintCodec::decode_meta_value(name.to_string(), &value)),
            None => Ok(None),
        }
    }

    /// Get all constraints for a label
    pub fn get_constraints_for_label(&self, label_id: LabelId) -> Result<Vec<ConstraintMeta>, GraphStoreError> {
        let cf = self.db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let prefix = ConstraintCodec::encode_label_constraint_prefix(label_id);

        let mut constraints = Vec::new();
        let iter = self.db.prefix_iterator_cf(&cf, &prefix);

        for item in iter {
            let (key, _) = item?;
            if !key.starts_with(&prefix) {
                break;
            }

            // Extract constraint name from the key
            let name_len_offset = 3; // prefix (1) + label_id (2)
            if key.len() < name_len_offset + 2 {
                continue;
            }
            let name_len = u16::from_le_bytes([key[name_len_offset], key[name_len_offset + 1]]) as usize;
            if key.len() < name_len_offset + 2 + name_len {
                continue;
            }
            let name = String::from_utf8_lossy(&key[name_len_offset + 2..name_len_offset + 2 + name_len]).to_string();

            // Get the full constraint metadata
            if let Some(meta) = self.get_constraint(&name)? {
                constraints.push(meta);
            }
        }

        Ok(constraints)
    }

    /// Store a constraint (metadata + label mapping)
    pub fn put_constraint(&self, meta: &ConstraintMeta) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_constraint::CF_NAME).unwrap();

        // Store metadata
        let meta_key = ConstraintCodec::encode_meta_key(&meta.name);
        let meta_value = ConstraintCodec::encode_meta_value(meta);
        self.db.put_cf(&cf, &meta_key, &meta_value)?;

        // Store label-to-constraint mapping
        let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_id, &meta.name);
        self.db.put_cf(&cf, &label_key, [])?;

        Ok(())
    }

    /// Delete a constraint
    pub fn delete_constraint(&self, name: &str) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_constraint::CF_NAME).unwrap();

        // Get the constraint first to find the label_id
        if let Some(meta) = self.get_constraint(name)? {
            // Delete label-to-constraint mapping
            let label_key = ConstraintCodec::encode_label_constraint_key(meta.label_id, name);
            self.db.delete_cf(&cf, &label_key)?;
        }

        // Delete metadata
        let meta_key = ConstraintCodec::encode_meta_key(name);
        self.db.delete_cf(&cf, &meta_key)?;

        Ok(())
    }

    /// Check if a unique index entry exists
    pub fn unique_index_exists(
        &self,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<bool, GraphStoreError> {
        let cf = self.db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        Ok(self.db.get_cf(&cf, &key)?.is_some())
    }

    /// Get node_id from unique index
    pub fn get_unique_index(
        &self,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<Option<NodeId>, GraphStoreError> {
        let cf = self.db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        match self.db.get_cf(&cf, &key)? {
            Some(value) => Ok(UniqueIndexCodec::decode_value(&value)),
            None => Ok(None),
        }
    }

    /// Put unique index entry
    pub fn put_unique_index(
        &self,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
        node_id: NodeId,
    ) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        let value = UniqueIndexCodec::encode_value(node_id);
        self.db.put_cf(&cf, &key, &value)?;
        Ok(())
    }

    /// Delete unique index entry
    pub fn delete_unique_index(
        &self,
        label_id: LabelId,
        prop_key_ids: &[PropertyKeyId],
        prop_values: &[&[u8]],
    ) -> Result<(), GraphStoreError> {
        let cf = self.db.cf_handle(cf_constraint::CF_NAME).unwrap();
        let key = UniqueIndexCodec::encode_key(label_id, prop_key_ids, prop_values);
        self.db.delete_cf(&cf, &key)?;
        Ok(())
    }
}
