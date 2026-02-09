use itertools::Itertools;

use crate::plan::error::PlanError;
use crate::plan::ir::mutating_pattern::MutatingPattern;
use crate::plan::ir::query::{IrSingleQuery, IrSingleQueryPart};
use crate::plan::ir::query_project::QueryProjection;
use crate::plan::plan_node::{Apply, ApplyInner, Argument, ArgumentInner, BlackHole, BlackHoleInner, PlanExpr, Unit};
use crate::planner::PlannerContext;
use crate::planner::create::plan_create;
use crate::planner::load::plan_load;
use crate::planner::match_::plan_reading_pattern;
use crate::planner::project::plan_query_projection;

pub fn plan_single_query(
    ctx: &mut PlannerContext,
    single_query @ IrSingleQuery { parts }: &IrSingleQuery,
) -> Result<Box<PlanExpr>, PlanError> {
    assert!(!parts.is_empty());
    let mut part_iter = parts.iter();
    let head = part_iter.next().unwrap();

    // plan head
    let mut root = plan_head(ctx, head)?;

    // plan tail
    for tail in part_iter {
        root = plan_tail_part(ctx, root, tail)?
    }

    // if not in subquery context and there's no return clause(empty projection), then generate an BlackHole PlanNode.
    // this is for the case of `CREATE (:Person{name:'Alex'})`
    // TODO(pgao): check if in subquery
    if single_query.parts.last().unwrap().projection.is_none() {
        root = PlanExpr::BlackHole(BlackHole::new(BlackHoleInner { input: root })).boxed();
    }

    Ok(root)
}

fn plan_head(
    ctx: &mut PlannerContext,
    part @ IrSingleQueryPart {
        input_binding,
        match_pattern,
        optional_match_patterns,
        mutating_patterns,
        projection,
    }: &IrSingleQueryPart,
) -> Result<Box<PlanExpr>, PlanError> {
    assert!(input_binding.is_empty(), "input binding should be empty for head part");
    // Check if this is a LOAD clause - if so, handle it specially
    // NB: if this is an load clause, then there must be no query graph here.
    if let Some(QueryProjection::Load(ir_load)) = projection {
        assert!(match_pattern.is_none());
        assert!(optional_match_patterns.is_empty());
        assert!(mutating_patterns.is_empty());
        // LOAD is the root - no match or mutating patterns
        return plan_load(ctx, ir_load);
    }

    let mut root = None;
    // plan match pattern and optional match pattern
    if match_pattern.is_some() || !optional_match_patterns.is_empty() {
        root = Some(plan_reading_pattern(ctx, part)?);
    }

    // plan updating pattern
    if !mutating_patterns.is_empty() {
        if root.is_none() {
            // put an unit here to drive the mutating pattern
            root = Some(PlanExpr::Unit(Unit::new(ctx.ctx.clone())).boxed());
        }
        for mutating_pattern in mutating_patterns.iter() {
            if let Some(lhs) = root {
                root = Some(plan_mutating_pattern(ctx, lhs, mutating_pattern)?);
            }
        }
    }

    // plan projection
    if let Some(proj) = projection {
        // put an unit here to drive the projection
        if root.is_none() {
            root = Some(PlanExpr::Unit(Unit::new(ctx.ctx.clone())).boxed());
        }
        root = Some(plan_query_projection(ctx, root.unwrap(), proj)?);
    }
    Ok(root.unwrap())
}

fn plan_mutating_pattern(
    ctx: &mut PlannerContext,
    root: Box<PlanExpr>,
    mutating_pattern: &MutatingPattern,
) -> Result<Box<PlanExpr>, PlanError> {
    match mutating_pattern {
        MutatingPattern::Create(create) => plan_create(ctx, root, create),
    }
}

fn plan_tail_part(
    ctx: &mut PlannerContext,
    lhs_plan: Box<PlanExpr>,
    part: &IrSingleQueryPart,
) -> Result<Box<PlanExpr>, PlanError> {
    let mut root = if part.has_reading_pattern() {
        // plan rhs
        let rhs_plan = plan_reading_pattern(ctx, part)?;
        // plan apply
        plan_apply(ctx, lhs_plan, rhs_plan)?
    } else if !part.input_binding.is_empty() {
        // if have input_binding, plan apply
        let rhs_plan = PlanExpr::Argument(Argument::new(ArgumentInner {
            variables: part.input_bindings().clone().into_iter().collect_vec(),
            ctx: ctx.ctx.clone(),
        }))
        .boxed();
        plan_apply(ctx, lhs_plan, rhs_plan)?
    } else {
        lhs_plan
    };

    // plan mutate pattern
    for mutating_pattern in part.mutating_patterns.iter() {
        root = plan_mutating_pattern(ctx, root, mutating_pattern)?;
    }
    // plan projection
    if let Some(proj) = &part.projection {
        root = plan_query_projection(ctx, root, proj)?;
    }
    Ok(root)
}

fn plan_apply(
    _ctx: &mut PlannerContext,
    lhs_plan: Box<PlanExpr>,
    rhs_plan: Box<PlanExpr>,
) -> Result<Box<PlanExpr>, PlanError> {
    Ok(PlanExpr::Apply(Apply::new(ApplyInner {
        left: lhs_plan,
        right: rhs_plan,
    }))
    .boxed())
}
