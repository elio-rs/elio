use std::collections::{HashMap, HashSet};

use elio_common::scalar::ScalarValue;
use elio_common::schema::Variable;
use elio_common::variable::VariableName;
use elio_parser::ast::{self, ReturnItem};
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;

use crate::binder::BindContext;
use crate::binder::builder::IrSingleQueryBuilder;
use crate::binder::expr::{bind_expr, bind_where};
use crate::binder::query::ClauseKind;
use crate::binder::scope::{Scope, ScopeItem};
use crate::plan::error::{PlanError, SemanticError};
use crate::plan::expr::{Constant, Expr, ExprNode, ExprRewriter, FilterExprs, VariableRef};
use crate::plan::ir::order::SortItem;
use crate::plan::ir::query_project::{
    AggregateProjection, DistinctProjection, Pagination, Projection, QueryProjection, RegularProjection,
};
use crate::plan::variable::VariableGenerator;

pub enum BoundReturnItems {
    Project(Vec<(Variable, Expr)>),
    Aggregate(),
}

pub fn expand_star_expression(scope: &Scope, return_items: &ast::ReturnItems) -> Result<Vec<ReturnItem>, PlanError> {
    if return_items.projection_kind.include_existing() {
        let mut new_return_items = return_items.items.clone();
        for item in scope.symbol_items() {
            if new_return_items.iter().any(|x| match (&x.alias, &item.symbol) {
                (Some(a), Some(b)) => a.eq(b),
                _ => false,
            }) {
                continue;
            } else {
                new_return_items.push(ReturnItem {
                    expr: Box::new(ast::Expr::new_variable(item.symbol.clone().unwrap())),
                    alias: Some(item.symbol.clone().unwrap()),
                });
            }
        }
        Ok(new_return_items)
    } else {
        Ok(return_items.items.clone())
    }
}

// 1. extract top level aggregate
// 4. Bind include existing: Variable -> Expr, by in scope, existing will be put into group_by
// 2. Bind GroupBy arguments: varaible -> Expr. by in scope
// 3. Bind Agg arguments: variable -> Expr, by in scope
// 5. Bind post_agg arguments: variable -> Expr by agg out scope
// 5. pre_agg = group_by_args(include existings) + agg_args
// 6. agg: group_by + agg(agg_args)
// 7. post_agg =
//
//
// # pre_agg:
//  - in_scope: input scope of whole clause
//  - out_scope:
//     - symbols should be appear at final output
//     - symbols/exprs acts as args of group_by and agg
//  - action: Proj
//     - symbol -> Expr
// # agg:
//  - in_scope: out scope of pre_agg_proj
//  - out_scope:
//     - symbols should be appear at final output
//     - group_by symbols and agg result symbols
//  - action: Agg
//     - symbol -> Expr
// # post_agg
//  - in_scope: out scope of agg
//  - out_scope:
//     - symbols should be appear at final output
//     - extra_project output symbols
//  - action: Proj
//     - symbol -> Expr
#[allow(clippy::too_many_arguments)]
pub fn bind_return_items(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    scope: Scope,
    distinct: bool,
    for_clause: &ClauseKind,
    return_items: &ast::ReturnItems,
    order_by: Option<&ast::OrderBy>,
    skip: Option<&ast::Expr>,
    limit: Option<&ast::Expr>,
    where_: Option<&ast::Expr>,
) -> Result<Scope, PlanError> {
    let return_items_str = return_items.to_string();
    let return_items = expand_star_expression(&scope, return_items)?;
    if return_items.is_empty() {
        return Err(SemanticError::at_least_one_return_item(&return_items_str).into());
    }

    let contains_agg_or_distinct = distinct || {
        let mut found_agg = false;
        for item in return_items.iter() {
            if contains_agg(bctx, &item.expr)? {
                found_agg = true;
                break;
            }
        }
        found_agg
    };

    // the scope rule is different
    // #1 contains agg or distinct
    //   in this case, order by and limit can only reference the projected results.
    // #2 does not contain agg or distinct
    //   in this case, order by and limit are able to reference input + project symbols.

    if contains_agg_or_distinct {
        bind_group_by_and_agg(
            bctx,
            builder,
            scope,
            distinct,
            for_clause,
            &return_items,
            order_by,
            skip,
            limit,
            where_,
        )
    } else {
        bind_normal_projection(
            bctx,
            builder,
            scope,
            for_clause,
            &return_items,
            order_by,
            skip,
            limit,
            where_,
        )
    }
}

