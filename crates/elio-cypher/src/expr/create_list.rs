use elio_common::data_type::DataType;

use crate::expr::{Expr, ExprNode};

/// CreateList expression - creates a list from multiple elements.
///
/// This is a special expression (not a regular function) because:
/// 1. It accepts a variable number of arguments
/// 2. The return type `List<T>` depends on the element types
/// 3. It needs special type inference to find the common element type
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct CreateList {
    /// List elements
    pub elements: Vec<Expr>,
    /// The resolved list type (e.g., List<Int64>)
    typ: DataType,
}

impl CreateList {
    /// Create a new CreateList expression with inferred type.
    ///
    /// # Arguments
    /// * `elements` - The list elements
    /// * `elem_type` - The common element type (should be pre-computed by the binder)
    pub fn new(elements: Vec<Expr>, elem_type: DataType) -> Self {
        let typ = DataType::new_list(elem_type);
        Self { elements, typ }
    }

    /// Create an empty list with the specified element type.
    pub fn empty(elem_type: DataType) -> Self {
        Self::new(vec![], elem_type)
    }

    /// Returns true if this is an empty list.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Returns the element type of the list.
    pub fn elem_type(&self) -> DataType {
        match &self.typ {
            DataType::List(inner) => (**inner).clone(),
            _ => unreachable!("CreateList should have list type"),
        }
    }

    /// Pretty print the list expression.
    pub fn pretty(&self) -> String {
        format!(
            "[{}]",
            self.elements.iter().map(|e| e.pretty()).collect::<Vec<_>>().join(", ")
        )
    }
}

impl ExprNode for CreateList {
    fn typ(&self) -> DataType {
        self.typ.clone()
    }
}

impl From<CreateList> for Expr {
    fn from(value: CreateList) -> Self {
        Expr::CreateList(value)
    }
}
