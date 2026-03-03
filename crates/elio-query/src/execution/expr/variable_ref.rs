use elio_common::array::ArrayRef;
use elio_common::array::chunk::DataChunk;
use elio_common::data_type::LogicalType;

use crate::execution::error::EvalError;
use crate::execution::expr::{EvalCtx, Expression};

#[derive(Debug)]
pub struct VariableRefExpr {
    pub idx: usize,
    typ: LogicalType,
}

impl VariableRefExpr {
    pub fn new(idx: usize, typ: LogicalType) -> Self {
        Self { idx, typ }
    }
}

impl Expression for VariableRefExpr {
    fn typ(&self) -> &LogicalType {
        &self.typ
    }

    fn eval_batch(&self, chunk: &DataChunk, _ctx: &dyn EvalCtx) -> Result<ArrayRef, EvalError> {
        Ok(chunk.column(self.idx))
    }
}
