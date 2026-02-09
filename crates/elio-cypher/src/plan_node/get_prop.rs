use std::sync::Arc;

use elio_common::schema::Schema;
use elio_common::variable::VariableName;

use super::*;
use crate::plan_node::plan_base::PlanBase;
use crate::plan_node::{InnerNode, PlanExpr};

/// Fetch all properties of given entity
/// This should be an enforce operator
#[derive(Clone, Debug)]
pub struct GetProperty {
    pub base: PlanBase,
    pub(crate) inner: GetPropertyInner,
}

impl GetProperty {
    pub fn new(inner: GetPropertyInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for GetProperty {
    type Inner = GetPropertyInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![("entities", Pretty::from(self.inner.entities.join(", ")))];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("GetProperty", fields, children)
    }
}

#[derive(Clone, Debug)]
pub struct GetPropertyInner {
    pub input: Box<PlanExpr>,
    pub entities: Vec<VariableName>,
}

impl GetPropertyInner {
    fn build_schema(&self) -> Arc<Schema> {
        self.input.schema()
    }
}

impl InnerNode for GetPropertyInner {
    fn build_base(&self) -> PlanBase {
        let schema = self.build_schema();
        PlanBase::new(schema, self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
