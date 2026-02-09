//! Unnest apply conditions
//!
//! 1. Apply(Lhs, Args) => Lhs

use super::*;
use crate::optimizer::{RewriteOrder, RuleContext};
use crate::plan::plan_node::{ApplyInner, PlanExpr, PlanNode};

#[derive(Default)]
pub struct UnnestApplyRule;

impl OptimizationRule for UnnestApplyRule {
    fn name(&self) -> &str {
        "unnest_apply"
    }

    fn order(&self) -> RewriteOrder {
        RewriteOrder::BottomUp
    }

    fn apply(&self, plan: PlanExpr, _ctx: &mut RuleContext<'_>) -> Result<Option<PlanExpr>, PlanError> {
        match plan {
            PlanExpr::Apply(apply) => {
                let ApplyInner { left, right } = apply.inner().clone();
                if right.as_argument().is_some() {
                    // Apply(Lhs, Argument) => Lhs
                    Ok(Some(*left))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
}
