use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{ArrayRef, StructArray};
use elio_common::data_type::LogicalType;

use crate::execution::error::EvalError;
use crate::execution::expr::{EvalCtx, Expression, SharedExpression};

#[derive(Debug)]
pub struct CreateStructExpr {
    // struct keys and values
    pub fields: Vec<(Arc<str>, SharedExpression)>,
    pub typ: LogicalType,
}

impl CreateStructExpr {
    pub fn new(fields: Vec<(Arc<str>, SharedExpression)>, typ: LogicalType) -> Self {
        Self { fields, typ }
    }
}

impl Expression for CreateStructExpr {
    fn typ(&self) -> &LogicalType {
        &self.typ
    }

    fn eval_batch(&self, chunk: &DataChunk, ctx: &dyn EvalCtx) -> Result<ArrayRef, EvalError> {
        let mut sub_fields = vec![];
        let valid = BitVec::repeat(true, chunk.visible_row_len());

        // build sub fields
        for (name, expr) in &self.fields {
            let field = expr.eval_batch(chunk, ctx)?;
            sub_fields.push((name.clone(), field));
        }

        let output = StructArray::from_parts(sub_fields.into_boxed_slice(), valid);
        Ok(Arc::new(output.into()))
    }
}
