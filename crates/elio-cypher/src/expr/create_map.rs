use std::sync::Arc;

use elio_common::data_type::DataType;

use crate::expr::{Expr, ExprNode};

/// Create Property Map
/// TODO(pgao): should guaranteee Expr return type is only can be viewed as propety value types
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct CreateStruct {
    pub properties: Vec<(Arc<str>, Expr)>,
    typ: DataType,
}

impl CreateStruct {
    pub fn new(properties: Vec<(Arc<str>, Expr)>) -> Self {
        let typ = DataType::new_struct(properties.iter().map(|(name, expr)| (name.clone(), expr.typ())));
        Self { properties, typ }
    }

    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }
}

impl ExprNode for CreateStruct {
    fn typ(&self) -> DataType {
        self.typ.clone()
    }
}

impl From<CreateStruct> for Expr {
    fn from(value: CreateStruct) -> Self {
        Expr::CreateStruct(value)
    }
}
