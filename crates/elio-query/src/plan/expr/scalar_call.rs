use elio_common::data_type::LogicalType;

use crate::function::SCALAR_FUNCTION_REGISTRY;
use crate::function::sig::{FunctionData, ScalarFunction};
use crate::plan::expr::{Expr, ExprNode};

// scalar function call
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct ScalarCall {
    pub function: ScalarFunction,
    pub function_data: Option<Box<dyn FunctionData>>,
    pub args: Vec<Expr>,
}

impl ScalarCall {
    pub fn new_unchecked(
        function: ScalarFunction,
        function_data: Option<Box<dyn FunctionData>>,
        args: Vec<Expr>,
    ) -> Self {
        Self {
            function,
            function_data,
            args,
        }
    }

    // TODO(pgao): consider move logical operator to top level expression
    pub fn and_unchecked(args: Vec<Expr>) -> Self {
        assert_eq!(args.len(), 2);
        let and_impl = SCALAR_FUNCTION_REGISTRY.get_and_function();
        Self::new_unchecked(and_impl, None, args)
    }

    pub fn or_unchecked(args: Vec<Expr>) -> Self {
        let or_impl = SCALAR_FUNCTION_REGISTRY.get_or_function();
        Self::new_unchecked(or_impl, None, args)
    }

    pub fn equal_unchecked(args: Vec<Expr>) -> Self {
        let equal_impl = SCALAR_FUNCTION_REGISTRY.get_eq_function();
        Self::new_unchecked(equal_impl, None, args)
    }
}

impl ExprNode for ScalarCall {
    fn typ(&self) -> LogicalType {
        self.function.return_type.clone()
    }
}

impl From<ScalarCall> for Expr {
    fn from(val: ScalarCall) -> Self {
        Expr::ScalarCall(val)
    }
}
