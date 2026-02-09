use std::sync::Arc;

use elio_common::array::chunk::DataChunk;
use elio_common::array::{ArrayRef, PhysicalType};
use elio_common::data_type::DataType;
use elio_common::scalar::{ListValue, ScalarVTable};

use crate::error::EvalError;
use crate::impl_::{EvalCtx, Expression, SharedExpression};

/// Expression that creates a list from multiple element expressions.
///
/// For each row in the input chunk, this expression evaluates all element
/// expressions and combines them into a single list value.
#[derive(Debug)]
pub struct CreateListExpr {
    /// Element expressions - each produces one element per row
    pub elements: Vec<SharedExpression>,
    /// The result type (List<T>)
    pub typ: DataType,
    /// Physical type for building arrays
    pub physical_type: PhysicalType,
}

impl CreateListExpr {
    pub fn new(elements: Vec<SharedExpression>, typ: DataType) -> Self {
        let physical_type = typ.physical_type();
        Self {
            elements,
            typ,
            physical_type,
        }
    }
}

impl Expression for CreateListExpr {
    fn typ(&self) -> &DataType {
        &self.typ
    }

    fn eval_batch(&self, chunk: &DataChunk, ctx: &dyn EvalCtx) -> Result<ArrayRef, EvalError> {
        let len = chunk.visible_row_len();

        // Evaluate all element expressions
        // Note: The returned arrays are already compacted to visible_row_len()
        let element_arrays: Vec<ArrayRef> = self
            .elements
            .iter()
            .map(|expr| expr.eval_batch(chunk, ctx))
            .collect::<Result<Vec<_>, _>>()?;

        // Build the output list array
        let mut builder = self.physical_type.array_builder(len).into_list().unwrap();

        for idx in chunk.visibility().iter_ones() {
            let mut items = Vec::with_capacity(element_arrays.len());
            for arr in &element_arrays {
                // arr.get() returns None for null values
                // unwrap_or_default() produces ScalarValue::Unknown (null placeholder)
                // TODO(pgao): we should handle nulls correctly
                let item = arr.get(idx).map(|v| v.to_owned_scalar());
                items.push(item.unwrap_or_default());
            }
            let list_value = ListValue::new(items);
            builder.push(Some(list_value.as_scalar_ref()));
        }

        Ok(Arc::new(builder.finish().into()))
    }
}
