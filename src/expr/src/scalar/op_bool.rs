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

use bitvec::prelude::*;
use elio_common::array::*;

use crate::define_function;
use crate::error::EvalError;
use crate::scalar::FunctionRegistry;

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

pub(crate) fn register(registry: &mut FunctionRegistry) {
    let and = define_function!( name: "and", impls: [ {args: [{exact Bool}, {exact Bool}], ret: Bool, func: bool_and_batch}],is_agg: false);
    registry.insert(and);

    let or = define_function!( name: "or", impls: [ {args: [{exact Bool}, {exact Bool}], ret: Bool, func: bool_or_batch}],is_agg: false);
    registry.insert(or);

    let xor = define_function!( name: "xor", impls: [ {args: [{exact Bool}, {exact Bool}], ret: Bool, func: bool_xor_batch}],is_agg: false);
    registry.insert(xor);

    let not =
        define_function!( name: "not", impls: [ {args: [{exact Bool}], ret: Bool, func: bool_not_batch}],is_agg: false);
    registry.insert(not);

    let is_null = define_function!( name: "is_null", impls: [ {args: [{exact Bool}], ret: Bool, func: bool_is_null_batch}],is_agg: false);
    registry.insert(is_null);

    let is_not_null = define_function!( name: "is_not_null", impls: [ {args: [{exact Bool}], ret: Bool, func: bool_is_not_null_batch}],is_agg: false);
    registry.insert(is_not_null);
}
