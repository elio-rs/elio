use std::backtrace::Backtrace;
use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::array::Array;
use elio_common::catalog::ConstraintKind;
use elio_common::mapb::IndexKeyCodec;
use elio_common::scalar::ScalarRef;
use elio_common::{LabelId, PropertyKeyId, TokenKind};
use elio_storage::transaction::NodeScanOptions;
use hashbrown::HashSet;

use crate::execution::QueryContext;
use crate::execution::error::ExecError;
use crate::execution::executor::CHUNK_SIZE;

pub struct CreateConstraintExecutor {
    constraint_kind: ConstraintKind,
    label_id: LabelId,
    property_key_ids: Vec<PropertyKeyId>,
    property_names: Vec<String>,
}

impl CreateConstraintExecutor {
    pub fn new(
        constraint_kind: ConstraintKind,
        label_id: LabelId,
        property_key_ids: Vec<PropertyKeyId>,
        property_names: Vec<String>,
    ) -> Self {
        Self {
            constraint_kind,
            label_id,
            property_key_ids,
            property_names,
        }
    }

    pub fn execute(&self, qctx: &QueryContext) -> Result<(), ExecError> {
        if matches!(self.constraint_kind, ConstraintKind::Unique | ConstraintKind::NodeKey) {
            build_index(
                qctx,
                &self.constraint_kind,
                self.label_id,
                &self.property_key_ids,
                &self.property_names,
            )?;
        }
        Ok(())
    }
}

pub struct DropConstraintExecutor;

impl DropConstraintExecutor {
    pub fn execute(&self, _qctx: &QueryContext) -> Result<(), ExecError> {
        Ok(())
    }
}

/// Scans all existing nodes, validates uniqueness, and populates the index.
///
/// Uses an in-memory HashSet for duplicate detection because RocksDB write batch
/// entries are not visible to snapshot reads within the same transaction.
fn build_index(
    qctx: &QueryContext,
    constraint_kind: &ConstraintKind,
    label_id: LabelId,
    property_key_ids: &[PropertyKeyId],
    property_names: &[String],
) -> Result<(), ExecError> {
    let label_name = qctx.token_store().get_token_val(label_id, TokenKind::Label)?;
    let is_node_key = matches!(constraint_kind, ConstraintKind::NodeKey);

    let mut seen_keys: HashSet<Vec<u8>> = HashSet::new();

    let opts = NodeScanOptions { batch_size: CHUNK_SIZE };
    let mut iter = qctx.graph_store().node_scan(qctx.txn().as_ref(), opts)?;

    while let Some(chunk) = iter.next_batch()? {
        let vnode_col = chunk.column(0);
        let vnode_array = vnode_col
            .as_virtual_node()
            .expect("node_scan should return VirtualNodeArray");

        let vis = BitVec::repeat(true, vnode_array.len());
        let node_array = qctx
            .graph_store()
            .materialize_node(qctx.txn().as_ref(), vnode_array, &vis)?;

        for idx in 0..node_array.len() {
            let node = match node_array.get(idx) {
                Some(n) => n,
                None => continue,
            };

            // filter by label
            if !node.labels.iter().any(|l: &Arc<str>| l.as_ref() == label_name.as_ref()) {
                continue;
            }

            // extract property values
            match extract_node_properties(&node.props, property_names) {
                PropertyResult::AllPresent(encoded_values) => {
                    // Build composite key for in-memory dedup
                    let composite_key: Vec<u8> = encoded_values.iter().flat_map(|v| v.iter().copied()).collect();

                    if !seen_keys.insert(composite_key) {
                        return Err(ExecError::ConstraintViolation {
                            constraint: format!("constraint on :{}", label_name),
                            reason: format!("Duplicate property values for node {}", node.id),
                            trace: Backtrace::capture(),
                        });
                    }

                    // Insert index entry
                    let refs: Vec<&[u8]> = encoded_values.iter().map(|v| v.as_slice()).collect();
                    qctx.graph_store().put_unique_index(
                        qctx.txn().as_ref(),
                        label_id,
                        property_key_ids,
                        &refs,
                        node.id,
                    )?;
                }
                PropertyResult::MissingOrNull(prop_name) => {
                    if is_node_key {
                        return Err(ExecError::ConstraintViolation {
                            constraint: format!("NODE KEY constraint on :{}", label_name),
                            reason: format!("Node {} has missing or null property '{}'", node.id, prop_name),
                            trace: Backtrace::capture(),
                        });
                    }
                    // UNIQUE: skip nodes with null/missing properties
                }
            }
        }
    }

    Ok(())
}

enum PropertyResult {
    AllPresent(Vec<Vec<u8>>),
    MissingOrNull(String),
}

/// Extract and encode property values from a node's properties.
fn extract_node_properties(
    props: &elio_common::scalar::StructValueRef<'_>,
    property_names: &[String],
) -> PropertyResult {
    let mut encoded = Vec::with_capacity(property_names.len());

    for prop_name in property_names {
        match props.field_at(prop_name) {
            None | Some(ScalarRef::Null) => {
                return PropertyResult::MissingOrNull(prop_name.clone());
            }
            Some(val) => {
                encoded.push(IndexKeyCodec::encode_single(&val));
            }
        }
    }

    PropertyResult::AllPresent(encoded)
}
