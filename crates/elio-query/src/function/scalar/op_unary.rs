//! Unary op
//!  - unary_add
//!  - unary_subtract

use bitvec::prelude::*;
use elio_common::array::*;
use elio_common::data_type::LogicalType;
use elio_common::scalar::*;
use elio_expr_macros::cypher_func;

use crate::define_function;
use crate::execution::error::EvalError;
use crate::function::scalar::FunctionRegistry;

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

pub(crate) fn register(registry: &mut FunctionRegistry) {
    let add = define_function!(
        name: "unary_add",
        impls: [
            {args: [{anyof INTEGER | FLOAT}], ret: ANY, func: any_unary_add_batch },
            {args: [{exact ANY}], ret: ANY, func: any_unary_add_batch}
        ],
        is_agg: false
    );
    registry.insert(add);

    let sub = define_function!(
        name: "unary_substract",
        impls: [
            {args: [{anyof INTEGER | FLOAT}], ret: ANY, func: any_unary_subtract_batch},
            {args: [{exact ANY}], ret: ANY, func: any_unary_subtract_batch}
        ],
        is_agg: false
    );
    registry.insert(sub);
}