// in this case
// the order by/pagination/where_ clause only works on the output of aggregation
// and the output scope is only the output of post-aggregation projection
#[allow(clippy::too_many_arguments)]
fn bind_group_by_and_agg(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    scope: Scope,
    distinct: bool,
    for_clause: &ClauseKind,
    return_items: &[ReturnItem],
    order_by: Option<&ast::OrderBy>,
    skip: Option<&ast::Expr>,
    limit: Option<&ast::Expr>,
    where_: Option<&ast::Expr>,
) -> Result<Scope, PlanError> {
    let mut group_keys = vec![];
    // contains aggregate expressions
    let mut aggs = vec![];
    let mut post_projection = IndexMap::new();

    for item in return_items.iter() {
        if contains_agg(bctx, &item.expr)? {
            aggs.push(item);
        } else {
            group_keys.push(item);
        }
    }

    let (group_exprs, mut out_scope) = {
        let mut group_exprs = IndexMap::new();
        let mut out_scope = Scope::empty();

        let clause = format!("{} Clause", for_clause);
        let ectx = bctx.derive_expr_context(&scope, &clause);
        for item in group_keys.iter() {
            let symbol = item.alias.clone().unwrap_or(item.expr.to_string());
            let bound_expr = bind_expr(&ectx, &bctx.outer_scopes, &item.expr)?;
            let bound_var_name = if let Some(var_ref) = bound_expr.as_variable_ref() {
                var_ref.name.clone()
            } else {
                bctx.variable_generator.named(&symbol)
            };
            let item = ScopeItem {
                symbol: Some(symbol.to_string()),
                variable: bound_var_name.clone(),
                expr: HashSet::from_iter(vec![*item.expr.clone()]),
                typ: bound_expr.typ(),
                bound_expr: None,
            };
            let ref_bound_var = Expr::from_variable(&Variable {
                name: bound_var_name.clone(),
                typ: bound_expr.typ(),
            });
            out_scope.add_item(item);
            group_exprs.insert(bound_var_name.clone(), bound_expr);
            post_projection.insert(bound_var_name, ref_bound_var);
        }

        (group_exprs, out_scope)
    };

    if aggs.is_empty() {
        // does not contain agg, this is a distinct only projection
        assert!(distinct);

        let order_by_expr = if let Some(order_by) = order_by {
            bind_order_by(bctx, &scope, order_by)?
        } else {
            Default::default()
        };

        let pagination = bind_pagination(bctx, &scope, skip, limit)?;

        let filter = if let Some(where_) = where_ {
            bind_where(bctx, &scope, where_)?
        } else {
            FilterExprs::empty()
        };

        let proj = DistinctProjection {
            group_by: group_exprs,
            order_by: order_by_expr,
            pagination,
            filter,
        };
        builder
            .tail_mut()
            .unwrap()
            .with_projection(QueryProjection::Project(Projection::Distinct(proj)));
        return Ok(out_scope);
    }

    // aggregation projection
    let agg_exprs = {
        let mut agg_exprs = IndexMap::new();
        let clause = format!("{} Clause", for_clause);
        let ectx = bctx.derive_expr_context(&scope, &clause);
        for item in aggs {
            let symbol = item.alias.clone().unwrap_or(item.expr.to_string());
            let bound_expr = bind_expr(&ectx, &bctx.outer_scopes, &item.expr)?;
            let bound_var_name = if let Some(var_ref) = bound_expr.as_variable_ref() {
                var_ref.name.clone()
            } else {
                bctx.variable_generator.named(&symbol)
            };
            let item = ScopeItem {
                symbol: Some(symbol.to_string()),
                variable: bound_var_name.clone(),
                expr: HashSet::from_iter(vec![*item.expr.clone()]),
                typ: bound_expr.typ(),
                bound_expr: None,
            };
            out_scope.add_item(item);
            agg_exprs.insert(bound_var_name, bound_expr);
        }
        agg_exprs
    };

    // split agg_exprs into two parts:
    // 1. pure_agg expressions
    // 2. final projection
    // 3 * (SUM(a.age) + 1)
    let mut aggregate_map = IndexMap::default();
    let mut dedup = HashMap::default();

    for (var_name, expr) in agg_exprs.iter() {
        let mut extractor = AggExtractor::new(
            bctx.variable_generator.as_ref(),
            &mut aggregate_map,
            &mut dedup,
            Some(var_name.clone()),
        );
        let rewritten = extractor.rewrite(expr.clone());
        post_projection.insert(var_name.clone(), rewritten);
    }

    let order_by_expr = if let Some(order_by) = order_by {
        bind_order_by(bctx, &scope, order_by)?
    } else {
        Default::default()
    };

    let pagination = bind_pagination(bctx, &scope, skip, limit)?;

    let filter = if let Some(where_) = where_ {
        bind_where(bctx, &scope, where_)?
    } else {
        FilterExprs::empty()
    };

    let proj = AggregateProjection {
        group_by: group_exprs,
        aggregate: aggregate_map,
        post_projection,
        order_by: order_by_expr,
        pagination,
        filter,
    };

    builder
        .tail_mut()
        .unwrap()
        .with_projection(QueryProjection::Project(Projection::Aggregate(proj)));
    Ok(out_scope)
}

