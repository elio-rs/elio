use std::collections::HashSet;

use elio_catalog::FunctionCatalog;
use elio_common::scalar::ScalarValue;
use elio_common::schema::Variable;
use elio_parser::ast::{self, ReturnItem};
use indexmap::IndexMap;
use itertools::Itertools;

use crate::binder::BindContext;
use crate::binder::builder::IrSingleQueryBuilder;
use crate::binder::expr::bind_expr;
use crate::binder::query::ClauseKind;
use crate::binder::scope::{Scope, ScopeItem};
use crate::error::{PlanError, SemanticError};
use crate::expr::{Constant, Expr, ExprNode, FilterExprs, VariableRef};
use crate::ir::order::SortItem;
use crate::ir::query_project::{
    AggregateProjection, DistinctProjection, Pagination, Projection, QueryProjection, RegularProjection,
};

pub enum BoundReturnItems {
    Project(Vec<(Variable, Expr)>),
    Aggregate(),
}

pub fn bind_return_items(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    scope: Scope,
    distinct: bool,
    for_clause: &ClauseKind,
    _return_items @ ast::ReturnItems { projection_kind, items }: &ast::ReturnItems,
) -> Result<Scope, PlanError> {
    let mut group_by = vec![];
    let mut existing = vec![];
    let mut agg = vec![];
    let mut post_proj = vec![];

    for item in items {
        let args = extract_top_level_aggregate(bctx, &item.expr)?;
        if args.is_empty() {
            group_by.push(item);
        } else {
            post_proj.push(item);
            agg.push(args);
        }
    }

    // WITH clause must be aliased if it contains an non-variable or function call expression
    if matches!(for_clause, ClauseKind::With) {
        for item in items.iter() {
            if item.alias.is_none() && !item.expr.is_variable() {
                return Err(SemanticError::return_item_must_be_aliased(&item.to_string(), "WITH").into());
            }
        }
    }

    // expand star
    if projection_kind.include_existing() {
        for in_item in scope.symbol_items() {
            if group_by
                .iter()
                .chain(post_proj.iter())
                .any(|ReturnItem { alias, .. }| match (alias, &in_item.symbol) {
                    (Some(new), Some(existing)) => new.eq(existing),
                    _ => false,
                })
            {
                // overwrite the existing item
                continue;
            } else {
                // add symbol to existing
                existing.push((
                    in_item.variable.clone(),
                    in_item.symbol.clone().unwrap(),
                    in_item.as_expr(),
                ));
            }
        }
    }

    let (mut agg_in_scope, group_by_expr) = {
        // regular projection or distinct or before aggregation
        // TODO(pgao): if there's no projection, the scope should be inscope
        // for the case WITH * [WHERE]
        let mut out_scope = Scope::empty();
        let mut projections = IndexMap::new();
        let clause = format!("{} Clause", for_clause);
        let ectx = bctx.derive_expr_context(&scope, &clause);
        for ReturnItem { expr, alias } in group_by {
            let bound_expr = bind_expr(&ectx, &bctx.outer_scopes, expr)?;
            let symbol = alias.clone().unwrap_or(expr.to_string());
            let var_name = bctx.variable_generator.named(&symbol);
            let item = ScopeItem {
                symbol: Some(symbol.to_string()),
                variable: var_name.clone(),
                expr: HashSet::from_iter(vec![*expr.clone()]),
                typ: bound_expr.typ(),
                bound_expr: None,
            };
            out_scope.add_item(item);
            projections.insert(var_name, bound_expr);
        }

        // add existing to out scope and projections
        for (var, symbol, expr) in existing.iter() {
            let item = ScopeItem {
                symbol: Some(symbol.to_string()),
                variable: var.clone(),
                expr: Default::default(),
                typ: expr.typ(),
                bound_expr: None,
            };
            out_scope.add_item(item);
            projections.insert(var.clone(), expr.clone());
        }

        (out_scope, projections)
    };

    if group_by_expr.is_empty() {
        // TODO(pgao): we should have an bind context here
        return Err(SemanticError::at_least_one_return_item("").into());
    }

    if post_proj.is_empty() {
        let query_project = {
            if distinct {
                let proj = DistinctProjection {
                    group_by: group_by_expr,
                    order_by: Default::default(),
                    pagination: Default::default(),
                    filter: FilterExprs::empty(),
                };
                let proj = Projection::Distinct(proj);
                QueryProjection::Project(proj)
            } else {
                let proj = Projection::Regular(RegularProjection {
                    items: group_by_expr,
                    order_by: Default::default(),
                    pagination: Default::default(),
                    filter: FilterExprs::empty(),
                });
                QueryProjection::Project(proj)
            }
        };
        builder.tail_mut().unwrap().with_projection(query_project);
        return Ok(agg_in_scope);
    }

    // handle aggreegation
    // MATCH (a)--(b) WITH *, a.name AS col1, SUM(a.age + b.age)/COUNT(b) AS b
    // in_scope = [*]
    // agg_in_scope = [*, a.name AS col1, a.age + b.age AS col2, b]
    // agg_out_scope = [*, col1, SUM(col2), COUNT(b)]
    // project_out_scope = [*, col1, SUM(col2)/COUNT(b) AS b]

    let post_proj_scope = agg_in_scope.clone();
    let ectx = bctx.derive_expr_context(&scope, "Aggregation");
    // bind agg args in agg_in_scope
    let agg_args = agg
        .iter()
        .flat_map(|expr| {
            let mut ret_args = vec![];
            for e in expr.iter() {
                if let ast::Expr::FunctionCall { args, .. } = e {
                    ret_args.extend(args);
                }
            }
            ret_args
        })
        .collect_vec();

    for arg in agg_args {
        let bound_expr = bind_expr(&ectx, &bctx.outer_scopes, arg)?;
        let item = if let Expr::VariableRef(VariableRef { name, .. }) = &bound_expr {
            ScopeItem {
                symbol: None,
                variable: name.clone(),
                expr: HashSet::from_iter(vec![arg.clone()]),
                typ: bound_expr.typ(),
                bound_expr: None,
            }
        } else {
            // create another variable
            let var_name = bctx.variable_generator.unnamed();
            ScopeItem {
                symbol: None,
                variable: var_name,
                expr: HashSet::from_iter(vec![arg.clone()]),
                typ: bound_expr.typ(),
                bound_expr: None,
            }
        };
        if agg_in_scope.resolve_variable(&item.variable).is_none() {
            agg_in_scope.add_item(item);
        }
    }

    // bind aggregate, works on agg_in_scope
    let mut agg_expr = IndexMap::default();

    let ectx = bctx.derive_expr_context(&agg_in_scope, "Aggregation");
    for item in agg.iter().flatten() {
        let bound_expr = bind_expr(&ectx, &bctx.outer_scopes, item)?;
        let var_name = bctx.variable_generator.unnamed();
        agg_expr.insert(var_name, bound_expr);
    }

    // add projection to builder
    {
        let agg_proj = AggregateProjection {
            group_by: group_by_expr.clone(),
            aggregate: agg_expr,
            order_by: Default::default(),
            pagination: Default::default(),
            filter: Default::default(),
        };
        builder
            .tail_mut()
            .unwrap()
            .with_projection(QueryProjection::Project(Projection::Aggregate(agg_proj)));
    }

    // post projection
    let mut out_scope = post_proj_scope.clone();
    {
        let mut projs = group_by_expr.clone();
        let ectx = bctx.derive_expr_context(&post_proj_scope, "Aggregation");
        for item in post_proj {
            let expr = bind_expr(&ectx, &bctx.outer_scopes, &item.expr)?;
            let symbol = item.alias.clone().unwrap_or(item.expr.to_string());
            let var_name = bctx.variable_generator.named(&symbol);
            let typ = expr.typ();

            projs.insert(var_name.clone(), expr);
            out_scope.add_item(ScopeItem {
                symbol: Some(symbol),
                variable: var_name,
                expr: HashSet::from_iter(vec![*item.expr.clone()]),
                typ,
                bound_expr: None,
            });
        }
        // add new part
        // TODO(pgao): we should add the imported variables to the tail
        builder.new_tail(Default::default());
        builder
            .tail_mut()
            .unwrap()
            .with_projection(QueryProjection::Project(Projection::Regular(RegularProjection {
                items: projs,
                order_by: Default::default(),
                pagination: Default::default(),
                filter: FilterExprs::empty(),
            })));
    }

    Ok(out_scope)
}

