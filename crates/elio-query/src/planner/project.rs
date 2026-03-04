use std::collections::HashSet;

use elio_common::order::ColumnOrder;
use elio_common::schema::Schema;
use indexmap::IndexMap;
use itertools::Itertools;

use crate::plan::error::PlanError;
use crate::plan::expr::{AggCall, Expr, ExprNode, FilterExprs};
use crate::plan::ir::order::SortItem;
use crate::plan::ir::query_project::{
    AggregateProjection, DistinctProjection, Pagination, Projection, QueryProjection, RegularProjection, Unwind,
};
use crate::plan::plan_node::{
    Aggregate, AggregateInner, Distinct, DistinctInner, Filter, FilterInner, PaginationInner, PlanExpr, Project,
    ProjectInner, Sort, SortInner, UnwindInner,
};
use crate::planner::PlannerContext;

pub fn plan_query_projection(
    ctx: &mut PlannerContext,
    root: Box<PlanExpr>,
    query_project: &QueryProjection,
) -> Result<Box<PlanExpr>, PlanError> {
    match query_project {
        QueryProjection::Unwind(unwind) => plan_unwind(ctx, root, unwind),
        QueryProjection::Project(Projection::Regular(reg)) => plan_project(ctx, root, reg),
        QueryProjection::Project(Projection::Aggregate(agg)) => plan_aggregate(ctx, root, agg),
        QueryProjection::Project(Projection::Distinct(dist)) => plan_distinct(ctx, root, dist),
        QueryProjection::Load(_load) => {
            // Load is handled specially in plan_head, not through plan_query_projection
            // This branch should not be reached
            Err(PlanError::not_supported("Load should be planned separately"))
        }
    }
}

fn plan_project(
    ctx: &mut PlannerContext,
    root: Box<PlanExpr>,
    project @ RegularProjection {
        items,
        order_by,
        pagination,
        filter,
        output: _,
    }: &RegularProjection,
) -> Result<Box<PlanExpr>, PlanError> {
    let inner = ProjectInner {
        input: root,
        projections: items.clone().into_iter().collect_vec(),
    };
    let mut root: Box<PlanExpr> = Project::new(inner).into();

    if !filter.is_true() {
        root = plan_selection(ctx, root, filter)?;
    }

    if !order_by.is_empty() {
        root = plan_sort(ctx, root, order_by)?;
    }

    if !pagination.is_empty() {
        root = plan_pagination(ctx, root, pagination)?;
    }

    if let Some(extra_proj) = project.extra_project() {
        let inner = ProjectInner {
            input: root,
            projections: extra_proj.iter().map(|(k, v)| (k.clone(), v.clone())).collect_vec(),
        };
        root = Project::new(inner).into();
    }

    Ok(root)
}

fn plan_aggregate(
    ctx: &mut PlannerContext,
    mut root: Box<PlanExpr>,
    agg_proj @ AggregateProjection {
        group_by,
        aggregate,
        post_projection: _,
        order_by,
        pagination,
        filter,
    }: &AggregateProjection,
) -> Result<Box<PlanExpr>, PlanError> {
    // pre-aggregation projection
    let mut group_exprs = vec![];
    let mut agg_exprs = vec![];
    if agg_proj.needs_pre_agg_proj() {
        let mut pre_agg_proj = IndexMap::new();
        for (var, expr) in group_by.iter() {
            // if expr is varref or constant, let it pass through
            if expr.as_variable_ref().is_some() || expr.as_constant().is_some() {
                pre_agg_proj.insert(var.clone(), expr.clone());
                group_exprs.push((var.clone(), expr.clone()));
            } else {
                let proj_var = ctx.ctx.var_gen().unnamed();
                let proj_var_ref = Expr::new_variable_ref(proj_var.clone(), expr.typ().clone());
                pre_agg_proj.insert(proj_var.clone(), expr.clone());
                group_exprs.push((var.clone(), proj_var_ref));
            }
        }

        for (var, expr) in aggregate.iter() {
            let agg_call = expr.as_agg_call().unwrap();
            let mut new_children = vec![];
            for child in expr.children() {
                if let Some(child_var) = child.as_variable_ref() {
                    pre_agg_proj.insert(child_var.name.clone(), child.clone());
                    new_children.push(child.clone());
                } else {
                    let proj_var = ctx.ctx.var_gen().unnamed();
                    pre_agg_proj.insert(proj_var.clone(), child.clone());
                    new_children.push(Expr::new_variable_ref(proj_var, child.typ().clone()));
                }
            }
            agg_exprs.push((
                var.clone(),
                Expr::AggCall(AggCall::new_unchecked(
                    agg_call.function.clone(),
                    agg_call.function_data.clone(),
                    new_children,
                    agg_call.distinct,
                )),
            ));
        }

        let inner = ProjectInner {
            input: root,
            projections: pre_agg_proj.iter().map(|(k, v)| (k.clone(), v.clone())).collect_vec(),
        };
        root = Project::new(inner).into();
    } else {
        group_exprs = group_by.iter().map(|(k, v)| (k.clone(), v.clone())).collect_vec();
        agg_exprs = aggregate.iter().map(|(k, v)| (k.clone(), v.clone())).collect_vec();
    }

    // aggregate
    let inner = AggregateInner {
        input: root,
        group_by: group_exprs,
        aggregate: agg_exprs,
    };
    let mut root: Box<PlanExpr> = Aggregate::new(inner).into();

    // post-aggregation projection
    if let Some(extra_proj) = agg_proj.extra_project() {
        let inner = ProjectInner {
            input: root,
            projections: extra_proj.iter().map(|(k, v)| (k.clone(), v.clone())).collect_vec(),
        };
        root = Project::new(inner).into();
    }

    if !filter.is_true() {
        root = plan_selection(ctx, root, filter)?;
    }

    if !order_by.is_empty() {
        root = plan_sort(ctx, root, order_by)?;
    }

    if !pagination.is_empty() {
        root = plan_pagination(ctx, root, pagination)?;
    }

    Ok(root)
}

