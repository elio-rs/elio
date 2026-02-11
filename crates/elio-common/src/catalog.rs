use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{EntityKind, LabelId, PropertyKeyId};

// Helper functions for serializing/deserializing Arc<str>
fn serialize_arc_str<S>(arc_str: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    arc_str.as_ref().serialize(serializer)
}

fn deserialize_arc_str<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(Arc::from(s))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintKind {
    Unique,
    NodeKey,
    NotNull,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintCatalogEntry {
    #[serde(serialize_with = "serialize_arc_str", deserialize_with = "deserialize_arc_str")]
    pub name: Arc<str>,
    pub entity_kind: EntityKind,
    pub label_or_rel_id: LabelId, // label or rel id
    pub constraint_kind: ConstraintKind,
    pub property_key_ids: Vec<PropertyKeyId>,
}

/// Index hint for query optimization
#[derive(Debug, Clone)]
pub struct IndexHint {
    /// Constraint/index name
    pub constraint_name: Arc<str>,
    /// Label ID
    pub label_id: LabelId,
    /// Property key IDs in the index
    pub property_key_ids: Vec<PropertyKeyId>,
}
