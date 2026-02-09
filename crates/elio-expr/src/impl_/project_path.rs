use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::NodeId;
use elio_common::array::{
    Array, ArrayImpl, ListArrayBuilder, RelArrayBuilder, VirtualNodeArrayBuilder, VirtualPathArray,
};
use elio_common::scalar::RelValueRef;

use super::*;

#[derive(Debug)]
pub struct ProjectPathExpr {
    // expected to be interleaved by node and rel
    pub inputs: Vec<SharedExpression>,
    // output type, expected to be virtualpath
    pub typ: DataType,
}

impl Expression for ProjectPathExpr {
    fn typ(&self) -> &DataType {
        &self.typ
    }

    fn eval_batch(&self, chunk: &DataChunk, ctx: &dyn EvalCtx) -> Result<ArrayRef, EvalError> {
        let steps = self
            .inputs
            .iter()
            .map(|expr| expr.eval_batch(chunk, ctx))
            .collect::<Result<Vec<_>, _>>()?;

        let len = steps.first().unwrap().len();

        // push nodes
        let mut node_builder = ListArrayBuilder::new(Box::new(VirtualNodeArrayBuilder::with_capacity(len).into()));
        let mut rel_builder = ListArrayBuilder::new(Box::new(RelArrayBuilder::with_capacity(len).into()));

        let mut nodes = Vec::with_capacity(steps.len() / 2 + 1);
        let mut rels = Vec::with_capacity(steps.len() / 2);
        for (i, step) in steps.iter().enumerate() {
            if i % 2 == 0 {
                match step.as_ref() {
                    ArrayImpl::VirtualNode(_) | ArrayImpl::Node(_) => nodes.push(step.clone()),
                    other => {
                        return Err(EvalError::TypeError(format!(
                            "Expected node array at step {}, found {:?}",
                            i,
                            other.physical_type()
                        )));
                    }
                }
            } else {
                match step.as_ref() {
                    ArrayImpl::Rel(_) => rels.push(step.clone()),
                    other => {
                        return Err(EvalError::TypeError(format!(
                            "Expected rel array at step {}, found {:?}",
                            i,
                            other.physical_type()
                        )));
                    }
                }
            }
        }

        fn get_nodes(node_steps: &[ArrayRef], rowid: usize) -> impl Iterator<Item = NodeId> + '_ {
            node_steps.iter().filter_map(move |step| {
                if let ArrayImpl::VirtualNode(virtual_node_array) = step.as_ref() {
                    virtual_node_array.get(rowid)
                } else if let ArrayImpl::Node(node_array) = step.as_ref() {
                    node_array.get(rowid).map(|x| x.id)
                } else {
                    None
                }
            })
        }

        fn get_rels(rel_steps: &[ArrayRef], rowid: usize) -> impl Iterator<Item = RelValueRef<'_>> + '_ {
            rel_steps.iter().filter_map(move |step| {
                if let ArrayImpl::Rel(rel_array) = step.as_ref() {
                    rel_array.get(rowid)
                } else {
                    None
                }
            })
        }

        let mut valid = BitVec::with_capacity(len);
        // build node list
        for rowid in 0..len {
            let nodes = get_nodes(&nodes, rowid);
            let rels = get_rels(&rels, rowid);
            node_builder.push_virtual_nodes(nodes);
            rel_builder.push_rels(rels);
            valid.push(true);
        }

        let path = VirtualPathArray::from_parts(Arc::new(node_builder.finish()), Arc::new(rel_builder.finish()), valid);

        Ok(Arc::new(path.into()))
    }
}
