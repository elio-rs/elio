//! Arithmetic functions
//! - add
//! - subtract
//! - multiply
//! - divide
//! - modulo
//! - pow

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

#[cypher_func(batch_name = "any_add_batch", sig = "(any, any) -> any")]
fn any_add(lhs: ScalarRef<'_>, rhs: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match (lhs, rhs) {
        (ScalarRef::Null, _) | (_, ScalarRef::Null) => Ok(ScalarValue::Unknown),
        // Numeric
        (ScalarRef::Integer(a), ScalarRef::Integer(b)) => a
            .checked_add(b)
            .map(ScalarValue::Integer)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![a.to_string(), b.to_string()])),
        (ScalarRef::Float(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(a + b)),
        (ScalarRef::Integer(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(F64::from(a as f64) + b)),
        (ScalarRef::Float(a), ScalarRef::Integer(b)) => Ok(ScalarValue::Float(a + F64::from(b as f64))),

        // String
        (ScalarRef::String(a), ScalarRef::String(b)) => Ok(ScalarValue::String(format!("{}{}", a, b))),
        (ScalarRef::String(a), b) => Ok(ScalarValue::String(format!("{}{}", a, b.to_owned_scalar()))),
        (a, ScalarRef::String(b)) => Ok(ScalarValue::String(format!("{}{}", a.to_owned_scalar(), b))),

        // List
        (ScalarRef::List(a), ScalarRef::List(b)) => {
            let values: Vec<ScalarValue> = a.iter().chain(b.iter()).map(|x| x.to_owned_scalar()).collect();
            Ok(ScalarValue::List(Box::new(ListValue::new(values))))
        }
        (ScalarRef::List(a), b) => {
            let mut values: Vec<ScalarValue> = a.iter().map(|x| x.to_owned_scalar()).collect();
            values.push(b.to_owned_scalar());
            Ok(ScalarValue::List(Box::new(ListValue::new(values))))
        }
        (a, ScalarRef::List(b)) => {
            let mut values = vec![a.to_owned_scalar()];
            values.extend(b.iter().map(|x| x.to_owned_scalar()));
            Ok(ScalarValue::List(Box::new(ListValue::new(values))))
        }

        // Temporal
        (ScalarRef::Date(d), ScalarRef::Duration(dur)) => d
            .checked_add(&dur)
            .map(ScalarValue::Date)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![d.to_string(), dur.to_string()])),
        (ScalarRef::Duration(dur), ScalarRef::Date(d)) => d
            .checked_add(&dur)
            .map(ScalarValue::Date)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![d.to_string(), dur.to_string()])),

        (ScalarRef::LocalTime(t), ScalarRef::Duration(dur)) => t
            .checked_add(&dur)
            .map(ScalarValue::LocalTime)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![t.to_string(), dur.to_string()])),
        (ScalarRef::Duration(dur), ScalarRef::LocalTime(t)) => t
            .checked_add(&dur)
            .map(ScalarValue::LocalTime)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![t.to_string(), dur.to_string()])),

        (ScalarRef::LocalDateTime(dt), ScalarRef::Duration(dur)) => dt
            .checked_add(&dur)
            .map(ScalarValue::LocalDateTime)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![dt.to_string(), dur.to_string()])),
        (ScalarRef::Duration(dur), ScalarRef::LocalDateTime(dt)) => dt
            .checked_add(&dur)
            .map(ScalarValue::LocalDateTime)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![dt.to_string(), dur.to_string()])),

        (ScalarRef::ZonedDateTime(dt), ScalarRef::Duration(dur)) => dt
            .checked_add(&dur)
            .map(ScalarValue::ZonedDateTime)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![dt.to_string(), dur.to_string()])),
        (ScalarRef::Duration(dur), ScalarRef::ZonedDateTime(dt)) => dt
            .checked_add(&dur)
            .map(ScalarValue::ZonedDateTime)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![dt.to_string(), dur.to_string()])),

        (ScalarRef::Duration(a), ScalarRef::Duration(b)) => a
            .checked_add(&b)
            .map(ScalarValue::Duration)
            .ok_or_else(|| EvalError::arithmetic_overflow("add", vec![a.to_string(), b.to_string()])),

        _ => Err(EvalError::type_error(format!("Cannot add {:?} and {:?}", lhs, rhs))),
    }
}

