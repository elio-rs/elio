use crate::error::PlanError;
use crate::ir::mutating_pattern::MutatingPattern;
use crate::ir::query::{IrSingleQuery, IrSingleQueryPart};
use crate::ir::query_project::QueryProjection;
use crate::plan_node::{Apply, ApplyInner, BlackHole, BlackHoleInner, PlanExpr};
use crate::planner::PlannerContext;
use crate::planner::create::plan_create;
use crate::planner::load::plan_load;
use crate::planner::match_::plan_match;
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
    if single_query.parts.last().unwrap().query_project.is_none() {
        root = PlanExpr::BlackHole(BlackHole::new(BlackHoleInner { input: root })).boxed();
    }

    Ok(root)
}

fn plan_head(
    ctx: &mut PlannerContext,
    part @ IrSingleQueryPart {
        query_graph,
        query_project,
    }: &IrSingleQueryPart,
) -> Result<Box<PlanExpr>, PlanError> {
    // Check if this is a LOAD clause - if so, handle it specially
    // NB: if this is an load clause, then there must be no query graph here.
    if let Some(QueryProjection::Load(ir_load)) = query_project {
        assert!(query_graph.mutating_patterns.is_empty());
        assert!(query_graph.rels.is_empty());
        // LOAD is the root - no match or mutating patterns
        return plan_load(ctx, ir_load);
    }

    // plan match
    let mut root = plan_match(ctx, part, false)?;
    // plan updating pattern
    for mutating_pattern in query_graph.mutating_patterns.iter() {
        root = plan_mutating_pattern(ctx, root, mutating_pattern)?;
    }
    // plan projection
    if let Some(proj) = query_project {
        root = plan_query_projection(ctx, root, proj)?;
    }
    Ok(root)
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
    part @ IrSingleQueryPart {
        query_graph,
        query_project,
    }: &IrSingleQueryPart,
) -> Result<Box<PlanExpr>, PlanError> {
    // plan rhs
    let rhs_plan = plan_match(ctx, part, true)?;

    // plan apply
    let mut root = plan_apply(ctx, lhs_plan, rhs_plan)?;

    // plan mutate pattern
    for mutating_pattern in query_graph.mutating_patterns.iter() {
        root = plan_mutating_pattern(ctx, root, mutating_pattern)?;
    }
    // plan projection
    if let Some(proj) = query_project {
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
