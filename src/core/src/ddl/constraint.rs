//! Constraint DDL operations
//!
//! Handles CREATE CONSTRAINT and DROP CONSTRAINT statements.

use std::backtrace::Backtrace;
use std::collections::HashSet;
use std::sync::Arc;

use elio_common::array::Array;
use elio_common::mapb::IndexKeyCodec;
use elio_exec::error::ExecError;
use elio_parser::ast;
use elio_storage::constraint::{ConstraintKind, ConstraintMeta, EntityType};
use elio_storage::graph::{GraphStore, TransactionMode};
use elio_storage::transaction::NodeScanOptions;

use crate::error::Error;

/// Execute CREATE CONSTRAINT statement
///
/// This function:
/// 1. Validates the constraint doesn't already exist
/// 2. Acquires an exclusive lock on the label
/// 3. Scans existing data to check for constraint violations
/// 4. Builds the unique index for existing data
/// 5. Stores the constraint metadata
pub fn create_constraint(store: &Arc<GraphStore>, constraint: &ast::CreateConstraint) -> Result<(), Error> {
    let tx = store.transaction(TransactionMode::ReadWrite);

    // 1. Check if constraint already exists
    if tx.constraint_exists(&constraint.name)? {
        if constraint.if_not_exists {
            return Ok(());
        }
        return Err(Error::ConstraintAlreadyExists(constraint.name.clone()));
    }

    // 2. Extract label and properties from the constraint
    let (entity_type, label_name) = match &constraint.entity {
        ast::ConstraintEntity::Node { label, .. } => (EntityType::Node, label.clone()),
        ast::ConstraintEntity::Relationship { rel_type, .. } => (EntityType::Relationship, rel_type.clone()),
    };

    let properties: Vec<String> = match &constraint.constraint_type {
        ast::ConstraintType::Unique { properties } => properties.iter().map(|p| p.property.clone()).collect(),
        ast::ConstraintType::NodeKey { properties } => properties.iter().map(|p| p.property.clone()).collect(),
        ast::ConstraintType::NotNull { property } => vec![property.property.clone()],
    };

    let constraint_kind = match &constraint.constraint_type {
        ast::ConstraintType::Unique { .. } => ConstraintKind::Unique,
        ast::ConstraintType::NodeKey { .. } => ConstraintKind::NodeKey,
        ast::ConstraintType::NotNull { .. } => ConstraintKind::NotNull,
    };

    // 3. Get or create token IDs
    let label_id = store.token_store().get_or_create_label_id(&label_name)?;
    let prop_key_ids: Vec<u16> = properties
        .iter()
        .map(|p| store.token_store().get_or_create_property_key_id(p))
        .collect::<Result<_, _>>()?;

    // 4. Acquire exclusive lock for the label
    let _lock = store.acquire_label_write(label_id);

    // 5. For UNIQUE/NODE KEY constraints, scan existing data and check for duplicates
    if matches!(constraint_kind, ConstraintKind::Unique | ConstraintKind::NodeKey) {
        backfill_unique_index(
            store,
            &tx,
            &label_name,
            label_id,
            &properties,
            &prop_key_ids,
            &constraint.name,
        )?;
    }

    // 6. Store constraint metadata
    let meta = ConstraintMeta {
        name: constraint.name.as_str().into(),
        entity_type,
        label_id,
        constraint_kind,
        property_key_ids: prop_key_ids,
    };
    tx.put_constraint(&meta)?;

    // 7. Commit the transaction
    tx.commit()?;

    Ok(())
}

/// Backfill unique index for existing data
///
/// Scans all nodes with the given label, checks for duplicate values,
/// and builds the unique index.
fn backfill_unique_index(
    _store: &Arc<GraphStore>,
    tx: &Arc<elio_storage::transaction::TransactionImpl>,
    label_name: &str,
    label_id: u16,
    properties: &[String],
    prop_key_ids: &[u16],
    constraint_name: &str,
) -> Result<(), Error> {
    // Track seen values to detect duplicates
    let mut seen_values: HashSet<Vec<u8>> = HashSet::new();

    // Scan all nodes
    let opts = NodeScanOptions { batch_size: 1024 };
    let mut iter = tx.node_scan(opts)?;

    while let Some(chunk) = iter.next_batch()? {
        // Materialize nodes to get their properties
        let vis = chunk.visibility().clone();
        let column = chunk.column(0);
        let node_ids = column.as_virtual_node().unwrap();
        let nodes = tx.materialize_node(node_ids, &vis)?;

        for node_opt in nodes.iter() {
            if let Some(node) = node_opt {
                // Check if node has the required label
                let has_label = node.labels.iter().any(|l: &Arc<str>| l.as_ref() == label_name);
                if !has_label {
                    continue;
                }

                // Extract property values for the constraint
                let prop_values = extract_property_values(&node.props, properties);
                let Some(prop_values) = prop_values else {
                    continue;
                };

                // Create a composite key for the hash set
                let composite_key: Vec<u8> = prop_values.concat();

                // Check for duplicates
                if !seen_values.insert(composite_key.clone()) {
                    return Err(ExecError::ConstraintViolation {
                        constraint: constraint_name.to_string(),
                        reason: format!(
                            "Duplicate value found for property {:?} on label {}",
                            properties, label_name
                        ),
                        trace: Backtrace::capture(),
                    }
                    .into());
                }

                // Build the unique index
                let prop_value_refs: Vec<&[u8]> = prop_values.iter().map(|v| v.as_slice()).collect();
                tx.put_unique_index(label_id, prop_key_ids, &prop_value_refs, node.id)?;
            }
        }
    }

    Ok(())
}

/// Extract property values from a node's properties
fn extract_property_values(
    props: &elio_common::scalar::StructValueRef<'_>,
    property_names: &[String],
) -> Option<Vec<Vec<u8>>> {
    let mut prop_values: Vec<Vec<u8>> = Vec::new();

    for prop_name in property_names {
        let found = props
            .iter()
            .find(|(k, _): &(&Arc<str>, _)| k.as_ref() == prop_name.as_str());

        if let Some((_, value)) = found {
            // Encode the value using IndexKeyCodec
            let encoded = IndexKeyCodec::encode_single(&value);
            prop_values.push(encoded);
        } else {
            return None;
        }
    }

    Some(prop_values)
}

/// Execute DROP CONSTRAINT statement
pub fn drop_constraint(store: &Arc<GraphStore>, constraint: &ast::DropConstraint) -> Result<(), Error> {
    let tx = store.transaction(TransactionMode::ReadWrite);

    // Check if constraint exists
    let meta = tx.get_constraint(&constraint.name)?;
    if meta.is_none() {
        if constraint.if_exists {
            return Ok(());
        }
        return Err(Error::ConstraintNotFound(constraint.name.clone()));
    }

    // Delete the constraint
    tx.delete_constraint(&constraint.name)?;

    // TODO: Delete all unique index entries for this constraint
    // This would require scanning all index entries with this constraint's prefix

    // Commit the transaction
    tx.commit()?;

    Ok(())
}
