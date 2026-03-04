pub mod error;
pub mod index;
use enum_as_inner::EnumAsInner;

use crate::function::sig::{AggFunctionSet, ScalarFunctionSet};

#[derive(EnumAsInner)]
pub enum FunctionCatalogEntry {
    Scalar(ScalarFunctionSet),
    Agg(AggFunctionSet),
}
