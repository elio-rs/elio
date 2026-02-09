use std::sync::Arc;

use educe;
use educe::Educe;
use elio_common::schema::Schema;

use crate::plan_context::PlanContext;
use crate::props::order::Ordering;

pub type PlanNodeId = usize;

// Plan logical properties
#[derive(Educe)]
#[educe(Debug, Clone)]
pub struct PlanBase {
    // logical properties
    id: PlanNodeId,
    schema: Arc<Schema>,
    // pub func_deps: FuncDepSet,
    // physical properties
    attrs: PlanNodeAttrs,
    // TODO(pgao): properties enforcer
    // context
    #[educe(Debug(ignore))]
    ctx: Arc<PlanContext>,
}

impl PlanBase {
    pub fn new(schema: Arc<Schema>, ctx: Arc<PlanContext>) -> Self {
        let id = ctx.plan_node_id();
        Self {
            id,
            schema,
            attrs: PlanNodeAttrs::empty(),
            ctx,
        }
    }

    pub fn id(&self) -> PlanNodeId {
        self.id
    }

    pub fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    pub fn ctx(&self) -> Arc<PlanContext> {
        self.ctx.clone()
    }
}

#[derive(Debug, Clone)]
pub struct PlanNodeAttrs {
    // bitset to mark if ordering is valid
    // none for ordering is not derived and needs to be computed
    pub order: Option<Ordering>,
}

impl PlanNodeAttrs {
    pub fn empty() -> Self {
        Self { order: None }
    }
}
