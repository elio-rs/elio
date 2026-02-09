use std::sync::Arc;

use educe::Educe;
use elio_common::schema::Schema;

use super::*;
use crate::plan::plan_context::PlanContext;
use crate::plan::plan_node::plan_base::PlanBase;
use crate::plan::plan_node::{InnerNode, PlanExpr, PlanNode};

/// Generate one empty row
/// used to drive the create node/rel

#[derive(Debug, Clone)]
pub struct Unit {
    pub base: PlanBase,
    pub(crate) inner: UnitInner,
}

impl Unit {
    pub fn new(ctx: Arc<PlanContext>) -> Self {
        let inner = UnitInner { ctx };
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Unit {
    type Inner = UnitInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        XmlNode::simple_record("Unit", vec![], vec![])
    }
}

#[derive(Educe)]
#[educe(Debug, Clone)]
pub struct UnitInner {
    #[educe(Debug(ignore))]
    ctx: Arc<PlanContext>,
}

impl InnerNode for UnitInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(Arc::new(Schema::empty()), self.ctx.clone())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![]
    }
}
