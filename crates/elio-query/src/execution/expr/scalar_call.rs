use std::sync::Arc;

use bitvec::vec::BitVec;
use educe::Educe;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{ArrayImpl, ArrayRef};
use elio_common::data_type::LogicalType;

use crate::execution::error::EvalError;
use crate::execution::expr::{EvalCtx, Expression, SharedExpression};

pub trait ScalarFunctionExec: Send + Sync + 'static {
    fn execute(&self, inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError>;
}

/// For simple function, we just use the pure function pointer
impl<T: Fn(&[ArrayRef], &BitVec, usize) -> Result<ArrayImpl, EvalError>> ScalarFunctionExec for T
where
    T: Send + Sync + 'static,
{
    fn execute(&self, inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
        (self)(inputs, vis, len)
    }
}

#[derive(Educe)]
#[educe(Debug)]
pub struct ScalarCallExpr {
    pub inputs: Vec<SharedExpression>,
    #[educe(Debug(ignore))]
    pub function_exec: Arc<dyn ScalarFunctionExec>,
    pub typ: LogicalType,
}

impl Expression for ScalarCallExpr {
    fn typ(&self) -> &LogicalType {
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
        let res = self.function_exec.execute(&args, vis, len)?;
        Ok(res.into())
    }
}