fn extract_top_level_aggregate(bctx: &BindContext, expr: &ast::Expr) -> Result<Vec<ast::Expr>, PlanError> {
    let mut aggs = vec![];
    match expr {
        ast::Expr::Unary { oprand, .. } => aggs.extend(extract_top_level_aggregate(bctx, oprand)?),
        ast::Expr::Binary { left, right, .. } => {
            let children = extract_top_level_aggregate(bctx, left)?
                .into_iter()
                .chain(extract_top_level_aggregate(bctx, right)?)
                .collect_vec();
            aggs.extend(children);
        }
        ast::Expr::FunctionCall { name, args, .. } => {
            let func = resolve_function(bctx, name)?;
            if func.func.is_agg {
                aggs.push(expr.clone());
            } else {
                let children = args
                    .iter()
                    .map(|x| extract_top_level_aggregate(bctx, x))
                    .collect::<Result<Vec<_>, _>>()?;
                aggs.extend(children.into_iter().flatten());
            }
        }
        _ => (),
    };
    Ok(aggs)
}

// TODO(pgao): should catalog with static lifetime
fn resolve_function(bctx: &BindContext, name: &str) -> Result<FunctionCatalog, PlanError> {
    bctx.session()
        .get_function_by_name(name)
        .cloned()
        .ok_or(PlanError::from(SemanticError::unknown_function(name, "")))
}

