use crate::plan::error::PlanError;
use crate::plan::plan_node::PlanExpr;
use crate::planner::PlannerContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteOrder {
    TopDown,
    BottomUp,
}

pub struct RuleContext<'a> {
    pub plan_ctx: &'a mut PlannerContext,
}

pub trait OptimizationRule: Send + Sync {
    fn name(&self) -> &str;

    /// Whether to apply the rule top-down or bottom-up.
    fn order(&self) -> RewriteOrder {
        RewriteOrder::BottomUp
    }

    /// Try to rewrite a single plan node. Return `Some(new_plan)` if transformed.
    fn apply(&self, plan: PlanExpr, ctx: &mut RuleContext<'_>) -> Result<Option<PlanExpr>, PlanError>;
}
