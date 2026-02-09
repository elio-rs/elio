use super::*;
use crate::plan::expr::FilterExprs;
use crate::plan::ir::query_graph::QueryGraph;
use crate::plan::plan_node::{CrossProduct, CrossProductInner, Filter, FilterInner};
use crate::planner::component::plan_qg_simple;

// plan the query graph in following order:
// 1. plan match parts
// 2. plan optional match parts
// NB: Require at least one match pattern or optional match pattern
pub fn plan_reading_pattern(
    ctx: &mut PlannerContext,
    _part @ IrSingleQueryPart {
        input_binding: _,
        match_pattern,
        optional_match_patterns,
        mutating_patterns: _,
        projection: _,
    }: &IrSingleQueryPart,
) -> Result<Box<PlanExpr>, PlanError> {
    let mut root = None;
    // match pattern
    if let Some(match_pattern) = match_pattern {
        root = Some(plan_match_pattern(ctx, match_pattern, false)?);
    };
    // TODO(pgao): plan optional match pattern and connect to match pattern
    if !optional_match_patterns.is_empty() {
        return Err(PlanError::not_supported("optional match not supported yet"));
    }

    root.ok_or_else(|| PlanError::bad_plan("at least one match pattern is required"))
}

fn plan_query_graph(ctx: &mut PlannerContext, qg: &QueryGraph, is_optional: bool) -> Result<Box<PlanExpr>, PlanError> {
    if is_optional {
        return Err(PlanError::not_supported("optional match not supported yet"));
    }
    // connected components
    let (qgs, remaining_filter) = qg.connected_component();
    assert!(!qgs.is_empty());

    // plan components
    let plans = qgs
        .iter()
        .map(|qg| plan_component(ctx, qg))
        .collect::<Result<Vec<_>, _>>()?;
    // connect components by Joins.
    let root = plan_connect_components(ctx, plans, remaining_filter)?;
    Ok(root)
}

fn plan_match_pattern(
    ctx: &mut PlannerContext,
    match_pattern: &QueryGraph,
    is_optional: bool,
) -> Result<Box<PlanExpr>, PlanError> {
    plan_query_graph(ctx, match_pattern, is_optional)
}

fn plan_component(ctx: &mut PlannerContext, qg: &QueryGraph) -> Result<Box<PlanExpr>, PlanError> {
    // we can have different qg planning strategy here
    plan_qg_simple(ctx, qg)
}

// Solve the qgs and remaining predicates with
// - CrossProduct if predicates are non-equal predicates
// - HashJoin if the predicates are all equal predicates
// TODO(pgao): use cost based method to connect components
// TODO(pgao): generate hash join for eq predicates
fn plan_connect_components(
    _ctx: &mut PlannerContext,
    components: Vec<Box<PlanExpr>>,
    predicates: FilterExprs,
) -> Result<Box<PlanExpr>, PlanError> {
    // if there's only one component, just return the plan
    if components.len() == 1 {
        return Ok(components.into_iter().next().unwrap());
    }

    // if there's more than one component, connect them by cross product
    let mut plan_iter = components.into_iter();
    let mut root = plan_iter.next().expect("at least one component is required");
    for plan in plan_iter {
        root = PlanExpr::CrossProduct(CrossProduct::new(CrossProductInner {
            left: root,
            right: plan,
        }))
        .into();
    }

    // filter
    if !predicates.is_true() {
        root = PlanExpr::Filter(Filter::new(FilterInner {
            input: root,
            condition: predicates,
        }))
        .boxed();
    }

    Ok(root)
}
