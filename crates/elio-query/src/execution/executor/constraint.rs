//! Constraint checking and index management utilities
//!
//! This module provides reusable functions for:
//! - Checking unique constraints before data modifications
//! - Updating unique indexes after data modifications

use std::backtrace::Backtrace;
use std::sync::Arc;

use elio_common::TokenKind;
use elio_common::array::{Array, NodeArray, StructArray};
use elio_common::mapb::IndexKeyCodec;
use elio_storage::constraint::{ConstraintKind, ConstraintMeta};
use elio_storage::graph::GraphStore;
use elio_storage::transaction::TransactionImpl;

use crate::execution::error::ExecError;

/// Collected constraints for a set of labels
pub struct LabelConstraints {
    /// List of (label_id, constraint) pairs
    pub constraints: Vec<(u16, ConstraintMeta)>,
}

impl LabelConstraints {
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }
}

/// Fetch all UNIQUE/NODE KEY constraints for the given labels
pub fn fetch_constraints_for_labels(
    store: &Arc<GraphStore>,
    tx: &Arc<TransactionImpl>,
    labels: &[Arc<str>],
) -> Result<LabelConstraints, ExecError> {
    let mut constraints = Vec::new();

    for label in labels {
        if let Some(label_id) = store.token_store().get_label_id(label) {
            let label_constraints = tx.get_constraints_for_label(label_id)?;
            for c in label_constraints {
                if matches!(c.constraint_kind, ConstraintKind::Unique | ConstraintKind::NodeKey) {
                    constraints.push((label_id, c));
                }
            }
        }
    }

    Ok(LabelConstraints { constraints })
}

/// Check unique constraints for a batch of properties before creating/updating nodes
///
/// Returns an error if any constraint would be violated.
/// For NODE KEY constraints, also checks that all properties exist and are not NULL.
pub fn check_unique_constraints(
    store: &Arc<GraphStore>,
    tx: &Arc<TransactionImpl>,
    label_constraints: &LabelConstraints,
    props: &StructArray,
) -> Result<(), ExecError> {
    for (label_id, constraint) in &label_constraints.constraints {
        let is_node_key = matches!(constraint.constraint_kind, ConstraintKind::NodeKey);

        for row_idx in 0..props.len() {
            let extraction_result =
                extract_property_values_with_null_check(store, props, &constraint.property_key_ids, row_idx);

            match extraction_result {
                PropertyExtractionResult::Success(prop_values) => {
                    // Check if this value already exists in the index
                    let prop_value_refs: Vec<&[u8]> = prop_values.iter().map(|v| v.as_slice()).collect();
                    if tx.unique_index_exists(*label_id, &constraint.property_key_ids, &prop_value_refs)? {
                        return Err(ExecError::ConstraintViolation {
                            constraint: constraint.name.to_string(),
                            reason: "Node with this property value already exists".to_string(),
                            trace: Backtrace::capture(),
                        });
                    }
                }
                PropertyExtractionResult::MissingProperty(prop_name) => {
                    // For NODE KEY, missing property is an error
                    if is_node_key {
                        return Err(ExecError::ConstraintViolation {
                            constraint: constraint.name.to_string(),
                            reason: format!("Property '{}' is required by NODE KEY constraint", prop_name),
                            trace: Backtrace::capture(),
                        });
                    }
                    // For UNIQUE, skip rows with missing properties
                }
                PropertyExtractionResult::NullValue(prop_name) => {
                    // For NODE KEY, NULL value is an error
                    if is_node_key {
                        return Err(ExecError::ConstraintViolation {
                            constraint: constraint.name.to_string(),
                            reason: format!("Property '{}' cannot be NULL for NODE KEY constraint", prop_name),
                            trace: Backtrace::capture(),
                        });
                    }
                    // For UNIQUE, skip rows with NULL values
                }
            }
        }
    }

    Ok(())
}

/// Update unique indexes after creating nodes
pub fn update_unique_indexes(
    store: &Arc<GraphStore>,
    tx: &Arc<TransactionImpl>,
    label_constraints: &LabelConstraints,
    props: &StructArray,
    nodes: &NodeArray,
) -> Result<(), ExecError> {
    for (label_id, constraint) in &label_constraints.constraints {
        for (row_idx, node_opt) in nodes.iter().enumerate() {
            if let Some(node) = node_opt {
                let prop_values = extract_property_values(store, props, &constraint.property_key_ids, row_idx);
                if let Some(prop_values) = prop_values {
                    let prop_value_refs: Vec<&[u8]> = prop_values.iter().map(|v| v.as_slice()).collect();
                    tx.put_unique_index(*label_id, &constraint.property_key_ids, &prop_value_refs, node.id)?;
                }
            }
        }
    }

    Ok(())
}

/// Result of property extraction for constraint checking
enum PropertyExtractionResult {
    /// All properties found and non-null
    Success(Vec<Vec<u8>>),
    /// A property is missing from the node
    MissingProperty(String),
    /// A property has NULL value
    NullValue(String),
}

/// Extract property values for constraint checking with NULL detection
fn extract_property_values_with_null_check(
    store: &Arc<GraphStore>,
    props: &StructArray,
    prop_key_ids: &[u16],
    row_idx: usize,
) -> PropertyExtractionResult {
    let mut prop_values: Vec<Vec<u8>> = Vec::new();

    for prop_key_id in prop_key_ids {
        let prop_name = match store.token_store().get_token_val(*prop_key_id, TokenKind::PropertyKey) {
            Ok(name) => name,
            Err(_) => return PropertyExtractionResult::MissingProperty(format!("property_id:{}", prop_key_id)),
        };

        let field = props.fields().iter().find(|(k, _)| k.as_ref() == prop_name.as_ref());

        match field {
            None => return PropertyExtractionResult::MissingProperty(prop_name.to_string()),
            Some((_, arr)) => match arr.get(row_idx) {
                None => return PropertyExtractionResult::NullValue(prop_name.to_string()),
                Some(val) => {
                    let encoded = IndexKeyCodec::encode_single(&val);
                    prop_values.push(encoded);
                }
            },
        }
    }

    PropertyExtractionResult::Success(prop_values)
}

/// Extract property values for constraint checking (simple version, returns None for missing/null)
fn extract_property_values(
    store: &Arc<GraphStore>,
    props: &StructArray,
    prop_key_ids: &[u16],
    row_idx: usize,
) -> Option<Vec<Vec<u8>>> {
    match extract_property_values_with_null_check(store, props, prop_key_ids, row_idx) {
        PropertyExtractionResult::Success(values) => Some(values),
        _ => None,
    }
}
