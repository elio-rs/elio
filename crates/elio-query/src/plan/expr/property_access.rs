use elio_common::data_type::LogicalType;

use crate::plan::expr::{Expr, ExprNode, IrToken};

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct PropertyAccess {
    pub expr: Box<Expr>,
    pub property: IrToken,
    // in most cases, the typ should be any, since we do not support constaint for now
    typ: LogicalType,
}

impl PropertyAccess {
    pub fn new_unchecked(expr: Box<Expr>, property: &IrToken, typ: &LogicalType) -> Self {
        Self {
            expr,
            property: property.to_owned(),
            typ: typ.clone(),
        }
    }
}

impl ExprNode for PropertyAccess {
    fn typ(&self) -> elio_common::data_type::LogicalType {
        self.typ.clone()
    }
}

impl From<PropertyAccess> for Expr {
    fn from(val: PropertyAccess) -> Self {
        Expr::PropertyAccess(val)
    }
}
