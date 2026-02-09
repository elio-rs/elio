//! Unary op
//!  - unary_add
//!  - unary_subtract

use bitvec::prelude::*;
use elio_common::array::*;
use elio_common::scalar::*;
use expr_macros::cypher_func;

use crate::define_function;
use crate::error::EvalError;
use crate::scalar::FunctionRegistry;

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
            {args: [{anyof Integer | Float}], ret: Any, func: any_unary_add_batch },
            {args: [{exact Any}], ret: Any, func: any_unary_add_batch}
        ],
        is_agg: false
    );
    registry.insert(add);

    let sub = define_function!(
        name: "unary_substract",
        impls: [
            {args: [{anyof Integer | Float}], ret: Any, func: any_unary_subtract_batch},
            {args: [{exact Any}], ret: Any, func: any_unary_subtract_batch}
        ],
        is_agg: false
    );
    registry.insert(sub);
}
