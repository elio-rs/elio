use elio_common::data_type::DataType;
use elio_common::schema::Variable;
use elio_common::variable::VariableName;

use crate::expr::{Expr, ExprNode};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct VariableRef {
    pub name: VariableName,
    pub typ: DataType,
}

impl VariableRef {
    pub fn new_unchecked(name: VariableName, typ: DataType) -> Self {
        Self { name, typ }
    }

    pub fn from_variable(var: &Variable) -> Self {
        Self::new_unchecked(var.name.clone(), var.typ.clone())
    }

    pub fn as_variable(&self) -> Variable {
        Variable::new(&self.name, &self.typ)
    }
}

impl ExprNode for VariableRef {
    fn typ(&self) -> DataType {
        self.typ.clone()
    }
}

impl From<VariableRef> for Expr {
    fn from(val: VariableRef) -> Self {
        Expr::VariableRef(val)
    }
}
