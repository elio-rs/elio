use core::f64;
use std::sync::Arc;

use elio_common::data_type::{F64, LogicalType};
use elio_common::{IrToken, TokenKind};
use elio_parser::ast;
use itertools::Itertools;
use ordered_float::Float;
use paste::paste;

use crate::binder::BindContext;
use crate::binder::scope::Scope;
use crate::catalog::FunctionCatalog;
use crate::function::scalar::sig::FuncImpl;
use crate::not_supported;
use crate::plan::error::{PlanError, SemanticError};
use crate::plan::expr::value::Constant;
use crate::plan::expr::{AggCall, CreateList, Expr, ExprNode, FilterExprs, FuncCall, PropertyAccess, VariableRef};

#[derive(Clone)]
pub struct ExprContext<'a> {
    pub bctx: &'a BindContext<'a>,
    pub scope: &'a Scope,
    pub name: &'a str,
    pub sema_flags: ExprSemanticFlag,
}

#[derive(Default, Copy, Clone, Debug)]
pub struct ExprSemanticFlag(u64);

const EXPR_REJECT_OUTER_REFERENCE: u64 = 0x1;
const EXPR_REJECT_AGGREGATE: u64 = 0x2;

macro_rules! impl_sema_flag {
    ($flag:ident, $mask:ident) => {
        paste! {
            pub fn [<set_ $flag>](&mut self, value: bool) {
                if value {
                    self.0 |= $mask;
                } else {
                    self.0 &= !$mask;
                }
            }
            pub fn [<$flag>](&self) -> bool {
                self.0 & $mask != 0
            }
        }
    };
}

impl ExprSemanticFlag {
    impl_sema_flag!(reject_outer_reference, EXPR_REJECT_OUTER_REFERENCE);

    impl_sema_flag!(reject_aggregate, EXPR_REJECT_AGGREGATE);
}

pub fn bind_expr(ectx: &ExprContext, outer_scope: &[Scope], expr: &ast::Expr) -> Result<Expr, PlanError> {
    // if expr already bound, just return the symbol
    if let Some(item) = ectx.scope.resolve_expr(expr) {
        return Ok(item.as_expr());
    }
    match expr {
        ast::Expr::Literal { lit } => bind_constant(ectx, lit).map(Into::into),
        ast::Expr::Variable { name } => bind_variable(ectx, name, outer_scope).map(Into::into),
        ast::Expr::Parameter { .. } => not_supported!("parameter binding"),
        ast::Expr::MapExpression { .. } => {
            unreachable!("map expr should not be bind directly, must be in some context")
        }
        ast::Expr::PropertyAccess { map, key } => {
            let expr = bind_expr(ectx, outer_scope, map)?;
            // resolve property keys
            let token: IrToken = ectx.bctx.resolve_token(key, TokenKind::PropertyKey);
            // TODO(pgao): maybe we can resolve the property types here
            let pa = PropertyAccess::new_unchecked(expr.boxed(), &token, &LogicalType::ANY);
            Ok(pa.into())
        }
        ast::Expr::Unary { op, oprand } => bind_unary(ectx, outer_scope, op, oprand),
        ast::Expr::Binary { left, op, right } => bind_binary(ectx, outer_scope, left, op, right),
        ast::Expr::FunctionCall { name, distinct, args } => bind_func_call(ectx, outer_scope, name, *distinct, args),
        ast::Expr::ListExpression { items } => bind_list_expression(ectx, outer_scope, items),
        ast::Expr::ListSlice { list, start, end } => bind_list_slice(ectx, outer_scope, list, start, end),
        ast::Expr::ListIndex { list, index } => bind_list_index(ectx, outer_scope, list, index),
    }
}

fn bind_constant(_ectx: &ExprContext, lit: &ast::Literal) -> Result<Constant, PlanError> {
    match lit {
        ast::Literal::Boolean(b) => Ok(Constant::boolean(*b)),
        ast::Literal::Integer(i) => {
            if let Ok(i) = i.parse::<i64>() {
                Ok(Constant::integer(i))
            } else {
                Err(SemanticError::invalid_literal(&LogicalType::INTEGER, &lit.to_string()).into())
            }
        }
        ast::Literal::Float(f) => {
            if let Ok(f) = f.parse::<f64>() {
                Ok(Constant::float(f.into()))
            } else {
                Err(SemanticError::invalid_literal(&LogicalType::FLOAT, &lit.to_string()).into())
            }
        }
        ast::Literal::String(s) => Ok(Constant::string(s.clone())),
        // null should be resolved type in an expr context
        ast::Literal::Null => Ok(Constant::untyped_null()),
        ast::Literal::Inf => Ok(Constant::float(F64::infinity())),
    }
}

