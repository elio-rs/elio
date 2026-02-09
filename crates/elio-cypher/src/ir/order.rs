use elio_common::order::SortDirection;

use crate::expr::Expr;

#[derive(derive_more::Display)]
#[display("{} {}", expr.pretty(), direction)]
pub struct SortItem {
    pub expr: Box<Expr>,
    pub direction: SortDirection,
}

impl SortItem {
    pub fn needs_extra_project(&self) -> bool {
        self.expr.as_variable_ref().is_none()
    }
}
