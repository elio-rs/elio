use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::IrToken;
use elio_common::array::{ArrayImpl, BoolArray, BoolArrayBuilder, EntityArray};
use elio_common::scalar::EntityScalarRef;

use super::*;
use crate::impl_::Expression;

// Expected input types, entity:
//   - NodeArray
//   - RelArray
//
// If input is VirtualNode / VirtualRel, they should be materialized before the call of eval_batch.
#[derive(Debug)]
pub struct HasLabelExpr {
    pub entity: SharedExpression,
    // TODO(pgao): use token id
    pub label: IrToken,
}

impl Expression for HasLabelExpr {
    fn typ(&self) -> &DataType {
        &DataType::Bool
    }

    fn eval_batch(&self, chunk: &DataChunk, ctx: &dyn EvalCtx) -> Result<ArrayRef, EvalError> {
        let entity = self.entity.eval_batch(chunk, ctx)?;
        let vis = chunk.visibility();

        match entity.as_ref() {
            ArrayImpl::Node(node_array) => {
                let valid = node_array.valid_map().clone() & chunk.visibility();
                let out = label_contains(node_array, &valid, &self.label);
                Ok(Arc::new(out.into()))
            }
            ArrayImpl::Rel(rel_array) => {
                let valid = rel_array.valid_map().clone() & chunk.visibility();
                let out = label_contains(rel_array, &valid, &self.label);
                Ok(Arc::new(out.into()))
            }
            ArrayImpl::VirtualNode(vnode_array) => {
                let node = ctx.materialize_node(vnode_array, vis)?;
                let valid = vnode_array.valid_map().clone() & chunk.visibility();
                let out = label_contains(&node, &valid, &self.label);
                Ok(Arc::new(out.into()))
            }
            _ => Err(EvalError::TypeError(format!(
                "Expected node array or rel array, found {:?}",
                entity.physical_type()
            ))),
        }
    }
}

fn label_contains<E: EntityArray>(array: &E, valid: &BitVec, label: &IrToken) -> BoolArray
where
    for<'a> <E as elio_common::array::Array>::RefItem<'a>: elio_common::scalar::EntityScalarRef,
{
    let mut builder = BoolArrayBuilder::with_capacity(array.len());
    for i in 0..array.len() {
        if valid[i] {
            builder.push(Some(unsafe { array.get_unchecked(i).has_ir_label(label) }));
        } else {
            builder.push(None);
        }
    }
    builder.finish()
}
