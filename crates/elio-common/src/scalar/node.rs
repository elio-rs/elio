pub use super::*;
use crate::IrToken;

#[derive(Debug, Clone, Default, Eq, PartialEq, Hash, derive_more::Display, ScalarPartialOrd)]
#[scalar_partial_ord(id)]
#[display("{{id: {}, labels: [{}], props: {}}}", id, labels.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", "), props)]
pub struct NodeValue {
    pub id: NodeId,
    pub labels: Vec<Arc<str>>,
    // TODO(pgao): lazy deserialize
    pub props: StructValue,
}

impl ScalarVTable for NodeValue {
    type RefType<'a> = NodeValueRef<'a>;

    fn as_scalar_ref(&self) -> Self::RefType<'_> {
        NodeValueRef {
            id: self.id,
            labels: &self.labels,
            props: self.props.as_scalar_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display, ScalarPartialOrd)]
#[scalar_partial_ord(id)]
#[display("{{id: {}, labels: [{}], props: {}}}", id, labels.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", "), props)]
pub struct NodeValueRef<'a> {
    pub id: NodeId,
    pub labels: &'a [Arc<str>],
    pub props: StructValueRef<'a>,
}

impl<'a> NodeValueRef<'a> {
    // TODO(pgao): optimize by using token id, instead of using string compare
    pub fn has_label(&self, label: &IrToken) -> bool {
        self.labels.iter().any(|l| l == label.name())
    }
}

impl<'a> ScalarRefVTable<'a> for NodeValueRef<'a> {
    type ScalarType = NodeValue;

    fn to_owned_scalar(&self) -> Self::ScalarType {
        NodeValue {
            id: self.id,
            labels: self.labels.to_vec(),
            props: self.props.to_owned_scalar(),
        }
    }
}

impl<'a> EntityScalarRef for NodeValueRef<'a> {
    fn has_ir_label(&self, label: &IrToken) -> bool {
        self.has_label(label)
    }
}