// in this case, the order by/pagination/where_ clause works on the input + output of projection
// but the output will only be return_items
#[allow(clippy::too_many_arguments)]
fn bind_normal_projection(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    scope: Scope,
    for_clause: &ClauseKind,
    return_items: &[ReturnItem],
    order_by: Option<&ast::OrderBy>,
    skip: Option<&ast::Expr>,
    limit: Option<&ast::Expr>,
    where_: Option<&ast::Expr>,
) -> Result<Scope, PlanError> {
    let mut out_scope = Scope::empty();
    let mut final_output = IndexSet::new();
    let clause = format!("{} Clause", for_clause);
    let ectx = bctx.derive_expr_context(&scope, &clause);
    let mut proj_exprs = IndexMap::new();

    for ReturnItem { expr, alias } in return_items.iter() {
        let symbol = alias.clone().unwrap_or(expr.to_string());
        let bound_expr = bind_expr(&ectx, &bctx.outer_scopes, expr)?;
        let bound_var_name = if let Some(var_ref) = bound_expr.as_variable_ref() {
            var_ref.name.clone()
        } else {
            bctx.variable_generator.named(&symbol)
        };
        let item = ScopeItem {
            symbol: Some(symbol.to_string()),
            variable: bound_var_name.clone(),
            expr: HashSet::from_iter(vec![*expr.clone()]),
            typ: bound_expr.typ(),
            bound_expr: None,
        };
        out_scope.add_item(item);
        proj_exprs.insert(bound_var_name.clone(), bound_expr);
        final_output.insert(bound_var_name);
    }

    // add input_scope to out_scope in order to bind orderby/where_/paginaition clause
    for item in scope.symbol_items() {
        let input_alias = item.symbol.as_ref().unwrap();
        if out_scope.resolve_symbol(input_alias).is_some() {
            // let output override the input symbol
            continue;
        }
        out_scope.add_item(item.clone());
    }

    let order_by_expr = if let Some(order_by) = order_by {
        bind_order_by(bctx, &out_scope, order_by)?
    } else {
        Default::default()
    };

    let pagination = bind_pagination(bctx, &out_scope, skip, limit)?;

    let filter = if let Some(where_) = where_ {
        bind_where(bctx, &out_scope, where_)?
    } else {
        FilterExprs::empty()
    };

    // let output scope only contains the final output variables
    out_scope.retain(|item| final_output.contains(&item.variable));

    let proj = RegularProjection {
        items: proj_exprs,
        order_by: order_by_expr,
        pagination,
        filter,
        output: final_output,
    };

    builder
        .tail_mut()
        .unwrap()
        .with_projection(QueryProjection::Project(Projection::Regular(proj)));

    Ok(out_scope)
}

fn contains_agg(bctx: &BindContext, expr: &ast::Expr) -> Result<bool, PlanError> {
    let res = match expr {
        ast::Expr::Literal { .. } => false,
        ast::Expr::Variable { .. } => false,
        ast::Expr::Parameter { .. } => false,
        ast::Expr::MapExpression { values, .. } => {
            for value in values.iter() {
                if contains_agg(bctx, value)? {
                    return Ok(true);
                }
            }
            false
        }
        ast::Expr::PropertyAccess { map, .. } => contains_agg(bctx, map)?,
        ast::Expr::ListExpression { items } => {
            for item in items.iter() {
                if contains_agg(bctx, item)? {
                    return Ok(true);
                }
            }
            false
        }
        ast::Expr::ListSlice { list, .. } => contains_agg(bctx, list)?,
        ast::Expr::ListIndex { list, .. } => contains_agg(bctx, list)?,
        ast::Expr::Unary { oprand, .. } => contains_agg(bctx, oprand)?,
        ast::Expr::Binary { left, right, .. } => contains_agg(bctx, left)? || contains_agg(bctx, right)?,
        ast::Expr::FunctionCall { args, name, .. } => {
            let resolved_func = bctx
                .resolve_function(name)
                .ok_or_else(|| PlanError::from(SemanticError::unknown_function(name, "")))?;
            if resolved_func.is_agg() {
                return Ok(true);
            }
            for arg in args.iter() {
                if contains_agg(bctx, arg)? {
                    return Ok(true);
                }
            }
            false
        }
    };
    Ok(res)
}
/// Rewrites `Expr::AggCall` into `VariableRef`, collecting the lifted aggregates.
/// - Deduplicates identical aggregate calls.
/// - If the whole expression is a single agg and an alias is provided, uses that alias; otherwise generates an
///   anonymous variable.
struct AggExtractor<'a> {
    var_gen: &'a VariableGenerator,
    agg_map: &'a mut IndexMap<VariableName, Expr>,
    dedup: &'a mut HashMap<Expr, VariableName>,
    alias_hint: Option<VariableName>,
    at_root: bool,
}

