//! Temporal functions:
//!
//! Date Functions:
//! - date:
//!   - input: String | Date | ZonedDateTime
//!   - input: []
//!   - output: Date
//!   - propogate nulls: true
//!   - produce nulls: false
//!   - error on invalid input: true
//!   
//!
//! LocalTime Functions:
//! - localtime:
//!   - input: String | LocalTime | LocalDateTime | ZonedDateTime
//!   - input: []
//!   - output: LocalTime
//!   - propogate nulls: true
//!   - produce nulls: false
//!   - error on invalid input: true
//!   
//!
//! LocalDateTime Functions:
//! - localdatetime:
//!   - input: String | LocalDateTime | ZonedDateTime
//!   - input: []
//!   - output: LocalDateTime
//!   - propogate nulls: true
//!   - produce nulls: false
//!   - error on invalid input: true
//!   
//!
//! ZonedDateTime Functions:
//! - zoneddatetime:
//!   - input: String | ZonedDateTime
//!   - input: []
//!   - output: ZonedDateTime
//!   - propogate nulls: true
//!   - produce nulls: false
//!   - error on invalid input: true
//!   
//!
//! Duration Functions:

use bitvec::prelude::*;
use elio_common::array::*;
use elio_common::scalar::*;
use expr_macros::cypher_func;

use crate::define_function;
use crate::error::EvalError;
use crate::func::FunctionRegistry;

#[cypher_func(batch_name = "date_batch", sig = "(any) -> any")]
fn date(arg: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match arg {
        ScalarRef::Date(date) => Ok(ScalarValue::Date(date)),
        ScalarRef::LocalDateTime(local_date_time) => Ok(ScalarValue::Date(local_date_time.to_date())),
        ScalarRef::ZonedDateTime(zoned_date_time) => Ok(ScalarValue::Date(zoned_date_time.to_date())),
        ScalarRef::String(s) => Ok(ScalarValue::Date(
            Date::try_from(s).map_err(|(ctx, expected, actual)| EvalError::invalid_argument(ctx, expected, actual))?,
        )),
        _ => Err(EvalError::invalid_argument(
            "date()",
            "Date, LocalTime, LocalDateTime, ZonedDateTime, or String",
            format!("{:?}", arg),
        )),
    }
}

#[cypher_func(batch_name = "current_date_batch", sig = "() -> any")]
fn current_date() -> Result<ScalarValue, EvalError> {
    Ok(ScalarValue::Date(Date::from(chrono::Local::now().naive_local().date())))
}

#[cypher_func(batch_name = "local_time_batch", sig = "(any) -> any")]
fn any_local_time(arg: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match arg {
        ScalarRef::LocalTime(local_time) => Ok(ScalarValue::LocalTime(local_time)),
        ScalarRef::LocalDateTime(local_date_time) => Ok(ScalarValue::LocalTime(local_date_time.to_local_time())),
        ScalarRef::ZonedDateTime(zoned_date_time) => Ok(ScalarValue::LocalTime(zoned_date_time.to_local_time())),
        ScalarRef::String(s) => Ok(ScalarValue::LocalTime(
            LocalTime::try_from(s)
                .map_err(|(ctx, expected, actual)| EvalError::invalid_argument(ctx, expected, actual))?,
        )),
        _ => Err(EvalError::invalid_argument(
            "localTime()",
            "LocalTime, LocalDateTime, ZonedDateTime, or String",
            format!("{:?}", arg),
        )),
    }
}

#[cypher_func(batch_name = "current_local_time_batch", sig = "() -> any")]
fn current_local_time() -> Result<ScalarValue, EvalError> {
    Ok(ScalarValue::LocalTime(LocalTime::from(
        chrono::Local::now().naive_local().time(),
    )))
}

