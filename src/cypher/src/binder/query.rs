use std::sync::Arc;

use elio_common::variable::VariableName;
use elio_parser::ast;
use indexmap::IndexMap;

use crate::binder::BindContext;
use crate::binder::builder::IrSingleQueryBuilder;
use crate::binder::create::bind_create;
use crate::binder::expr::bind_where;
use crate::binder::load::bind_load;
use crate::binder::match_::bind_match;
use crate::binder::project_body::{bind_order_by, bind_pagination, bind_return_items};
use crate::binder::scope::Scope;
use crate::error::{PlanError, SemanticError};
use crate::ir::query::{IrQuery, IrQueryRoot, IrSingleQuery};
use crate::session::PlannerSession;

#[derive(Debug, Clone, derive_more::Display)]
pub enum ClauseKind {
    Create,
    Match,
    With,
    Return,
}

pub fn bind_root_query(sctx: Arc<dyn PlannerSession>, query: &ast::RegularQuery) -> Result<IrQueryRoot, PlanError> {
    let bctx = BindContext::new(sctx);

    let (ir, scope) = bind_query(&bctx, query)?;

    let names: IndexMap<String, VariableName> = scope
        .items
        .iter()
        .map(|x| (x.symbol.clone().unwrap().to_string(), x.variable.clone()))
        .collect();

    let root = IrQueryRoot {
        inner: ir,
        output_names: names,
    };

    Ok(root)
}

fn bind_query(
    bctx: &BindContext,
    _query @ ast::RegularQuery { queries, union_all }: &ast::RegularQuery,
) -> Result<(IrQuery, Scope), PlanError> {
    let mut singles = vec![];
    let mut head_scope: Option<Scope> = None;
    // TODO(pgao): output type should be union of given types
    for q in queries {
        let (single, scope) = bind_single_query(bctx, q)?;
        singles.push(single);
        if let Some(head) = &head_scope {
            // sema check: all column names should be the same
            if head.items.len() != scope.items.len() {
                return Err(SemanticError::invalid_union(&_query.to_string()).into());
            }

            for (lhs, rhs) in head.items.iter().zip(scope.items.iter()) {
                if lhs.symbol != rhs.symbol {
                    return Err(SemanticError::invalid_union(&_query.to_string()).into());
                }
            }
        } else {
            head_scope = Some(scope)
        }
    }

    let ir = IrQuery {
        queries: singles,
        union_all: *union_all,
    };

    Ok((ir, head_scope.unwrap()))
}

fn bind_single_query(bctx: &BindContext, query: &ast::SingleQuery) -> Result<(IrSingleQuery, Scope), PlanError> {
    let ast::SingleQuery { clauses } = query;
    let mut builder = IrSingleQueryBuilder::new();
    let mut in_scope = Scope::empty();
    for clause in clauses.iter() {
        in_scope = match clause {
            ast::Clause::Load(load_clause) => bind_load(bctx, &mut builder, in_scope, load_clause)?,
            ast::Clause::Create(create_clause) => bind_create(bctx, &mut builder, in_scope, create_clause)?,
            ast::Clause::Match(match_clause) => bind_match(bctx, &mut builder, in_scope, match_clause)?,
            ast::Clause::With(with_clause) => bind_with(bctx, &mut builder, in_scope, with_clause)?,
            ast::Clause::Return(return_clause) => bind_return(bctx, &mut builder, in_scope, return_clause)?,
            ast::Clause::Unwind(_unwind_clause) => todo!(),
        };
    }
    // fix: if clause does not ends with return, then return an empty in scope
    // if !matches!(clauses.last().unwrap(), ast::Clause::Return(_)) {
    // in_scope = Scope::empty();
    // }
    Ok((builder.build(), in_scope))
}

/// Execution order of with clause is
///  - Project/Distinct/Aggregate/Unwind
///  - OrderBy
///  - Pagination
///  - Where
///
/// If the projection is an aggregation, the order by and where subclause
/// only sees variables defined in with clause.
///
/// Otherwise, the order by and where subclause sees all variables defined
/// in previous with clause and all variables defined in incomming scope
fn bind_with(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    mut in_scope: Scope,
    _with @ ast::WithClause {
        distinct,
        return_items,
        order_by,
        skip,
        limit,
        where_,
    }: &ast::WithClause,
) -> Result<Scope, PlanError> {
    // remove anonymous variables in in_scope
    in_scope.remove_anonymous();
    // bind projection
    let mut scope = bind_return_items(bctx, builder, in_scope, *distinct, &ClauseKind::With, return_items)?;
    // bind order by
    if let Some(order_by) = order_by {
        bind_order_by(bctx, builder, &scope, order_by)?;
    }
    // bind pagination
    bind_pagination(bctx, builder, &scope, skip.as_deref(), limit.as_deref())?;
    // bind where_
    if let Some(where_) = where_ {
        let filter = bind_where(bctx, &scope, where_)?;
        builder.tail_mut().unwrap().update_projection(|p| p.set_filter(filter));
    }
    scope.remove_anonymous();
    // start a new part
    builder.new_tail(scope.items.iter().map(|item| item.as_variable()).collect());
    Ok(scope)
}

fn bind_return(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    mut in_scope: Scope,
    _return @ ast::ReturnClause {
        distinct,
        return_items,
        order_by,
        skip,
        limit,
    }: &ast::ReturnClause,
) -> Result<Scope, PlanError> {
    // remove anonymous variables in in_scope
    in_scope.remove_anonymous();
    // bind projection
    let mut scope = bind_return_items(bctx, builder, in_scope, *distinct, &ClauseKind::Return, return_items)?;
    // bind order by
    if let Some(order_by) = order_by {
        bind_order_by(bctx, builder, &scope, order_by)?;
    }
    // bind pagination
    bind_pagination(bctx, builder, &scope, skip.as_deref(), limit.as_deref())?;
    scope.remove_anonymous();
    Ok(scope)
}
