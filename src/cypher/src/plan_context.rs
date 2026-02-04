use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use crate::plan_node::plan_base::PlanNodeId;
use crate::variable::VariableGenerator;

#[derive(Default)]
pub struct PlanContext {
    // pub sctx: Arc<dyn PlannerSession>,
    plan_node_gen: AtomicUsize,
    var_gen: Arc<VariableGenerator>,
}

impl PlanContext {
    pub fn plan_node_id(&self) -> PlanNodeId {
        self.plan_node_gen.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn var_gen(&self) -> &Arc<VariableGenerator> {
        &self.var_gen
    }
}
