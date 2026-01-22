use std::sync::Arc;

mod rewriter;
mod rule;
mod rules;
mod visitor;

pub use crate::optimizer::rewriter::*;
pub use crate::optimizer::rule::{OptimizationRule, Result, RewriteOrder, RuleContext};
pub use crate::optimizer::rules::*;
pub use crate::optimizer::visitor::*;
use crate::plan_node::PlanExpr;

pub struct RuleBasedOptimizer {
    rules: Vec<Arc<dyn OptimizationRule>>,
    max_iterations: usize,
}

impl RuleBasedOptimizer {
    pub fn new_once(rules: Vec<Arc<dyn OptimizationRule>>) -> Self {
        Self {
            rules,
            max_iterations: 1,
        }
    }

    pub fn new_fixed_point(rules: Vec<Arc<dyn OptimizationRule>>, max_iterations: usize) -> Self {
        Self { rules, max_iterations }
    }

    pub fn optimize(&self, plan: PlanExpr, ctx: &mut RuleContext<'_>) -> Result<PlanExpr> {
        let mut root = plan;
        for _ in 0..self.max_iterations {
            let mut changed = false;
            for rule in self.rules.iter() {
                let (new_root, rule_changed) = match rule.order() {
                    RewriteOrder::BottomUp => apply_rule_bottom_up(rule.as_ref(), root, ctx)?,
                    RewriteOrder::TopDown => apply_rule_top_down(rule.as_ref(), root, ctx)?,
                };
                root = new_root;
                changed |= rule_changed;
            }
            if !changed {
                break;
            }
        }
        Ok(root)
    }
}

fn apply_rule_bottom_up(
    rule: &dyn OptimizationRule,
    plan: PlanExpr,
    ctx: &mut RuleContext<'_>,
) -> Result<(PlanExpr, bool)> {
    let mut changed = false;
    let rewritten_children = plan.map_children_result(|child| -> Result<PlanExpr> {
        let (new_child, child_changed) = apply_rule_bottom_up(rule, child, ctx)?;
        if child_changed {
            changed = true;
        }
        Ok(new_child)
    })?;

    let candidate = rewritten_children;
    let rewritten = match rule.apply(candidate.clone(), ctx)? {
        Some(new_plan) => {
            changed = true;
            new_plan
        }
        None => candidate,
    };

    Ok((rewritten, changed))
}

fn apply_rule_top_down(
    rule: &dyn OptimizationRule,
    plan: PlanExpr,
    ctx: &mut RuleContext<'_>,
) -> Result<(PlanExpr, bool)> {
    let mut changed = false;
    let mut current = plan;
    if let Some(new_plan) = rule.apply(current.clone(), ctx)? {
        current = new_plan;
        changed = true;
    }

    let current = current.map_children_result(|child| -> Result<PlanExpr> {
        let (new_child, child_changed) = apply_rule_top_down(rule, child, ctx)?;
        if child_changed {
            changed = true;
        }
        Ok(new_child)
    })?;

    Ok((current, changed))
}
