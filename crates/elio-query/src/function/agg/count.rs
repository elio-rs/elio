//! Count aggregate function
//! variants:
//!  - count(expr): counts non-null values
//!  - count(*): counts all rows (zero args)
//!
//! outputs:
//!  - Integer
//!
//! null handling:
//!  - count(expr) ignores nulls
//!  - count(*) counts all rows including nulls

use std::sync::Arc;

use elio_common::data_type::LogicalType;
use elio_common::scalar::{Datum, DatumRef, ScalarValue};

use crate::agg_function;
use crate::execution::error::EvalError;
use crate::execution::expr::AggFunctionExec;
use crate::execution::expr::agg_call::AggFunctionState;
use crate::function::AggFunctionRegistry;
use crate::function::sig::AggFunctionSet;

struct CountState {
    count: i64,
}

impl CountState {
    fn new() -> Self {
        Self { count: 0 }
    }
}

impl AggFunctionState for CountState {}

/// count(expr) — counts non-null values
#[derive(Debug)]
struct CountFunctionExec;

impl AggFunctionExec for CountFunctionExec {
    fn create_state(&self) -> Box<dyn AggFunctionState> {
        Box::new(CountState::new())
    }

    fn update_state(&self, state: &mut Box<dyn AggFunctionState>, row: &[DatumRef]) -> Result<(), EvalError> {
        assert_eq!(row.len(), 1);
        // skip nulls (None = null datum, Unknown = null scalar inside containers like lists)
        match row[0] {
            None | Some(ScalarValue::Unknown) => return Ok(()),
            _ => {}
        }
        let state = state
            .downcast_mut::<CountState>()
            .ok_or_else(|| EvalError::type_error("count state type mismatch"))?;
        state.count += 1;
        Ok(())
    }

    fn extract_value(&self, state: &mut Box<dyn AggFunctionState>) -> Datum {
        let state = state.downcast_mut::<CountState>().expect("expected count state");
        Some(ScalarValue::Integer(state.count))
    }
}

/// count(*) — counts all rows
#[derive(Debug)]
struct CountStarExec;

impl AggFunctionExec for CountStarExec {
    fn create_state(&self) -> Box<dyn AggFunctionState> {
        Box::new(CountState::new())
    }

    fn update_state(&self, state: &mut Box<dyn AggFunctionState>, _row: &[DatumRef]) -> Result<(), EvalError> {
        let state = state
            .downcast_mut::<CountState>()
            .ok_or_else(|| EvalError::type_error("count state type mismatch"))?;
        state.count += 1;
        Ok(())
    }

    fn extract_value(&self, state: &mut Box<dyn AggFunctionState>) -> Datum {
        let state = state.downcast_mut::<CountState>().expect("expected count state");
        Some(ScalarValue::Integer(state.count))
    }
}

pub(crate) fn register(registry: &mut AggFunctionRegistry) {
    let mut count = AggFunctionSet::new("count");
    count.add_function(agg_function!(
        "count",
        [LogicalType::ANY] -> LogicalType::INTEGER,
        |_args| Ok(Arc::new(CountFunctionExec))
    ));
    count.add_function(agg_function!(
        "count",
        [] -> LogicalType::INTEGER,
        |_args| Ok(Arc::new(CountStarExec))
    ));
    registry.insert(count);
}
