use elio_common::data_type::LogicalType;

use crate::plan::expr::{Expr, ExprNode};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Subquery {}

impl ExprNode for Subquery {
    fn typ(&self) -> LogicalType {
        todo!()
    }
}

impl From<Subquery> for Expr {
    fn from(val: Subquery) -> Self {
        Expr::Subquery(val)
    }
}
