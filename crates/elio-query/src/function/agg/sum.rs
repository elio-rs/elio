//! Sum aggregate function
//! inputs:
//!  - numeric datums: Integer, Float, Duration
//!
//! outputs:
//!  - numeric scalar: Integer, Float, Duration
//!
//! null handling:
//!  - ignore nulls

use std::sync::Arc;

use elio_common::data_type::{F64, LogicalType};
use elio_common::scalar::{Datum, DatumRef, ScalarValue};

use crate::agg_function;
use crate::execution::error::EvalError;
use crate::execution::expr::AggFunctionExec;
use crate::execution::expr::agg_call::AggFunctionState;
use crate::function::AggFunctionRegistry;
use crate::function::sig::AggFunctionSet;

struct SumState {
    acc: ScalarValue,
}

impl SumState {
    fn new() -> Self {
        Self {
            acc: ScalarValue::Integer(0),
        }
    }
}

impl AggFunctionState for SumState {}

#[derive(Debug)]
struct SumAggFunctionExec;

impl AggFunctionExec for SumAggFunctionExec {
    fn create_state(&self) -> Box<dyn AggFunctionState> {
        Box::new(SumState::new())
    }

    fn update_state(&self, state: &mut Box<dyn AggFunctionState>, row: &[DatumRef]) -> Result<(), EvalError> {
        assert_eq!(row.len(), 1);
        // ignore nulls
        let Some(val) = row[0] else {
            return Ok(());
        };
        let state = state
            .downcast_mut::<SumState>()
            .ok_or_else(|| EvalError::type_error("sum state type mismatch"))?;

        let acc = std::mem::take(&mut state.acc);
        match (acc, val) {
            (ScalarValue::Integer(acc), ScalarValue::Integer(val)) => {
                state.acc =
                    ScalarValue::Integer(acc.checked_add(*val).ok_or_else(|| {
                        EvalError::arithmetic_overflow("sum", vec![acc.to_string(), val.to_string()])
                    })?);
            }
            (ScalarValue::Integer(acc), ScalarValue::Float(val)) => {
                state.acc = ScalarValue::Float(F64::from(F64::from(acc as f64) + val));
            }
            (ScalarValue::Float(acc), ScalarValue::Float(val)) => {
                state.acc = ScalarValue::Float(F64::from(acc + *val));
            }
            // TODO(pgao): maybe we should have an data type category for numeric types
            (ScalarValue::Float(acc), ScalarValue::Integer(val)) => {
                state.acc = ScalarValue::Float(F64::from(acc + *val as f64))
            }
            (ScalarValue::Duration(acc), ScalarValue::Duration(val)) => {
                state.acc =
                    ScalarValue::Duration(acc.checked_add(val).ok_or_else(|| {
                        EvalError::arithmetic_overflow("sum", vec![acc.to_string(), val.to_string()])
                    })?);
            }
            _ => return Err(EvalError::type_error("sum state type mismatch")),
        }
        Ok(())
    }

    fn extract_value(&self, state: &mut Box<dyn AggFunctionState>) -> Datum {
        let state = state.downcast_mut::<SumState>().expect("expected sum state");
        Some(state.acc.clone())
    }
}

pub(crate) fn register(registry: &mut AggFunctionRegistry) {
    let mut sum = AggFunctionSet::new("sum");
    sum.add_function(agg_function!(
        "sum",
        [LogicalType::ANY] -> LogicalType::ANY,
        |_args| Ok(Arc::new(SumAggFunctionExec))
    ));
    registry.insert(sum);
}
