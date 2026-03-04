//! Boolean functions
//!
//! - And
//! - Or
//! - Xor
//! - Not
//! - Is NULL
//! - Is NOT NULL
//!
//! This implementation follows 3 value logic.

use std::sync::Arc;

use bitvec::prelude::*;
use elio_common::array::*;
use elio_common::data_type::LogicalType;

use crate::execution::error::EvalError;
use crate::function::ScalarFunctionRegistry;
use crate::function::sig::ScalarFunctionSet;
use crate::scalar_function;

// if either one of the input is null, return null
fn bool_and_batch(args: &[ArrayRef], _vis: &BitVec, _len: usize) -> Result<ArrayImpl, EvalError> {
    let arg0 = args[0].as_bool().unwrap();
    let arg1 = args[1].as_bool().unwrap();

    let out_data = arg0.to_filter_mask() & arg1.to_filter_mask();
    let out_valid = arg0.valid_map().clone() & arg1.valid_map().clone();
    Ok(BoolArray::from_parts(out_data, out_valid).into())
}

fn bool_or_batch(args: &[ArrayRef], _vis: &BitVec, _len: usize) -> Result<ArrayImpl, EvalError> {
    let arg0 = args[0].as_bool().unwrap();
    let arg1 = args[1].as_bool().unwrap();

    let out_data = arg0.to_filter_mask() | arg1.to_filter_mask();
    let out_valid = arg0.valid_map().clone() | arg1.valid_map().clone();
    Ok(BoolArray::from_parts(out_data, out_valid).into())
}

fn bool_xor_batch(args: &[ArrayRef], _vis: &BitVec, _len: usize) -> Result<ArrayImpl, EvalError> {
    let arg0 = args[0].as_bool().unwrap();
    let arg1 = args[1].as_bool().unwrap();

    let out_data = arg0.to_filter_mask() ^ arg1.to_filter_mask();
    let out_valid = arg0.valid_map().clone() & arg1.valid_map().clone();
    Ok(BoolArray::from_parts(out_data, out_valid).into())
}

fn bool_not_batch(args: &[ArrayRef], _vis: &BitVec, _len: usize) -> Result<ArrayImpl, EvalError> {
    let arg0 = args[0].as_bool().unwrap();

    let out_data = !arg0.to_filter_mask();
    let out_valid = arg0.valid_map().clone();
    Ok(BoolArray::from_parts(out_data, out_valid).into())
}

fn bool_is_null_batch(args: &[ArrayRef], _vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    let arg0 = &args[0];

    let out_data = !arg0.valid_map().clone();
    let out_valid = BitVec::repeat(true, len);
    Ok(BoolArray::from_parts(out_data, out_valid).into())
}

fn bool_is_not_null_batch(args: &[ArrayRef], _vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    let arg0 = &args[0];

    let out_data = arg0.valid_map().clone();
    let out_valid = BitVec::repeat(true, len);
    Ok(BoolArray::from_parts(out_data, out_valid).into())
}

pub(crate) fn register(registry: &mut ScalarFunctionRegistry) {
    let mut and = ScalarFunctionSet::new("and");
    and.add_function(scalar_function!(
        "and",
        [LogicalType::BOOL, LogicalType::BOOL] -> LogicalType::BOOL,
        |_| Ok(Arc::new(bool_and_batch))
    ));
    registry.insert(and);

    let mut or = ScalarFunctionSet::new("or");
    or.add_function(scalar_function!(
        "or",
        [LogicalType::BOOL, LogicalType::BOOL] -> LogicalType::BOOL,
        |_| Ok(Arc::new(bool_or_batch))
    ));
    registry.insert(or);

    let mut xor = ScalarFunctionSet::new("xor");
    xor.add_function(scalar_function!(
        "xor",
        [LogicalType::BOOL, LogicalType::BOOL] -> LogicalType::BOOL,
        |_| Ok(Arc::new(bool_xor_batch))
    ));
    registry.insert(xor);

    let mut not = ScalarFunctionSet::new("not");
    not.add_function(scalar_function!(
        "not",
        [LogicalType::BOOL] -> LogicalType::BOOL,
        |_| Ok(Arc::new(bool_not_batch))
    ));
    registry.insert(not);

    let mut is_null = ScalarFunctionSet::new("is_null");
    is_null.add_function(scalar_function!(
        "is_null",
        [LogicalType::BOOL] -> LogicalType::BOOL,
        |_| Ok(Arc::new(bool_is_null_batch))
    ));
    registry.insert(is_null);

    let mut is_not_null = ScalarFunctionSet::new("is_not_null");
    is_not_null.add_function(scalar_function!(
        "is_not_null",
        [LogicalType::BOOL] -> LogicalType::BOOL,
        |_| Ok(Arc::new(bool_is_not_null_batch))
    ));
    registry.insert(is_not_null);
}
