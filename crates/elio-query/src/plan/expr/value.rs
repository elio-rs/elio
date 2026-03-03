use elio_common::data_type::{F64, LogicalType};
use elio_common::scalar::ScalarValue;

use crate::plan::expr::{Expr, ExprNode};

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct Constant {
    pub data: Option<ScalarValue>,
    pub typ: Option<LogicalType>,
}

impl Constant {
    pub fn boolean(b: bool) -> Self {
        Self {
            data: Some(ScalarValue::Bool(b)),
            typ: Some(LogicalType::BOOL),
        }
    }

    pub fn integer(i: i64) -> Self {
        Self {
            data: Some(ScalarValue::Integer(i)),
            typ: Some(LogicalType::INTEGER),
        }
    }

    pub fn float(f: F64) -> Self {
        Self {
            data: Some(ScalarValue::Float(f)),
            typ: Some(LogicalType::FLOAT),
        }
    }

    pub fn string(s: String) -> Self {
        Self {
            data: Some(ScalarValue::String(s)),
            typ: Some(LogicalType::STRING),
        }
    }

    pub fn untyped_null() -> Self {
        Self { data: None, typ: None }
    }

    pub fn typed_null(typ: LogicalType) -> Self {
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
    fn typ(&self) -> LogicalType {
        self.typ.clone().unwrap_or(LogicalType::ANY)
    }
}

impl From<Constant> for Expr {
    fn from(val: Constant) -> Self {
        Expr::Constant(val)
    }
}
