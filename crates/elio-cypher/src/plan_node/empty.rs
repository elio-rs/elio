use std::sync::Arc;

use educe::Educe;
use elio_common::schema::Schema;

use super::*;
use crate::plan_context::PlanContext;
use crate::plan_node::plan_base::PlanBase;
use crate::plan_node::{InnerNode, PlanExpr, PlanNode};

#[derive(Debug, Clone)]
pub struct Empty {
    pub base: PlanBase,
    pub(crate) inner: EmptyInner,
}

impl Empty {
    pub fn new(schema: Arc<Schema>, ctx: Arc<PlanContext>) -> Self {
        let inner = EmptyInner { ctx, schema };
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Empty {
    type Inner = EmptyInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        XmlNode::simple_record("Empty", vec![], vec![])
    }
}

#[derive(Educe)]
#[educe(Debug, Clone)]
pub struct EmptyInner {
    #[educe(Debug(ignore))]
    ctx: Arc<PlanContext>,
    schema: Arc<Schema>,
}

impl InnerNode for EmptyInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.schema.clone(), self.ctx.clone())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![]
    }
}
