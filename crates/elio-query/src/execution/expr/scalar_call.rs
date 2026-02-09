use bitvec::vec::BitVec;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{ArrayImpl, ArrayRef};
use elio_common::data_type::DataType;

use crate::execution::error::EvalError;
use crate::execution::expr::{EvalCtx, Expression, SharedExpression};

// used to invoke the function call
pub type ScalarInvocation = fn(&[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError>;

// scalar function call expression
#[derive(Debug)]
pub struct ScalarCallExpr {
    pub inputs: Vec<SharedExpression>,
    pub func: ScalarInvocation,
    pub typ: DataType,
}

impl Expression for ScalarCallExpr {
    fn typ(&self) -> &DataType {
        &self.typ
    }

    fn eval_batch(&self, chunk: &DataChunk, ctx: &dyn EvalCtx) -> Result<ArrayRef, EvalError> {
        let args = self
            .inputs
            .iter()
            .map(|e| e.eval_batch(chunk, ctx))
            .collect::<Result<Vec<_>, _>>()?;
        let vis = chunk.visibility();
        let len = chunk.len();
        let res = (self.func)(&args, vis, len)?;
        Ok(res.into())
    }
}
