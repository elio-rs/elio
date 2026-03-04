//! Unary op
//!  - unary_add
//!  - unary_subtract

use std::sync::Arc;

use bitvec::prelude::*;
use elio_common::array::*;
use elio_common::data_type::LogicalType;
use elio_common::scalar::*;
use elio_expr_macros::cypher_func;

use crate::execution::error::EvalError;
use crate::function::ScalarFunctionRegistry;
use crate::function::sig::ScalarFunctionSet;
use crate::scalar_function;

#[cypher_func(batch_name = "any_unary_add_batch", sig = "(any) -> any")]
fn any_unary_add(arg: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match arg {
        ScalarRef::Integer(i) => Ok(ScalarValue::Integer(i)),
        ScalarRef::Float(f) => Ok(ScalarValue::Float(f)),
        _ => Err(EvalError::invalid_argument(
            "unary_add",
            "Integer | Float",
            arg.to_string(),
        )),
    }
}

#[cypher_func(batch_name = "any_unary_subtract_batch", sig = "(any) -> any")]
fn any_unary_subtract(arg: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match arg {
        ScalarRef::Integer(i) => Ok(ScalarValue::Integer(-i)),
        ScalarRef::Float(f) => Ok(ScalarValue::Float(-f)),
        _ => Err(EvalError::invalid_argument(
            "unary_subtract",
            "Integer | Float",
            arg.to_string(),
        )),
    }
}

pub(crate) fn register(registry: &mut ScalarFunctionRegistry) {
    let mut add = ScalarFunctionSet::new("unary_add");
    add.add_function(scalar_function!(
        "unary_add",
        [LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_unary_add_batch))
    ));
    registry.insert(add);

    let mut subtract = ScalarFunctionSet::new("unary_subtract");
    subtract.add_function(scalar_function!(
        "unary_subtract",
        [LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_unary_subtract_batch))
    ));
    registry.insert(subtract);
}
