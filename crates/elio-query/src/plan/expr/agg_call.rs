use elio_common::data_type::LogicalType;

use crate::plan::expr::{Expr, ExprNode};

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct AggCall {
    /// function name
    pub func: String,
    /// function implementation id
    pub func_id: String,
    pub args: Vec<Expr>,
    pub distinct: bool,
    typ: LogicalType,
}

impl AggCall {
    pub fn new_unchecked(func: String, func_id: String, args: Vec<Expr>, distinct: bool, typ: LogicalType) -> Self {
        Self {
            func,
            func_id,
            args,
            distinct,
            typ,
        }
    }
}

impl ExprNode for AggCall {
    fn typ(&self) -> LogicalType {
        self.typ.clone()
    }
}

impl From<AggCall> for Expr {
    fn from(val: AggCall) -> Self {
        Expr::AggCall(val)
    }
}
