use downcast_rs::{DowncastSend, impl_downcast};
use elio_common::data_type::DataType;
use elio_common::scalar::{Datum, DatumRef};

use crate::impl_::{Expression, SharedExpression};

// agg function call expression
#[derive(Debug)]
pub struct AggCallExpr {
    pub func: String,
    pub args: Vec<SharedExpression>,
    pub typ: DataType,
}

// call aggregate with distinct inputs
pub struct DistinctAggCallExpr {}

pub trait AggFuncImpl: Send + Sync + 'static {
    fn create_state(&self) -> Box<dyn AggFuncState>;
    fn update_state(&self, state: &mut Box<dyn AggFuncState>, row: &[DatumRef]);
    fn extract_value(&self, state: &mut Box<dyn AggFuncState>) -> Datum;
}

pub trait AggFuncState: Send + 'static + DowncastSend {}

impl_downcast!(AggFuncState);
