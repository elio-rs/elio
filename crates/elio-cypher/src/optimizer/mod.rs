use std::sync::Arc;

mod rewriter;
mod rule;
mod rules;
mod visitor;

use crate::error::PlanError;
pub use crate::optimizer::rewriter::*;
pub use crate::optimizer::rule::{OptimizationRule, RewriteOrder, RuleContext};
pub use crate::optimizer::rules::*;
pub use crate::optimizer::visitor::*;
use crate::plan_node::PlanExpr;
use crate::planner::{PlannerContext, RootPlan};

#[derive(Clone, Debug)]
pub enum RulePassKind {
    Once,
    FixedPoint(usize),
}

#[derive(Clone)]
pub struct RulePass {
    pub name: &'static str,
    pub rules: Vec<Arc<dyn OptimizationRule>>,
    pub kind: RulePassKind,
}

impl RulePass {
    pub fn new(name: &'static str, rules: Vec<Arc<dyn OptimizationRule>>, kind: RulePassKind) -> Self {
        Self { name, rules, kind }
    }

    pub fn optimizer(&self) -> RuleBasedOptimizer {
        match self.kind {
            RulePassKind::Once => RuleBasedOptimizer::new_once(self.rules.clone()),
            RulePassKind::FixedPoint(iter) => RuleBasedOptimizer::new_fixed_point(self.rules.clone(), iter),
        }
    }

    pub fn run(&self, plan: PlanExpr, ctx: &mut RuleContext<'_>) -> Result<PlanExpr, PlanError> {
        self.optimizer().optimize(plan, ctx)
    }
}

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

    pub fn optimize(&self, plan: PlanExpr, ctx: &mut RuleContext<'_>) -> Result<PlanExpr, PlanError> {
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

pub fn optimize_query(plan_ctx: &mut PlannerContext, root_plan: RootPlan) -> Result<RootPlan, PlanError> {
    let passes = logical_rule_passes();
    let RootPlan { plan, names } = root_plan;
    let mut plan = *plan;
    for pass in passes {
        let mut ctx = RuleContext { plan_ctx };
        plan = pass.run(plan, &mut ctx)?;
    }
    Ok(RootPlan {
        plan: plan.boxed(),
        names,
    })
}

fn apply_rule_bottom_up(
    rule: &dyn OptimizationRule,
    plan: PlanExpr,
    ctx: &mut RuleContext<'_>,
) -> Result<(PlanExpr, bool), PlanError> {
    let mut changed = false;
    let rewritten_children = plan.map_children_result(|child| -> Result<PlanExpr, PlanError> {
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
) -> Result<(PlanExpr, bool), PlanError> {
    let mut changed = false;
    let mut current = plan;
    if let Some(new_plan) = rule.apply(current.clone(), ctx)? {
        current = new_plan;
        changed = true;
    }

    let current = current.map_children_result(|child| -> Result<PlanExpr, PlanError> {
        let (new_child, child_changed) = apply_rule_top_down(rule, child, ctx)?;
        if child_changed {
            changed = true;
        }
        Ok(new_child)
    })?;

    Ok((current, changed))
}
