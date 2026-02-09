use derive_more::Display;
use elio_common::order::SortDirection;

use crate::ast::Expr;

#[derive(Debug, Display)]
#[display("{}", self.items.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))]
pub struct OrderBy {
    pub items: Vec<SortItem>,
}

#[derive(Debug, Display)]
#[display("{} {}", expr, direction)]
pub struct SortItem {
    pub expr: Box<Expr>,
    pub direction: SortDirection,
}

// TODO(pgao): move to common
// #[derive(Clone, Copy, Debug, Eq, PartialEq, Display, Default)]
// pub enum SortDirection {
//     #[default]
//     Asc,
//     Desc,
// }