fn plan_distinct(
    ctx: &mut PlannerContext,
    root: Box<PlanExpr>,
    _project @ DistinctProjection {
        group_by,
        order_by,
        pagination,
        filter,
    }: &DistinctProjection,
) -> Result<Box<PlanExpr>, PlanError> {
    let inner = DistinctInner {
        input: root,
        group_exprs: group_by.iter().map(|(k, v)| (k.clone(), v.clone())).collect_vec(),
    };
    let mut root: Box<PlanExpr> = Distinct::new(inner).into();

    if !filter.is_true() {
        root = plan_selection(ctx, root, filter)?;
    }

    if !order_by.is_empty() {
        root = plan_sort(ctx, root, order_by)?;
    }

    if !pagination.is_empty() {
        root = plan_pagination(ctx, root, pagination)?;
    }

    Ok(root)
}

fn plan_unwind(
    _ctx: &mut PlannerContext,
    root: Box<PlanExpr>,
    Unwind { variable, expr }: &Unwind,
) -> Result<Box<PlanExpr>, PlanError> {
    let inner = UnwindInner {
        input: root,
        expr: expr.clone(),
        variable: variable.clone(),
    };
    Ok(crate::plan::plan_node::Unwind::new(inner).into())
}

// WITH a, a.id + 1 AS b, c ORDER BY c.id + 1 ASC
// 1. [optional] project, if sort item needs extra projection
// 2. sort
// 3. [optional] project, remove extra projection items
fn plan_sort(
    ctx: &mut PlannerContext,
    mut root: Box<PlanExpr>,
    order_by: &[SortItem],
) -> Result<Box<PlanExpr>, PlanError> {
    let mut extra_projections = vec![];
    let mut column_orders = vec![];
    for item in order_by {
        if item.needs_extra_project() {
            // TODO(pgao): we can have named once the expr have display trait
            let var = ctx.ctx.var_gen().unnamed();
            extra_projections.push((var.clone(), item.expr.clone()));
            column_orders.push(ColumnOrder {
                column: var,
                direction: item.direction,
            });
        } else {
            // SAFETY: its a simple variable, safe to unwrap
            column_orders.push(ColumnOrder {
                column: item.expr.as_variable_ref().unwrap().name.clone(),
                direction: item.direction,
            });
        }
    }

    // extra project
    if !extra_projections.is_empty() {
        // add extra project
        let empty = PlanExpr::empty(Schema::empty().into(), root.ctx());
        let mut inner = ProjectInner::new_from_input(std::mem::replace(&mut root, Box::new(empty)));
        extra_projections
            .iter()
            .for_each(|(name, expr)| inner.add_unchecked(name.clone(), expr.as_ref().clone()));
        root = Project::new(inner).into();
    }

    // sort
    {
        let inner = SortInner {
            input: root,
            items: column_orders,
        };
        root = Sort::new(inner).into();
    }

    // remove extra project
    // TODO(pgao): maybe we can use the opt rule to remove unnecessary project
    if !extra_projections.is_empty() {
        let empty = PlanExpr::empty(Schema::empty().into(), root.ctx());
        let mut inner = ProjectInner::new_from_input(std::mem::replace(&mut root, Box::new(empty)));
        let extra_names: HashSet<_> = extra_projections.iter().map(|(n, _)| n).collect();
        inner.retain(|(name, _expr)| !extra_names.contains(name));
        root = Project::new(inner).into();
    }
    Ok(root)
}

fn plan_pagination(
    _ctx: &mut PlannerContext,
    root: Box<PlanExpr>,
    _pagination @ Pagination { offset, limit }: &Pagination,
) -> Result<Box<PlanExpr>, PlanError> {
    let inner = PaginationInner {
        input: root,
        offset: *offset,
        limit: *limit,
    };
    Ok(crate::plan::plan_node::Pagination::new(inner).into())
}

fn plan_selection(
    _ctx: &mut PlannerContext,
    root: Box<PlanExpr>,
    filter: &FilterExprs,
) -> Result<Box<PlanExpr>, PlanError> {
    let inner = FilterInner {
        input: root,
        condition: filter.to_owned(),
    };

    Ok(Filter::new(inner).into())
}
