use elio_common::data_type::DataType;

use crate::expr::{Expr, ExprNode};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Subquery {}

impl ExprNode for Subquery {
    fn typ(&self) -> DataType {
        todo!()
    }
}

impl From<Subquery> for Expr {
    fn from(val: Subquery) -> Self {
        Expr::Subquery(val)
    }
}