impl<'a> AggExtractor<'a> {
    fn new(
        var_gen: &'a VariableGenerator,
        agg_map: &'a mut IndexMap<VariableName, Expr>,
        dedup: &'a mut HashMap<Expr, VariableName>,
        alias_hint: Option<VariableName>,
    ) -> Self {
        Self {
            var_gen,
            agg_map,
            dedup,
            alias_hint,
            at_root: true,
        }
    }
}

impl<'a> ExprRewriter for AggExtractor<'a> {
    fn rewrite_agg_call(&mut self, expr: crate::plan::expr::AggCall) -> Expr {
        // children already rewritten inside map_children if called via rewrite_children
        let typ = expr.typ();
        let agg_expr = Expr::AggCall(expr.clone());
        if let Some(var) = self.dedup.get(&agg_expr) {
            return Expr::VariableRef(VariableRef::new_unchecked(var.clone(), typ));
        }

        let is_root = self.at_root;
        self.at_root = false;
        let var_name = if is_root {
            self.alias_hint.clone().unwrap_or_else(|| self.var_gen.unnamed())
        } else {
            self.var_gen.unnamed()
        };

        self.agg_map.insert(var_name.clone(), agg_expr.clone());
        self.dedup.insert(agg_expr, var_name.clone());
        Expr::VariableRef(VariableRef::new_unchecked(var_name, typ))
    }

    fn rewrite_func_call(&mut self, expr: crate::plan::expr::ScalarCall) -> Expr {
        self.at_root = false;
        self.rewrite_children(Expr::ScalarCall(expr))
    }

    fn rewrite_property_access(&mut self, expr: crate::plan::expr::PropertyAccess) -> Expr {
        self.at_root = false;
        self.rewrite_children(Expr::PropertyAccess(expr))
    }

    fn rewrite_create_struct(&mut self, expr: crate::plan::expr::CreateStruct) -> Expr {
        self.at_root = false;
        self.rewrite_children(Expr::CreateStruct(expr))
    }

    fn rewrite_create_list(&mut self, expr: crate::plan::expr::CreateList) -> Expr {
        self.at_root = false;
        self.rewrite_children(Expr::CreateList(expr))
    }

    fn rewrite_has_label(&mut self, expr: crate::plan::expr::HasLabel) -> Expr {
        self.at_root = false;
        self.rewrite_children(Expr::HasLabel(expr))
    }

    fn rewrite_project_path(&mut self, expr: crate::plan::expr::ProjectPath) -> Expr {
        self.at_root = false;
        Expr::ProjectPath(expr)
    }

    fn rewrite_variable_ref(&mut self, expr: VariableRef) -> Expr {
        self.at_root = false;
        Expr::VariableRef(expr)
    }

    fn rewrite_constant(&mut self, expr: Constant) -> Expr {
        self.at_root = false;
        Expr::Constant(expr)
    }
}

pub fn bind_order_by(
    bctx: &BindContext,
    scope: &Scope,
    order_by @ ast::OrderBy { items }: &ast::OrderBy,
) -> Result<Vec<SortItem>, PlanError> {
    let mut bound_items = vec![];
    let order_by_str = order_by.to_string();

    let ectx = bctx.derive_expr_context(scope, &order_by_str);
    ectx.sema_flags.reject_aggregate();
    ectx.sema_flags.reject_outer_reference();

    for item in items.iter() {
        let expr = bind_expr(&ectx, &bctx.outer_scopes, &item.expr)?;
        bound_items.push(SortItem {
            expr: Box::new(expr),
            direction: item.direction,
        });
    }
    Ok(bound_items)
}

pub fn bind_pagination(
    bctx: &BindContext,
    scope: &Scope,
    skip: Option<&ast::Expr>,
    limit: Option<&ast::Expr>,
) -> Result<Pagination, PlanError> {
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

    Ok(pagination)
}