fn bind_variable(ectx: &ExprContext, name: &str, outer_scope: &[Scope]) -> Result<VariableRef, PlanError> {
    if let Some(item) = ectx.scope.resolve_symbol(name) {
        return Ok(VariableRef::from_variable(&item.as_variable()));
    }
    if ectx.sema_flags.reject_outer_reference() {
        return Err(SemanticError::variable_not_defined(name, ectx.name).into());
    }

    // bind variable in outer scope
    for scope in outer_scope.iter() {
        let item = scope.resolve_symbol(name);
        if let Some(item) = item {
            return Ok(VariableRef::from_variable(&item.as_variable()));
        }
    }
    Err(SemanticError::variable_not_defined(name, ectx.name).into())
}

fn bind_unary(
    ectx: &ExprContext,
    outer_scope: &[Scope],
    op: &ast::UnaryOperator,
    oprand: &ast::Expr,
) -> Result<Expr, PlanError> {
    let oprand = bind_expr(ectx, outer_scope, oprand)?;
    let args = vec![oprand];

    // SAFETY: builtin operator are always ok
    let func_name = op.as_func_name();
    let (func_impl, _is_agg, typ, coerced_types) = resolve_func(ectx, func_name, &args)?;

    // Coerce untyped null arguments to typed nulls
    let args = coerce_null_args(args, &coerced_types);

    let func_call = FuncCall::new_unchecked(func_name.to_string(), func_impl.func_id, args, typ);
    Ok(func_call.into())
}

fn bind_binary(
    ectx: &ExprContext,
    outer_scope: &[Scope],
    left: &ast::Expr,
    op: &ast::BinaryOperator,
    right: &ast::Expr,
) -> Result<Expr, PlanError> {
    let left = bind_expr(ectx, outer_scope, left)?;
    let right = bind_expr(ectx, outer_scope, right)?;
    let args = vec![left, right];

    // SAFETY: builtin operator are always ok
    let func_name = op.as_func_name();
    let (func_impl, _is_agg, typ, coerced_types) = resolve_func(ectx, func_name, &args)?;

    // Coerce untyped null arguments to typed nulls
    let args = coerce_null_args(args, &coerced_types);

    let func_call = FuncCall::new_unchecked(func_name.to_string(), func_impl.func_id, args, typ);
    Ok(func_call.into())
}

fn bind_list_index(
    ectx: &ExprContext,
    outer_scope: &[Scope],
    list: &ast::Expr,
    index: &ast::Expr,
) -> Result<Expr, PlanError> {
    let list_expr = bind_expr(ectx, outer_scope, list)?;
    let index_expr = bind_expr(ectx, outer_scope, index)?;
    let args = vec![list_expr, index_expr];

    let (func_impl, _is_agg, typ, coerced_types) = resolve_func(ectx, "list_index", &args)?;
    let args = coerce_null_args(args, &coerced_types);

    let func_call = FuncCall::new_unchecked("list_index".to_string(), func_impl.func_id, args, typ);
    Ok(func_call.into())
}

fn bind_list_slice(
    ectx: &ExprContext,
    outer_scope: &[Scope],
    list: &ast::Expr,
    start: &Option<Box<ast::Expr>>,
    end: &Option<Box<ast::Expr>>,
) -> Result<Expr, PlanError> {
    let list_expr = bind_expr(ectx, outer_scope, list)?;

    // For start/end, use 0 and MAX_INT as defaults if not specified
    let start_expr = if let Some(s) = start {
        bind_expr(ectx, outer_scope, s)?
    } else {
        Expr::Constant(Constant::integer(0))
    };

    let end_expr = if let Some(e) = end {
        bind_expr(ectx, outer_scope, e)?
    } else {
        // Use a large number as "end of list" indicator
        // The actual implementation will clamp to list length
        Expr::Constant(Constant::integer(i64::MAX))
    };

    let args = vec![list_expr, start_expr, end_expr];

    let (func_impl, _is_agg, typ, coerced_types) = resolve_func(ectx, "list_slice", &args)?;
    let args = coerce_null_args(args, &coerced_types);

    let func_call = FuncCall::new_unchecked("list_slice".to_string(), func_impl.func_id, args, typ);
    Ok(func_call.into())
}

fn bind_list_expression(ectx: &ExprContext, outer_scope: &[Scope], items: &[ast::Expr]) -> Result<Expr, PlanError> {
    // Bind all element expressions
    let elements = items
        .iter()
        .map(|x| bind_expr(ectx, outer_scope, x))
        .collect::<Result<Vec<_>, _>>()?;

    // Infer the element type
    let elem_type = if elements.is_empty() {
        // Empty list defaults to Any type
        LogicalType::ANY
    } else {
        // Find common type among all elements
        // For now, use the first non-Any element type, or Any if all are Any
        let mut common_type = LogicalType::ANY;
        for elem in &elements {
            let elem_typ = elem.typ();
            if elem_typ != LogicalType::ANY {
                if common_type == LogicalType::ANY {
                    common_type = elem_typ;
                } else if common_type != elem_typ {
                    // Type mismatch - for now, fallback to Any
                    // TODO: implement proper type coercion
                    common_type = LogicalType::ANY;
                    break;
                }
            }
        }
        common_type
    };

    Ok(CreateList::new(elements, elem_type).into())
}

