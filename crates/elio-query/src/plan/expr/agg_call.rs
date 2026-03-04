use elio_common::data_type::LogicalType;

use crate::function::sig::{AggFunction, FunctionData};
use crate::plan::expr::{Expr, ExprNode};

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct AggCall {
    pub function: AggFunction,
    pub function_data: Option<Box<dyn FunctionData>>,
    pub args: Vec<Expr>,
    pub distinct: bool,
}

impl AggCall {
    pub fn new_unchecked(
        function: AggFunction,
        function_data: Option<Box<dyn FunctionData>>,
        args: Vec<Expr>,
        distinct: bool,
    ) -> Self {
        Self {
            function,
            function_data,
            args,
            distinct,
        }
    }
}

impl ExprNode for AggCall {
    fn typ(&self) -> LogicalType {
        self.function.return_type.clone()
    }
}

impl From<AggCall> for Expr {
    fn from(val: AggCall) -> Self {
        Expr::AggCall(val)
    }
}
