use elio_common::array::ArrayRef;
use elio_common::array::chunk::DataChunk;
use elio_common::data_type::DataType;

use crate::error::EvalError;
use crate::impl_::{EvalCtx, Expression};

#[derive(Debug)]
pub struct VariableRefExpr {
    pub idx: usize,
    typ: DataType,
}

impl VariableRefExpr {
    pub fn new(idx: usize, typ: DataType) -> Self {
        Self { idx, typ }
    }
}

impl Expression for VariableRefExpr {
    fn typ(&self) -> &DataType {
        &self.typ
    }

    fn eval_batch(&self, chunk: &DataChunk, _ctx: &dyn EvalCtx) -> Result<ArrayRef, EvalError> {
        Ok(chunk.column(self.idx))
    }
}
