use std::sync::Arc;

pub type VariableName = Arc<str>;

/// elements to construct to path
pub enum PathElement {
    Node(VariableName),
    Rel(VariableName),
}

impl PathElement {
    pub fn variable(&self) -> &VariableName {
        match self {
            PathElement::Node(var) => var,
            PathElement::Rel(var) => var,
        }
    }
}
