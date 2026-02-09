use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{ArrayRef, PhysicalType, StructArray};
use elio_common::data_type::DataType;

use crate::error::EvalError;
use crate::impl_::{EvalCtx, Expression, SharedExpression};

#[derive(Debug)]
pub struct CreateStructExpr {
    // struct keys and values
    pub fields: Vec<(Arc<str>, SharedExpression)>,
    pub typ: DataType,
    pub physical_type: PhysicalType,
}

impl CreateStructExpr {
    pub fn new(fields: Vec<(Arc<str>, SharedExpression)>, typ: DataType) -> Self {
        let physical_type = typ.physical_type();
        Self {
            fields,
            typ,
            physical_type,
        }
    }
}

impl Expression for CreateStructExpr {
    fn typ(&self) -> &DataType {
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
