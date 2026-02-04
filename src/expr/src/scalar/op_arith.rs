//! Arithmetic functions
//! - add
//! - subtract
//! - multiply
//! - divide
//! - modulo
//! - pow

use bitvec::prelude::*;
use elio_common::array::*;
use elio_common::data_type::F64;
use elio_common::scalar::*;
use expr_macros::cypher_func;

use crate::define_function;
use crate::error::EvalError;
use crate::scalar::FunctionRegistry;

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

pub(crate) fn register(registry: &mut FunctionRegistry) {
    let add = define_function!(
        name: "add",
        impls: [
            {args: [{exact Any}, {exact Any}], ret: Any, func: any_add_batch}
        ],
        is_agg: false
    );
    registry.insert(add);

    let subtract = define_function!(
        name: "subtract",
        impls: [
            {args: [{anyof Integer | Float}, {anyof Integer | Float}], ret: Any, func: any_sub_batch},
            {args: [{anyof Date | LocalTime | LocalDateTime | ZonedDateTime}, {exact Duration}], ret: Any, func: any_sub_batch},
            {args: [{exact Duration}, {exact Duration}], ret: Any, func: any_sub_batch},
            // in the case of input is generated by function which output any type,
            // we do not support union type for now, if we support it, we need to handle the case of union type
            {args: [{exact Any}, {exact Any}], ret: Any, func: any_sub_batch}
        ],
        is_agg: false
    );
    registry.insert(subtract);

    let multiply = define_function!(
        name: "multiply",
        impls: [
            {args: [{anyof Integer | Float}, {anyof Integer | Float}], ret: Any, func: any_mul_batch},
            {args: [{anyof Integer | Float}, {exact Duration}], ret: Any, func: any_mul_batch},
            {args: [{exact Duration}, {anyof Integer | Float}], ret: Any, func: any_mul_batch},
            // same as subtract
            {args: [{exact Any}, {exact Any}], ret: Any, func: any_mul_batch}
        ],
        is_agg: false
    );
    registry.insert(multiply);

    let divide = define_function!(
        name: "divide",
        impls: [
            {args: [{anyof Integer | Float}, {anyof Integer | Float}], ret: Any, func: any_div_batch},
            {args: [{exact Duration}, {anyof Integer | Float}], ret: Any, func: any_div_batch},
            // same as subtract
            {args: [{exact Any}, {exact Any}], ret: Any, func: any_div_batch}
        ],
        is_agg: false
    );
    registry.insert(divide);

    let modulo = define_function!(
        name: "modulo",
        impls: [
            {args: [{anyof Integer | Float}, {anyof Integer | Float}], ret: Any, func: any_mod_batch},
            {args: [{exact Any}, {exact Any}], ret: Any, func: any_mod_batch}
        ],
        is_agg: false
    );
    registry.insert(modulo);

    let pow = define_function!(
        name: "pow",
        impls: [
            {args: [{anyof Integer | Float}, {anyof Integer | Float}], ret: Any, func: any_pow_batch},
            {args: [{exact Any}, {exact Any}], ret: Any, func: any_pow_batch}
        ],
        is_agg: false
    );
    registry.insert(pow);
}