#[cypher_func(batch_name = "any_sub_batch", sig = "(any, any) -> any")]
fn any_sub(lhs: ScalarRef<'_>, rhs: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match (lhs, rhs) {
        (ScalarRef::Null, _) | (_, ScalarRef::Null) => Ok(ScalarValue::Unknown),
        // Numeric
        (ScalarRef::Integer(a), ScalarRef::Integer(b)) => a
            .checked_sub(b)
            .map(ScalarValue::Integer)
            .ok_or_else(|| EvalError::arithmetic_overflow("subtract", vec![a.to_string(), b.to_string()])),
        (ScalarRef::Float(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(a - b)),
        (ScalarRef::Integer(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(F64::from(a as f64) - b)),
        (ScalarRef::Float(a), ScalarRef::Integer(b)) => Ok(ScalarValue::Float(a - F64::from(b as f64))),

        // Temporal
        (ScalarRef::Date(d), ScalarRef::Duration(dur)) => d
            .checked_sub(&dur)
            .map(ScalarValue::Date)
            .ok_or_else(|| EvalError::arithmetic_overflow("subtract", vec![d.to_string(), dur.to_string()])),
        (ScalarRef::Date(a), ScalarRef::Date(b)) => Ok(ScalarValue::Duration(a.diff(&b))),

        (ScalarRef::LocalTime(t), ScalarRef::Duration(dur)) => t
            .checked_sub(&dur)
            .map(ScalarValue::LocalTime)
            .ok_or_else(|| EvalError::arithmetic_overflow("subtract", vec![t.to_string(), dur.to_string()])),
        // (ScalarRef::LocalTime(a), ScalarRef::LocalTime(b)) => Ok(ScalarValue::Duration(a.diff(&b))),
        (ScalarRef::LocalDateTime(dt), ScalarRef::Duration(dur)) => dt
            .checked_sub(&dur)
            .map(ScalarValue::LocalDateTime)
            .ok_or_else(|| EvalError::arithmetic_overflow("subtract", vec![dt.to_string(), dur.to_string()])),
        // (ScalarRef::LocalDateTime(a), ScalarRef::LocalDateTime(b)) => Ok(ScalarValue::Duration(a.diff(&b))),
        (ScalarRef::ZonedDateTime(dt), ScalarRef::Duration(dur)) => dt
            .checked_sub(&dur)
            .map(ScalarValue::ZonedDateTime)
            .ok_or_else(|| EvalError::arithmetic_overflow("subtract", vec![dt.to_string(), dur.to_string()])),
        // (ScalarRef::ZonedDateTime(a), ScalarRef::ZonedDateTime(b)) => Ok(ScalarValue::Duration(a.diff(&b))),
        (ScalarRef::Duration(a), ScalarRef::Duration(b)) => a
            .checked_sub(&b)
            .map(ScalarValue::Duration)
            .ok_or_else(|| EvalError::arithmetic_overflow("subtract", vec![a.to_string(), b.to_string()])),

        _ => Err(EvalError::type_error(format!(
            "Cannot subtract {:?} from {:?}",
            rhs, lhs
        ))),
    }
}

#[cypher_func(batch_name = "any_mul_batch", sig = "(any, any) -> any")]
fn any_mul(lhs: ScalarRef<'_>, rhs: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match (lhs, rhs) {
        (ScalarRef::Null, _) | (_, ScalarRef::Null) => Ok(ScalarValue::Unknown),
        // Numeric
        (ScalarRef::Integer(a), ScalarRef::Integer(b)) => a
            .checked_mul(b)
            .map(ScalarValue::Integer)
            .ok_or_else(|| EvalError::arithmetic_overflow("multiply", vec![a.to_string(), b.to_string()])),
        (ScalarRef::Float(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(a * b)),
        (ScalarRef::Integer(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(F64::from(a as f64) * b)),
        (ScalarRef::Float(a), ScalarRef::Integer(b)) => Ok(ScalarValue::Float(a * F64::from(b as f64))),

        // Duration * Numeric
        (ScalarRef::Duration(d), ScalarRef::Integer(i)) => d
            .checked_mul(i)
            .map(ScalarValue::Duration)
            .ok_or_else(|| EvalError::arithmetic_overflow("multiply", vec![d.to_string(), i.to_string()])),
        (ScalarRef::Integer(i), ScalarRef::Duration(d)) => d
            .checked_mul(i)
            .map(ScalarValue::Duration)
            .ok_or_else(|| EvalError::arithmetic_overflow("multiply", vec![i.to_string(), d.to_string()])),

        (ScalarRef::Duration(d), ScalarRef::Float(f)) => d
            .checked_mul_f64(f.0)
            .map(ScalarValue::Duration)
            .ok_or_else(|| EvalError::arithmetic_overflow("multiply", vec![d.to_string(), f.to_string()])),
        (ScalarRef::Float(f), ScalarRef::Duration(d)) => d
            .checked_mul_f64(f.0)
            .map(ScalarValue::Duration)
            .ok_or_else(|| EvalError::arithmetic_overflow("multiply", vec![f.to_string(), d.to_string()])),

        _ => Err(EvalError::type_error(format!(
            "Cannot multiply {:?} with {:?}",
            lhs, rhs
        ))),
    }
}

#[cypher_func(batch_name = "any_div_batch", sig = "(any, any) -> any")]
fn any_div(lhs: ScalarRef<'_>, rhs: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match (lhs, rhs) {
        (ScalarRef::Null, _) | (_, ScalarRef::Null) => Ok(ScalarValue::Unknown),
        // Numeric / Numeric
        (ScalarRef::Integer(a), ScalarRef::Integer(b)) => {
            if b == 0 {
                return Err(EvalError::arithmetic_overflow(
                    "divide",
                    vec![a.to_string(), b.to_string()],
                ));
            }
            Ok(ScalarValue::Integer(a / b))
        }
        (ScalarRef::Float(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(a / b)),
        (ScalarRef::Integer(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(F64::from(a as f64) / b)),
        (ScalarRef::Float(a), ScalarRef::Integer(b)) => Ok(ScalarValue::Float(a / F64::from(b as f64))),

        // Duration / Numeric
        (ScalarRef::Duration(d), ScalarRef::Integer(i)) => {
            if i == 0 {
                return Err(EvalError::arithmetic_overflow(
                    "divide",
                    vec![d.to_string(), i.to_string()],
                ));
            }
            d.checked_div(i as f64)
                .map(ScalarValue::Duration)
                .ok_or_else(|| EvalError::arithmetic_overflow("divide", vec![d.to_string(), i.to_string()]))
        }
        (ScalarRef::Duration(d), ScalarRef::Float(f)) => {
            if f.0 == 0.0 {
                return Err(EvalError::arithmetic_overflow(
                    "divide",
                    vec![d.to_string(), f.to_string()],
                ));
            }
            d.checked_div(*f)
                .map(ScalarValue::Duration)
                .ok_or_else(|| EvalError::arithmetic_overflow("divide", vec![d.to_string(), f.to_string()]))
        }

        _ => Err(EvalError::type_error(format!("Cannot divide {:?} by {:?}", lhs, rhs))),
    }
}

#[cypher_func(batch_name = "any_mod_batch", sig = "(any, any) -> any")]
fn any_mod(lhs: ScalarRef<'_>, rhs: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match (lhs, rhs) {
        (ScalarRef::Null, _) | (_, ScalarRef::Null) => Ok(ScalarValue::Unknown),
        // Numeric
        (ScalarRef::Integer(a), ScalarRef::Integer(b)) => {
            if b == 0 {
                return Err(EvalError::arithmetic_overflow(
                    "modulo",
                    vec![a.to_string(), b.to_string()],
                ));
            }
            Ok(ScalarValue::Integer(a % b))
        }
        (ScalarRef::Float(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(a % b)),
        (ScalarRef::Integer(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(F64::from(a as f64) % b)),
        (ScalarRef::Float(a), ScalarRef::Integer(b)) => Ok(ScalarValue::Float(a % F64::from(b as f64))),

        _ => Err(EvalError::type_error(format!("Cannot modulo {:?} by {:?}", lhs, rhs))),
    }
}

#[cypher_func(batch_name = "any_pow_batch", sig = "(any, any) -> any")]
fn any_pow(lhs: ScalarRef<'_>, rhs: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match (lhs, rhs) {
        (ScalarRef::Null, _) | (_, ScalarRef::Null) => Ok(ScalarValue::Unknown),
        // Numeric
        (ScalarRef::Integer(a), ScalarRef::Integer(b)) => Ok(ScalarValue::Float(F64::from((a as f64).powf(b as f64)))),
        (ScalarRef::Float(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(F64::from(a.0.powf(b.0)))),
        (ScalarRef::Integer(a), ScalarRef::Float(b)) => Ok(ScalarValue::Float(F64::from((a as f64).powf(b.0)))),
        (ScalarRef::Float(a), ScalarRef::Integer(b)) => Ok(ScalarValue::Float(F64::from(a.0.powf(b as f64)))),

        _ => Err(EvalError::type_error(format!("Cannot pow {:?} by {:?}", lhs, rhs))),
    }
}

pub(crate) fn register(registry: &mut ScalarFunctionRegistry) {
    let mut add = ScalarFunctionSet::new("add");
    add.add_function(scalar_function!(
        "add",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_add_batch))
    ));
    registry.insert(add);

    let mut subtract = ScalarFunctionSet::new("subtract");
    subtract.add_function(scalar_function!(
        "subtract",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_sub_batch))
    ));
    registry.insert(subtract);

    let mut multiply = ScalarFunctionSet::new("multiply");
    multiply.add_function(scalar_function!(
        "multiply",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_mul_batch))
    ));
    registry.insert(multiply);

    let mut divide = ScalarFunctionSet::new("divide");
    divide.add_function(scalar_function!(
        "divide",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_div_batch))
    ));
    registry.insert(divide);

    let mut modulo = ScalarFunctionSet::new("modulo");
    modulo.add_function(scalar_function!(
        "modulo",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_mod_batch))
    ));
    registry.insert(modulo);

    let mut pow = ScalarFunctionSet::new("pow");
    pow.add_function(scalar_function!(
        "pow",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::ANY,
        |_| Ok(Arc::new(any_pow_batch))
    ));
    registry.insert(pow);
}
