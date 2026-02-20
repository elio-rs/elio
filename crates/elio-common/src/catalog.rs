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

fn serialize_opt_arc_str<S>(opt: &Option<Arc<str>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match opt {
        Some(arc_str) => arc_str.as_ref().serialize(serializer),
        None => serializer.serialize_none(),
    }
}

fn deserialize_opt_arc_str<'de, D>(deserializer: D) -> Result<Option<Arc<str>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.map(Arc::from))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintKind {
    Unique,
    NodeKey,
    NotNull,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IndexKind {
    Unique,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintCatalogEntry {
    #[serde(serialize_with = "serialize_arc_str", deserialize_with = "deserialize_arc_str")]
    pub name: Arc<str>,
    pub entity_kind: EntityKind,
    pub label_or_rel_id: LabelId, // label or rel id
    pub constraint_kind: ConstraintKind,
    pub property_key_ids: Vec<PropertyKeyId>,
    // Name of the backing index (e.g., for Unique/NodeKey constraints).
    // None for constraints that don't require an index (e.g., NotNull).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt_arc_str",
        deserialize_with = "deserialize_opt_arc_str"
    )]
    pub backing_index: Option<Arc<str>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexCatalogEntry {
    #[serde(serialize_with = "serialize_arc_str", deserialize_with = "deserialize_arc_str")]
    pub name: Arc<str>,
    pub label_id: LabelId,
    pub index_kind: IndexKind,
    pub property_key_ids: Vec<PropertyKeyId>,
}

/// Index hint for query optimization
#[derive(Debug, Clone)]
pub struct IndexHint {
    // Index name
    pub index_name: Arc<str>,
    // Label ID
    pub label_id: LabelId,
    // Property key IDs in the index
    pub property_key_ids: Vec<PropertyKeyId>,
}
