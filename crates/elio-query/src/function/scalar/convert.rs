//! Convert functions
//!  - toBoolean(any|boolean) -> boolean
//!  - toInteger(any|boolean) -> integer
//!  - toFloat(any|boolean) -> float
//!  - toString(any|boolean) -> string
//!
//! NB: Currently we do not support integer/float/string array
//!     so the results will be an AnyArray.

use std::sync::Arc;

use bitvec::prelude::*;
use elio_common::array::*;
use elio_common::data_type::{F64, LogicalType};
use elio_common::scalar::*;
use elio_expr_macros::cypher_func;

use crate::execution::error::EvalError;
use crate::function::ScalarFunctionRegistry;
use crate::function::sig::ScalarFunctionSet;
use crate::scalar_function;

#[cypher_func(batch_name = "boolean_to_boolean_batch", sig = "(bool) -> bool")]
fn boolean_to_boolean(b: bool) -> bool {
    b
}

// Boolean | String | Integer | Float => Boolean
// if fails, return NULL
#[cypher_func(batch_name = "any_to_boolean_batch", sig = "(any) -> bool")]
fn any_to_boolean(arg: ScalarRef<'_>) -> Option<bool> {
    match arg {
        ScalarRef::Bool(b) => Some(b),
        ScalarRef::String(s) => Some(s.to_lowercase().parse::<bool>().unwrap_or(false)),
        ScalarRef::Integer(i) => Some(i != 0),
        ScalarRef::Float(f) => Some(f.0 != 0.0),
        _ => None,
    }
}

#[cypher_func(batch_name = "boolean_to_integer_batch", sig = "(bool) -> any")]
fn boolean_to_integer(b: bool) -> ScalarValue {
    if b {
        ScalarValue::Integer(1)
    } else {
        ScalarValue::Integer(0)
    }
}

// Boolean| String | Integer | Float => Integer
// if fails, return NULL
#[cypher_func(batch_name = "any_to_integer_batch", sig = "(any) -> any")]
fn any_to_integer(arg: ScalarRef<'_>) -> Option<ScalarValue> {
    match arg {
        ScalarRef::Integer(i) => Some(ScalarValue::Integer(i)),
        ScalarRef::Float(f) => Some(ScalarValue::Integer(f.0 as i64)),
        ScalarRef::Bool(b) => Some(ScalarValue::Integer(if b { 1 } else { 0 })),
        ScalarRef::String(s) => {
            let i = s.to_lowercase().parse::<i64>().ok()?;
            Some(ScalarValue::Integer(i))
        }
        _ => None,
    }
}

#[cypher_func(batch_name = "boolean_to_float_batch", sig = "(bool) -> any")]
fn boolean_to_float(b: bool) -> ScalarValue {
    if b {
        ScalarValue::Float(F64::from(1.0))
    } else {
        ScalarValue::Float(F64::from(0.0))
    }
}

// String | Integer | Float => Float
// if fails, return NULL
#[cypher_func(batch_name = "any_to_float_batch", sig = "(any) -> any")]
fn any_to_float(arg: ScalarRef<'_>) -> Option<ScalarValue> {
    match arg {
        ScalarRef::Float(f) => Some(ScalarValue::Float(f)),
        ScalarRef::Integer(i) => Some(ScalarValue::Float(F64::from(i as f64))),
        ScalarRef::String(s) => {
            let f = s.parse::<f64>().ok()?;
            Some(ScalarValue::Float(F64::from(f)))
        }
        _ => None,
    }
}

#[cypher_func(batch_name = "boolean_to_string_batch", sig = "(bool) -> any")]
fn boolean_to_string(b: bool) -> ScalarValue {
    if b {
        ScalarValue::String("true".to_string())
    } else {
        ScalarValue::String("false".to_string())
    }
}

// Integer | Float | Boolean | DATE | LocalTime | LocalDateTime | ZonedDateTime | Duration | String => String
// if fails, return NULL
#[cypher_func(batch_name = "any_to_string_batch", sig = "(any) -> any")]
fn any_to_string(arg: ScalarRef<'_>) -> Option<ScalarValue> {
    match arg {
        ScalarRef::Integer(i) => Some(ScalarValue::String(i.to_string())),
        ScalarRef::String(s) => Some(ScalarValue::String(s.to_string())),
        ScalarRef::Float(f) => Some(ScalarValue::String(f.to_string())),
        ScalarRef::Bool(b) => Some(ScalarValue::String(b.to_string())),
        ScalarRef::Date(d) => Some(ScalarValue::String(d.to_string())),
        ScalarRef::LocalTime(lt) => Some(ScalarValue::String(lt.to_string())),
        ScalarRef::LocalDateTime(ldt) => Some(ScalarValue::String(ldt.to_string())),
        ScalarRef::ZonedDateTime(zdt) => Some(ScalarValue::String(zdt.to_string())),
        ScalarRef::Duration(d) => Some(ScalarValue::String(d.to_string())),
        _ => None,
    }
}

pub(crate) fn register(registry: &mut ScalarFunctionRegistry) {
    let mut to_boolean = ScalarFunctionSet::new("toboolean");
    to_boolean.add_function(scalar_function!(
        "toboolean",
        [LogicalType::BOOL] -> LogicalType::BOOL,
        |_| Ok(Arc::new(boolean_to_boolean_batch))
    ));
    to_boolean.add_function(scalar_function!(
        "toboolean",
        [LogicalType::ANY] -> LogicalType::BOOL,
        |_| Ok(Arc::new(any_to_boolean_batch))
    ));
    registry.insert(to_boolean);

    let mut to_integer = ScalarFunctionSet::new("tointeger");
    to_integer.add_function(scalar_function!(
        "tointeger",
        [LogicalType::BOOL] -> LogicalType::ANY,
        |_| Ok(Arc::new(boolean_to_integer_batch))
    ));
    to_integer.add_function(scalar_function!(
        "tointeger",
        [LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_to_integer_batch))
    ));
    registry.insert(to_integer);

    let mut to_float = ScalarFunctionSet::new("tofloat");
    to_float.add_function(scalar_function!(
        "tofloat",
        [LogicalType::BOOL] -> LogicalType::ANY,
        |_| Ok(Arc::new(boolean_to_float_batch))
    ));
    to_float.add_function(scalar_function!(
        "tofloat",
        [LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_to_float_batch))
    ));
    registry.insert(to_float);

    let mut to_string = ScalarFunctionSet::new("tostring");
    to_string.add_function(scalar_function!(
        "tostring",
        [LogicalType::BOOL] -> LogicalType::ANY,
        |_| Ok(Arc::new(boolean_to_string_batch))
    ));
    to_string.add_function(scalar_function!(
        "tostring",
        [LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_to_string_batch))
    ));
    registry.insert(to_string);
}
