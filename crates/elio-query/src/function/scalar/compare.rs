//! compare functions
//!  - eq
//!  - not_eq
//!  - gt
//!  - gt_eq
//!  - lt
//!  - lt_eq

use std::cmp::Ordering;
use std::sync::Arc;

use bitvec::prelude::*;
use elio_common::array::*;
use elio_common::data_type::LogicalType;
use elio_common::scalar::*;

use crate::execution::error::EvalError;
use crate::execution::expr::ScalarFunctionExec;
use crate::function::ScalarFunctionRegistry;
use crate::function::sig::ScalarFunctionSet;
use crate::scalar_function;

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

pub struct AnyEqBatch;

impl ScalarFunctionExec for AnyEqBatch {
    fn execute(&self, inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
        do_compare(inputs, vis, len, |ord| matches!(ord, Ordering::Equal), || Some(false))
    }
}

pub struct AnyNotEqBatch;
impl ScalarFunctionExec for AnyNotEqBatch {
    fn execute(&self, inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
        do_compare(inputs, vis, len, |ord| !matches!(ord, Ordering::Equal), || Some(true))
    }
}

pub struct AnyGtBatch;
impl ScalarFunctionExec for AnyGtBatch {
    fn execute(&self, inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
        do_compare(inputs, vis, len, |ord| matches!(ord, Ordering::Greater), || None)
    }
}

pub struct AnyGtEqBatch;
impl ScalarFunctionExec for AnyGtEqBatch {
    fn execute(&self, inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
        do_compare(
            inputs,
            vis,
            len,
            |ord| matches!(ord, Ordering::Greater | Ordering::Equal),
            || None,
        )
    }
}

pub struct AnyLtBatch;
impl ScalarFunctionExec for AnyLtBatch {
    fn execute(&self, inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
        do_compare(inputs, vis, len, |ord| matches!(ord, Ordering::Less), || None)
    }
}

pub struct AnyLtEqBatch;
impl ScalarFunctionExec for AnyLtEqBatch {
    fn execute(&self, inputs: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
        do_compare(
            inputs,
            vis,
            len,
            |ord| matches!(ord, Ordering::Less | Ordering::Equal),
            || None,
        )
    }
}

pub(crate) fn register(registry: &mut ScalarFunctionRegistry) {
    let mut equal = ScalarFunctionSet::new("eq");
    equal.add_function(scalar_function!(
        "eq",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::BOOL,
        |_| Ok(Arc::new(AnyEqBatch))
    ));
    registry.insert(equal);

    let mut not_equal = ScalarFunctionSet::new("not_eq");
    not_equal.add_function(scalar_function!(
        "not_eq",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::BOOL,
        |_| Ok(Arc::new(AnyNotEqBatch))
    ));
    registry.insert(not_equal);

    let mut gt = ScalarFunctionSet::new("gt");
    gt.add_function(scalar_function!(
        "gt",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::BOOL,
        |_| Ok(Arc::new(AnyGtBatch))
    ));
    registry.insert(gt);

    let mut gt_eq = ScalarFunctionSet::new("gt_eq");
    gt_eq.add_function(scalar_function!(
        "gt_eq",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::BOOL,
        |_| Ok(Arc::new(AnyGtEqBatch))
    ));
    registry.insert(gt_eq);

    let mut lt = ScalarFunctionSet::new("lt");
    lt.add_function(scalar_function!(
        "lt",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::BOOL,
        |_| Ok(Arc::new(AnyLtBatch))
    ));
    registry.insert(lt);

    let mut lt_eq = ScalarFunctionSet::new("lt_eq");
    lt_eq.add_function(scalar_function!(
        "lt_eq",
        [LogicalType::ANY, LogicalType::ANY] -> LogicalType::BOOL,
        |_| Ok(Arc::new(AnyLtEqBatch))
    ));
    registry.insert(lt_eq);
}
