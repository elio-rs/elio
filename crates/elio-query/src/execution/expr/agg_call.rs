use downcast_rs::{DowncastSend, impl_downcast};
use elio_common::scalar::{Datum, DatumRef};

use crate::execution::error::EvalError;

pub trait AggFunctionExec: Send + Sync + 'static {
    fn create_state(&self) -> Box<dyn AggFunctionState>;
    fn update_state(&self, state: &mut Box<dyn AggFunctionState>, row: &[DatumRef]) -> Result<(), EvalError>;
    fn extract_value(&self, state: &mut Box<dyn AggFunctionState>) -> Datum;
}

pub trait AggFunctionState: Send + 'static + DowncastSend {}

impl_downcast!(AggFunctionState);
