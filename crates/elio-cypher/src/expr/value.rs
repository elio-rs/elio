use elio_common::data_type::{DataType, F64};
use elio_common::scalar::ScalarValue;

use crate::expr::{Expr, ExprNode};

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct Constant {
    pub data: Option<ScalarValue>,
    pub typ: Option<DataType>,
}

impl Constant {
    pub fn boolean(b: bool) -> Self {
        Self {
            data: Some(ScalarValue::Bool(b)),
            typ: Some(DataType::Bool),
        }
    }

    pub fn integer(i: i64) -> Self {
        Self {
            data: Some(ScalarValue::Integer(i)),
            typ: Some(DataType::Integer),
        }
    }

    pub fn float(f: F64) -> Self {
        Self {
            data: Some(ScalarValue::Float(f)),
            typ: Some(DataType::Float),
        }
    }

    pub fn string(s: String) -> Self {
        Self {
            data: Some(ScalarValue::String(s)),
            typ: Some(DataType::String),
        }
    }

    pub fn untyped_null() -> Self {
        Self { data: None, typ: None }
    }

    pub fn typed_null(typ: DataType) -> Self {
        Self {
            data: None,
            typ: Some(typ),
        }
    }

    pub fn is_untyped_null(&self) -> bool {
        self.typ.is_none() && self.data.is_none()
    }

    pub fn pretty(&self) -> String {
        self.data
            .as_ref()
            .map_or("null".to_string(), |d| d.as_scalar_ref().to_string())
    }

    pub fn is_null(&self) -> bool {
        self.data.is_none()
    }
}

impl ExprNode for Constant {
    fn typ(&self) -> DataType {
        self.typ.clone().unwrap_or(DataType::Any)
    }
}

impl From<Constant> for Expr {
    fn from(val: Constant) -> Self {
        Expr::Constant(val)
    }
}
