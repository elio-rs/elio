use elio_common::data_type::DataType;

use crate::function::FUNCTION_REGISTRY;
use crate::plan::expr::{Expr, ExprNode};

// scalar function call
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct FuncCall {
    // func name
    pub func: String,
    // function implementation id
    pub func_id: String,
    pub args: Vec<Expr>,
    typ: DataType,
}

impl FuncCall {
    pub fn new_unchecked(func: String, func_id: String, args: Vec<Expr>, typ: DataType) -> Self {
        Self {
            func,
            func_id,
            args,
            typ,
        }
    }

    pub fn and_unchecked(args: Vec<Expr>) -> Self {
        assert_eq!(args.len(), 2);
        let and_impl = FUNCTION_REGISTRY.get_and_func_impl();
        Self::new_unchecked("and".to_string(), and_impl.func_id.clone(), args, DataType::Bool)
    }

    pub fn or_unchecked(args: Vec<Expr>) -> Self {
        let or_impl = FUNCTION_REGISTRY.get_or_func_impl();
        Self::new_unchecked("or".to_string(), or_impl.func_id.clone(), args, DataType::Bool)
    }

    pub fn equal_unchecked(args: Vec<Expr>) -> Self {
        let equal_impl = FUNCTION_REGISTRY.get_equal_func_impl();
        Self::new_unchecked("eq".to_string(), equal_impl.func_id.clone(), args, DataType::Bool)
    }
}

impl ExprNode for FuncCall {
    fn typ(&self) -> DataType {
        self.typ.clone()
    }
}

impl From<FuncCall> for Expr {
    fn from(val: FuncCall) -> Self {
        Expr::FuncCall(val)
    }
}
