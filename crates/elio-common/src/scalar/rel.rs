use super::*;
use crate::IrToken;

// TODO(pgao): we needs to hash and Eq only on id.
// in varexpand, we needs to test if rel have already visited.
#[derive(Debug, Clone, Default, Eq, PartialEq, Hash, derive_more::Display, ScalarPartialOrd)]
#[scalar_partial_ord(id)]
#[display(
    "{{id: {}, rtype: {}, start: {}, end: {}, props: {}}}",
    id,
    reltype,
    start_id,
    end_id,
    props
)]
pub struct RelValue {
    pub id: RelationshipId,
    pub reltype: Arc<str>,
    pub start_id: NodeId,
    pub end_id: NodeId,
    pub props: StructValue,
}

impl ScalarVTable for RelValue {
    type RefType<'a> = RelValueRef<'a>;

    fn as_scalar_ref(&self) -> Self::RefType<'_> {
        RelValueRef {
            id: self.id,
            reltype: &self.reltype,
            start_id: self.start_id,
            end_id: self.end_id,
            props: self.props.as_scalar_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display, ScalarPartialOrd)]
#[scalar_partial_ord(id)]
#[display(
    "{{id: {}, rtype: {}, start: {}, end: {}, props: {}}}",
    id,
    reltype,
    start_id,
    end_id,
    props
)]
pub struct RelValueRef<'a> {
    pub id: RelationshipId,
    pub reltype: &'a Arc<str>,
    pub start_id: NodeId,
    pub end_id: NodeId,
    pub props: StructValueRef<'a>,
}

impl<'a> RelValueRef<'a> {
    pub fn has_label(&self, label: &IrToken) -> bool {
        self.reltype == label.name()
    }
}

impl<'a> ScalarRefVTable<'a> for RelValueRef<'a> {
    type ScalarType = RelValue;

    fn to_owned_scalar(&self) -> Self::ScalarType {
        RelValue {
            id: self.id,
            reltype: self.reltype.clone(),
            start_id: self.start_id,
            end_id: self.end_id,
            props: self.props.to_owned_scalar(),
        }
    }
}

impl<'a> RelValueRef<'a> {
    pub fn relative_dir(&self, node: NodeId) -> Option<RelDirection> {
        if node == self.start_id {
            Some(RelDirection::Out)
        } else if node == self.end_id {
            Some(RelDirection::In)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Hash, derive_more::Display, ScalarPartialOrd)]
#[scalar_partial_ord(id)]
#[display("VirtualRel{{id: {}, rtype: {}, start: {}, end: {}}}", id, reltype, start_id, end_id)]
pub struct VirtualRel {
    pub id: RelationshipId,
    pub reltype: Arc<str>,
    pub start_id: NodeId,
    pub end_id: NodeId,
}

impl ScalarVTable for VirtualRel {
    type RefType<'a> = VirtualRelRef<'a>;

    fn as_scalar_ref(&self) -> Self::RefType<'_> {
        VirtualRelRef {
            id: self.id,
            reltype: &self.reltype,
            start_id: self.start_id,
            end_id: self.end_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display, ScalarPartialOrd)]
#[scalar_partial_ord(id)]
#[display("{{id: {}, rtype: {}, start: {}, end: {}}}", id, reltype, start_id, end_id)]
pub struct VirtualRelRef<'a> {
    pub id: RelationshipId,
    pub reltype: &'a str,
    pub start_id: NodeId,
    pub end_id: NodeId,
}

impl<'a> ScalarRefVTable<'a> for VirtualRelRef<'a> {
    type ScalarType = VirtualRel;

    fn to_owned_scalar(&self) -> Self::ScalarType {
        VirtualRel {
            id: self.id,
            reltype: self.reltype.into(),
            start_id: self.start_id,
            end_id: self.end_id,
        }
    }
}

impl<'a> EntityScalarRef for RelValueRef<'a> {
    fn has_ir_label(&self, label: &IrToken) -> bool {
        self.has_label(label)
    }
}
