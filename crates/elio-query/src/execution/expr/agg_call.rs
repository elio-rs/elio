use std::sync::Arc;

use downcast_rs::{DowncastSend, impl_downcast};
use elio_common::scalar::{Datum, DatumRef};

use crate::execution::error::EvalError;

// TODO(pgao): should be refactored
pub type AggInvocation = fn(args: &[Datum]) -> Result<Arc<dyn AggFuncImpl>, EvalError>;

pub trait AggFuncImpl: Send + Sync + 'static {
    fn create_state(&self) -> Box<dyn AggFuncState>;
    fn update_state(&self, state: &mut Box<dyn AggFuncState>, row: &[DatumRef]) -> Result<(), EvalError>;
    fn extract_value(&self, state: &mut Box<dyn AggFuncState>) -> Datum;
}

pub trait AggFuncState: Send + 'static + DowncastSend {}

impl_downcast!(AggFuncState);
