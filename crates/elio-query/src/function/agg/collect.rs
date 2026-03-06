//! Collect aggregate function
//! variants:
//!  - collect(expr): collects non-null values into a list
//!
//! outputs:
//!  - List
//!
//! null handling:
//!  - ignores nulls

use std::sync::Arc;

use elio_common::data_type::LogicalType;
use elio_common::scalar::list::ListValue;
use elio_common::scalar::{Datum, DatumRef, ScalarValue};

use crate::agg_function;
use crate::execution::error::EvalError;
use crate::execution::expr::AggFunctionExec;
use crate::execution::expr::agg_call::AggFunctionState;
use crate::function::AggFunctionRegistry;
use crate::function::sig::AggFunctionSet;

struct CollectState {
    values: Vec<ScalarValue>,
}

impl CollectState {
    fn new() -> Self {
        Self { values: Vec::new() }
    }
}

impl AggFunctionState for CollectState {}

#[derive(Debug)]
struct CollectFunctionExec;

impl AggFunctionExec for CollectFunctionExec {
    fn create_state(&self) -> Box<dyn AggFunctionState> {
        Box::new(CollectState::new())
    }

    fn update_state(&self, state: &mut Box<dyn AggFunctionState>, row: &[DatumRef]) -> Result<(), EvalError> {
        assert_eq!(row.len(), 1);
        // skip nulls
        match row[0] {
            None | Some(ScalarValue::Unknown) => return Ok(()),
            _ => {}
        }
        let state = state
            .downcast_mut::<CollectState>()
            .ok_or_else(|| EvalError::type_error("collect state type mismatch"))?;
        state.values.push(row[0].unwrap().clone());
        Ok(())
    }

    fn extract_value(&self, state: &mut Box<dyn AggFunctionState>) -> Datum {
        let state = state.downcast_mut::<CollectState>().expect("expected collect state");
        let values = std::mem::take(&mut state.values);
        Some(ScalarValue::List(Box::new(ListValue::new(values))))
    }
}

pub(crate) fn register(registry: &mut AggFunctionRegistry) {
    let mut collect = AggFunctionSet::new("collect");
    collect.add_function(agg_function!(
        "collect",
        [LogicalType::ANY] -> LogicalType::new_list(LogicalType::ANY),
        |_args| Ok(Arc::new(CollectFunctionExec))
    ));
    registry.insert(collect);
}
