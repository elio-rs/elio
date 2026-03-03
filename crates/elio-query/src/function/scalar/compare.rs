//! compare functions
//!  - eq
//!  - not_eq
//!  - gt
//!  - gt_eq
//!  - lt
//!  - lt_eq

use std::cmp::Ordering;

use bitvec::prelude::*;
use elio_common::array::*;
use elio_common::scalar::*;

use crate::define_function;
use crate::execution::error::EvalError;
use crate::function::scalar::FunctionRegistry;

// Tenary Logic
// if lhs and rhs is not comparable, then return NULL
fn do_compare(
    inputs: &[ArrayRef],
    vis: &BitVec,
    len: usize,
    op: impl Fn(Ordering) -> bool,
    non_handling: impl Fn() -> Option<bool>,
) -> Result<ArrayImpl, EvalError> {
    assert_eq!(inputs.len(), 2);
    let lhs = &inputs[0];
    let rhs = &inputs[1];
    let valid_rows = vis.clone() & lhs.valid_map() & rhs.valid_map();
    let mut out_builder = BoolArrayBuilder::with_capacity(len);

    for i in 0..len {
        if valid_rows[i] {
            let lhs_val = lhs.get(i).unwrap();
            let rhs_val = rhs.get(i).unwrap();
            match lhs_val.scalar_partial_cmp(&rhs_val) {
                Some(ord) => out_builder.push(Some(op(ord))),
                None => out_builder.push(non_handling()),
            }
        } else {
            out_builder.push(None);
        }
    }

    Ok(out_builder.finish().into())
}

fn any_eq_batch(inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    do_compare(inputs, vis, len, |ord| matches!(ord, Ordering::Equal), || Some(false))
}

fn any_not_eq_batch(inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    do_compare(inputs, vis, len, |ord| !matches!(ord, Ordering::Equal), || Some(true))
}

fn any_gt_batch(inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    do_compare(inputs, vis, len, |ord| matches!(ord, Ordering::Greater), || None)
}

fn any_gt_eq_batch(inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    do_compare(
        inputs,
        vis,
        len,
        |ord| matches!(ord, Ordering::Greater | Ordering::Equal),
        || None,
    )
}

fn any_lt_batch(inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    do_compare(inputs, vis, len, |ord| matches!(ord, Ordering::Less), || None)
}

fn any_lt_eq_batch(inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    do_compare(
        inputs,
        vis,
        len,
        |ord| matches!(ord, Ordering::Less | Ordering::Equal),
        || None,
    )
}

pub(crate) fn register(registry: &mut FunctionRegistry) {
    let equal = define_function!( name: "eq", impls: [ {args: [{exact ANY}, {exact ANY}], ret: BOOL, func: any_eq_batch}],is_agg: false);
    let not_equal = define_function!( name: "not_eq", impls: [ {args: [{exact ANY}, {exact ANY}], ret: BOOL, func: any_not_eq_batch}],is_agg: false);

    let gt = define_function!( name: "gt", impls: [ {args: [{exact ANY}, {exact ANY}], ret: BOOL, func: any_gt_batch}],is_agg: false);
    let gt_eq = define_function!( name: "gt_eq", impls: [ {args: [{exact ANY}, {exact ANY}], ret: BOOL, func: any_gt_eq_batch}],is_agg: false);
    let lt = define_function!( name: "lt", impls: [ {args: [{exact ANY}, {exact ANY}], ret: BOOL, func: any_lt_batch}],is_agg: false);
    let lt_eq = define_function!( name: "lt_eq", impls: [ {args: [{exact ANY}, {exact ANY}], ret: BOOL, func: any_lt_eq_batch}],is_agg: false);

    registry.insert(equal);
    registry.insert(not_equal);
    registry.insert(gt);
    registry.insert(gt_eq);
    registry.insert(lt);
    registry.insert(lt_eq);
}
