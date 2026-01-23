use std::sync::Arc;

use crate::error::PlanError;
use crate::optimizer::{OptimizationRule, RulePass, RulePassKind};

mod unnest_apply;
pub use unnest_apply::UnnestApplyRule;

#[allow(clippy::vec_init_then_push)]
pub fn logical_rule_passes() -> Vec<RulePass> {
    // unnest apply
    let mut passes = vec![];
    passes.push(RulePass::new(
        "unnest_apply",
        vec![Arc::new(UnnestApplyRule)],
        RulePassKind::Once,
    ));
    passes
}
