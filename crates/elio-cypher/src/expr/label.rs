use elio_common::IrToken;
use elio_common::data_type::DataType;

use crate::expr::{Expr, ExprNode};

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct HasLabel {
    pub entity: Box<Expr>,
    pub label_or_rel: IrToken,
}

impl ExprNode for HasLabel {
    fn typ(&self) -> DataType {
        DataType::Bool
    }
}

impl From<HasLabel> for Expr {
    fn from(val: HasLabel) -> Self {
        Expr::HasLabel(val)
    }
}
