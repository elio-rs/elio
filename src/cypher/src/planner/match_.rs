use elio_common::schema::Schema;

use super::*;
use crate::expr::FilterExprs;
use crate::ir::query_graph::QueryGraph;
use crate::plan_node::{Argument, ArgumentInner, CrossProduct, CrossProductInner, Filter, FilterInner, Unit};
use crate::planner::component::plan_qg_simple;

// plan the query graph in following order:
// 1. plan match parts
// 2. plan optional match parts
pub fn plan_match(
    ctx: &mut PlannerContext,
    _part @ IrSingleQueryPart {
        query_graph,
        query_project: _,
    }: &IrSingleQueryPart,
    is_rhs: bool, // true on this is an rhs part of a join
) -> Result<Box<PlanExpr>, PlanError> {
    // TODO(pgao): pushdown projection order by to query graph
    plan_query_graph(ctx, query_graph, is_rhs)
}

fn plan_query_graph(ctx: &mut PlannerContext, qg: &QueryGraph, is_rhs: bool) -> Result<Box<PlanExpr>, PlanError> {
    // get connected component
    let (qgs, remaining_filter) = qg.connected_component();

    if qgs.is_empty() {
        // if qg have imported variable, just put an argument here.
        if !qg.imported().is_empty() {
            let root = PlanExpr::Argument(Argument::new(ArgumentInner {
                variables: qg.imported().into_iter().cloned().collect_vec(),
                ctx: ctx.ctx.clone(),
            }));
            return Ok(root.boxed());
        }

        // if qg does not have any imported variable and this is an lhs query graph, put an Unit node here to drive the
        // execution.
        if !is_rhs {
            let root = PlanExpr::Unit(Unit::new(ctx.ctx.clone()));
            return Ok(root.boxed());
        }

        // other cases, put an empty
        let root = PlanExpr::empty(Schema::empty().into(), ctx.ctx.clone());
        return Ok(root.boxed());
    }

    // plan components
    let plans = qgs
        .iter()
        .map(|qg| plan_component(ctx, qg))
        .collect::<Result<Vec<_>, _>>()?;

    // connect the components and optional match
    let root = plan_connect_component_and_optional_match(ctx, plans, remaining_filter)?;

    Ok(root)
}

fn plan_component(ctx: &mut PlannerContext, qg: &QueryGraph) -> Result<Box<PlanExpr>, PlanError> {
    // we can have different qg planning strategy here
    plan_qg_simple(ctx, qg)
}

// Solve the qgs and remaining predicates with
// - CrossProduct if predicates are non-equal predicates
// - HashJoin if the predicates are all equal predicates
// TODO(pgao): plan optional matches
// TODO(pgao): use cost based method to connect components
// TODO(pgao): generate hash join for eq predicates
fn plan_connect_component_and_optional_match(
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

    // TODO(pgao): Optional match

    Ok(root)
}