#[cypher_func(batch_name = "local_date_time_batch", sig = "(any) -> any")]
fn any_localdatetime(arg: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match arg {
        ScalarRef::LocalDateTime(local_date_time) => Ok(ScalarValue::LocalDateTime(local_date_time)),
        ScalarRef::ZonedDateTime(zoned_date_time) => {
            Ok(ScalarValue::LocalDateTime(zoned_date_time.to_local_date_time()))
        }
        ScalarRef::String(s) => Ok(ScalarValue::LocalDateTime(
            LocalDateTime::try_from(s)
                .map_err(|(ctx, expected, actual)| EvalError::invalid_argument(ctx, expected, actual))?,
        )),
        _ => Err(EvalError::invalid_argument(
            "localDateTime()",
            "LocalDateTime, ZonedDateTime, or String",
            format!("{:?}", arg),
        )),
    }
}

#[cypher_func(batch_name = "current_local_date_time_batch", sig = "() -> any")]
fn current_local_date_time() -> Result<ScalarValue, EvalError> {
    Ok(ScalarValue::LocalDateTime(LocalDateTime::from(
        chrono::Local::now().naive_local(),
    )))
}

#[cypher_func(batch_name = "zoned_date_time_batch", sig = "(any) -> any")]
fn any_zoned_date_time(arg: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match arg {
        ScalarRef::ZonedDateTime(zoned_date_time) => Ok(ScalarValue::ZonedDateTime(zoned_date_time)),
        ScalarRef::String(s) => Ok(ScalarValue::ZonedDateTime(
            ZonedDateTime::try_from(s)
                .map_err(|(ctx, expected, actual)| EvalError::invalid_argument(ctx, expected, actual))?,
        )),
        _ => Err(EvalError::invalid_argument(
            "zonedDateTime()",
            "ZonedDateTime or String",
            format!("{:?}", arg),
        )),
    }
}

#[cypher_func(batch_name = "current_zoned_date_time_batch", sig = "() -> any")]
fn current_zoned_date_time() -> Result<ScalarValue, EvalError> {
    Ok(ScalarValue::ZonedDateTime(ZonedDateTime::from(
        chrono::Local::now().fixed_offset(),
    )))
}

#[cypher_func(batch_name = "duration_batch", sig = "(any) -> any")]
fn any_duration(arg: ScalarRef<'_>) -> Result<ScalarValue, EvalError> {
    match arg {
        ScalarRef::Duration(d) => Ok(ScalarValue::Duration(d)),
        ScalarRef::String(s) => Ok(ScalarValue::Duration(
            Duration::try_from(s)
                .map_err(|(ctx, expected, actual)| EvalError::invalid_argument(ctx, expected, actual))?,
        )),
        _ => Err(EvalError::invalid_argument(
            "duration()",
            "Duration or String",
            format!("{:?}", arg),
        )),
    }
}

pub(crate) fn register(registry: &mut FunctionRegistry) {
    let date = define_function!( name: "date", impls: 
    [
        { args: [{anyof String | Date | LocalDateTime | ZonedDateTime| Any}], ret: Any, func: date_batch},
        { args: [], ret: Any, func: current_date_batch}
    ],
    is_agg: false);

    let local_time = define_function!( name: "localtime", impls: 
    [
        { args: [{anyof String | LocalTime | LocalDateTime | ZonedDateTime| Any}], ret: Any, func: local_time_batch},
        { args: [], ret: Any, func: current_local_time_batch}
    ],
    is_agg: false);

    let local_date_time = define_function!( name: "localdatetime", impls: 
    [
        { args: [{anyof String | LocalDateTime | ZonedDateTime| Any}], ret: Any, func: local_date_time_batch},
        { args: [], ret: Any, func: current_local_date_time_batch}
    ],
    is_agg: false);

    let date_time = define_function!( name: "datetime", impls: 
    [
        { args: [{anyof String | ZonedDateTime | Any}], ret: Any, func: zoned_date_time_batch},
        { args: [], ret: Any, func: current_zoned_date_time_batch}
    ],
    is_agg: false);

    let duration = define_function!( name: "duration", impls: 
    [
        { args: [{anyof String | Duration}], ret: Any, func: duration_batch}
    ],
    is_agg: false);

    registry.insert(date);
    registry.insert(local_time);
    registry.insert(local_date_time);
    registry.insert(date_time);
    registry.insert(duration);
}