pub fn bind_order_by(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    scope: &Scope,
    _order_by @ ast::OrderBy { items }: &ast::OrderBy,
) -> Result<(), PlanError> {
    let mut bound_items = vec![];

    let ectx = bctx.derive_expr_context(scope, "OrderBy");
    ectx.sema_flags.reject_aggregate();
    ectx.sema_flags.reject_outer_reference();

    for item in items.iter() {
        let expr = bind_expr(&ectx, &bctx.outer_scopes, &item.expr)?;
        bound_items.push(SortItem {
            expr: Box::new(expr),
            direction: item.direction,
        });
    }

    builder
        .tail_mut()
        .unwrap()
        .update_projection(|p| p.set_order_by(bound_items));
    Ok(())
}

pub fn bind_pagination(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    scope: &Scope,
    skip: Option<&ast::Expr>,
    limit: Option<&ast::Expr>,
) -> Result<(), PlanError> {
    let mut pagination = Pagination::default();

    if let Some(skip) = skip {
        let ectx = bctx.derive_expr_context(scope, "Skip");
        ectx.sema_flags.reject_aggregate();
        ectx.sema_flags.reject_outer_reference();
        let expr = bind_expr(&ectx, &bctx.outer_scopes, skip)?;
        if let Expr::Constant(Constant {
            data: Some(ScalarValue::Integer(i)),
            ..
        }) = expr
            && i >= 0
        {
            pagination.offset = i as u64;
        } else {
            return Err(SemanticError::invalid_pagination_offset_type(&skip.to_string()).into());
        }
    }

    if let Some(limit) = limit {
        let ectx = bctx.derive_expr_context(scope, "Limit");
        ectx.sema_flags.reject_aggregate();
        ectx.sema_flags.reject_outer_reference();
        let expr = bind_expr(&ectx, &bctx.outer_scopes, limit)?;
        if let Expr::Constant(Constant {
            data: Some(ScalarValue::Integer(i)),
            ..
        }) = expr
            && i >= 0
        {
            pagination.limit = i as u64;
        } else {
            return Err(SemanticError::invalid_pagination_limit_type(&limit.to_string()).into());
        }
    }

    builder
        .tail_mut()
        .unwrap()
        .update_projection(|p| p.set_pagination(pagination));
    Ok(())
}
