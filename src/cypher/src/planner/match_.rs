use elio_common::schema::Schema;

use super::*;
use crate::ir::query_graph::QueryGraph;
use crate::plan_node::{Argument, ArgumentInner, Unit};
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
    let qgs = qg.connected_component();
    if qgs.len() > 1 {
        return Err(PlanError::not_supported("mutliple query graph not supported."));
    }

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

    // plan component
    let plans = qgs
        .iter()
        .map(move |qg| plan_component(ctx, qg))
        .collect::<Result<Vec<_>, _>>()?;

    // TODO(pgao): connect component by cartisen product

    // TODO(pgao): plan optional match
    Ok(plans[0].clone())
}

fn plan_component(ctx: &mut PlannerContext, qg: &QueryGraph) -> Result<Box<PlanExpr>, PlanError> {
    // we can have different qg planning strategy here
    plan_qg_simple(ctx, qg)
}