fn bind_func_call(
    ectx: &ExprContext,
    outer_scope: &[Scope],
    name: &str,
    distinct: bool,
    args: &[ast::Expr],
) -> Result<Expr, PlanError> {
    let FunctionCatalog { name, func } = ectx
        .bctx
        .session()
        .catalog()
        .resolve_function(name)
        .ok_or(PlanError::from(SemanticError::unknown_function(name, ectx.name)))?;

    if func.is_agg && ectx.sema_flags.reject_aggregate() {
        return Err(SemanticError::agg_not_allowed(name, ectx.name).into());
    }

    let inner_ectx = if func.is_agg {
        let mut inner_ectx = ectx.clone();
        // aggregation function cannot be nested
        inner_ectx.sema_flags.set_reject_aggregate(true);
        inner_ectx
    } else {
        ectx.clone()
    };

    let args = args
        .iter()
        .map(|x| bind_expr(&inner_ectx, outer_scope, x))
        .collect::<Result<Vec<_>, _>>()?;

    let (func_impl, is_agg, typ, coerced_types) = resolve_func(ectx, name, &args)?;

    // Coerce untyped null arguments to typed nulls
    let args = coerce_null_args(args, &coerced_types);

    if is_agg {
        let agg = AggCall::new_unchecked(name.to_string(), func_impl.func_id, args, distinct, typ);
        Ok(agg.into())
    } else {
        if distinct {
            return Err(SemanticError::distinct_not_allowed(name).into());
        }
        let func_call = FuncCall::new_unchecked(name.to_string(), func_impl.func_id, args, typ);
        Ok(func_call.into())
    }
}

fn resolve_func(
    ectx: &ExprContext,
    name: &str,
    args: &[Expr],
) -> Result<(FuncImpl, bool, LogicalType, Vec<LogicalType>), PlanError> {
    let FunctionCatalog { name, func } = ectx
        .bctx
        .session()
        .catalog()
        .resolve_function(name)
        .ok_or(PlanError::from(SemanticError::unknown_function(name, ectx.name)))?;

    let is_agg = func.is_agg;

    // Prepare argument types and null flags
    let args_types: Vec<LogicalType> = args.iter().map(|x| x.typ()).collect_vec();
    let is_untyped_null: Vec<bool> = args
        .iter()
        .map(|arg| {
            if let Expr::Constant(const_val) = arg {
                const_val.is_untyped_null()
            } else {
                false
            }
        })
        .collect();

    let has_untyped_null = is_untyped_null.iter().any(|&x| x);

    // Select function implementation
    for func_impl in func.impls.iter() {
        if has_untyped_null {
            // Try matching with null coercion
            if let Some((ret, coerced_types)) = func_impl.matches_with_null_coercion(&args_types, &is_untyped_null) {
                return Ok((func_impl.clone(), is_agg, ret, coerced_types));
            }
        } else {
            // No untyped nulls, use regular matching
            if let Some(ret) = func_impl.matches(&args_types) {
                return Ok((func_impl.clone(), is_agg, ret, args_types.clone()));
            }
        }
    }

    // found no function matches the signature
    Err(SemanticError::invalid_function_arg_types(name, &args_types, ectx.name).into())
}

/// Coerce untyped null literals to typed nulls based on inferred types
fn coerce_null_args(args: Vec<Expr>, coerced_types: &[LogicalType]) -> Vec<Expr> {
    args.into_iter()
        .enumerate()
        .map(|(i, arg)| {
            // Check if argument is an untyped null constant
            if let Expr::Constant(const_val) = &arg
                && const_val.is_untyped_null()
            {
                let target_type = &coerced_types[i];
                // Create typed null if we found a concrete type (not Any)
                if target_type != &LogicalType::ANY {
                    return Expr::Constant(Constant::typed_null(target_type.clone()));
                }
            }
            arg
        })
        .collect()
}

pub fn bind_where(bctx: &BindContext, scope: &Scope, where_: &ast::Expr) -> Result<FilterExprs, PlanError> {
    let ctx = where_.to_string();
    let ectx = bctx.derive_expr_context(scope, &ctx);
    let expr = bind_expr(&ectx, &bctx.outer_scopes, where_)?;
    if expr.typ() != LogicalType::BOOL {
        return Err(SemanticError::invalid_filter_expr_type(&expr.typ(), ectx.name).into());
    }
    Ok(expr.into())
}

/// Bind map expression to property map.
pub fn bind_map_expr_to_property_map(
    ectx: &ExprContext,
    outer_scope: &[Scope],
    keys: &[String],
    values: &[ast::Expr],
) -> Result<Vec<(Arc<str>, Expr)>, PlanError> {
    assert_eq!(keys.len(), values.len());

    let exprs = values
        .iter()
        .map(|x| bind_expr(ectx, outer_scope, x))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(keys.iter().map(|x| x.as_str().into()).zip(exprs).collect())
}
