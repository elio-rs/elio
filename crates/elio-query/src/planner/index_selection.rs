//! Index selection logic for query optimization
//!
//! This module analyzes filters and determines if a unique index can be used
//! to directly lookup nodes instead of scanning all nodes.

use elio_common::schema::Variable;
use elio_common::variable::VariableName;
use elio_common::{IrToken, ResolvedIrToken};
use indexmap::IndexSet;

use crate::plan::expr::{Expr, FilterExprs, HasLabel};
use crate::plan::session::IndexHint;
use crate::planner::PlannerContext;

/// Information extracted from a filter that can potentially use an index
#[derive(Debug)]
pub struct IndexCandidate {
    /// variable (node) that has the filter
    pub variable: Variable,
    pub label: ResolvedIrToken,
    pub prop_tokens: Vec<ResolvedIrToken>,
    pub prop_values: Vec<Expr>,
    pub solved_predicate: FilterExprs,
    /// matching index hint
    pub index_hint: IndexHint,
}

/// Analyze the filter to find index candidates
pub fn find_index_candidates(
    ctx: &PlannerContext,
    arguments: &IndexSet<Variable>, // some variables in filter may be already sovled by the arguments
    filter: &FilterExprs,
    node_var: &Variable,
) -> Option<IndexCandidate> {
    let label_predicate = extract_label_predicate(node_var, filter);
    let prop_predicate = extract_sargable_predicate(arguments, filter, node_var);
    // We need both a label and at least one property condition
    if label_predicate.is_empty() && prop_predicate.is_empty() {
        return None;
    }

    // we just return the first index match here.
    // TODO(pgao): when we can calculate the statistics, we should return the best index match.
    for label in label_predicate.iter() {
        for prop in prop_predicate.iter() {
            let index_match = find_index_match(ctx, label, prop);
            if let Some(index_match) = index_match {
                return Some(index_match);
            }
        }
    }
    None
}

// find if there's an index match on the given predicate combination
// TODO(pgao): handle unique index with multiple properties.
fn find_index_match(
    ctx: &PlannerContext,
    label: &ResolvedIrToken,
    prop: &SargablePropPredicate,
) -> Option<IndexCandidate> {
    let index_hint = ctx.session().catalog().find_unique_index(label.id, &[prop.prop.id])?;
    let has_label_expr = Expr::new_has_label(
        prop.var.clone(),
        IrToken::Resolved {
            name: label.name.clone(),
            token: label.id,
        },
    );
    let solved_predicate = prop.solved_filter.clone().and(FilterExprs::from(has_label_expr));
    Some(IndexCandidate {
        variable: prop.var.clone(),
        label: label.clone(),
        prop_tokens: vec![prop.prop.clone()],
        prop_values: vec![prop.value.clone()],
        index_hint,
        solved_predicate,
    })
}

fn extract_label_predicate(target_var: &Variable, predicate: &FilterExprs) -> IndexSet<ResolvedIrToken> {
    let mut result = IndexSet::new();
    for expr in predicate.iter() {
        if let Expr::HasLabel(HasLabel { entity, label_or_rel }) = expr
            && let Expr::VariableRef(var_ref) = entity.as_ref()
            && var_ref.name == target_var.name
            && let IrToken::Resolved { name, token } = label_or_rel
        {
            result.insert(ResolvedIrToken {
                name: name.clone(),
                id: *token,
            });
        }
    }
    result
}

// var.prop = value
// sovled_filter is the orignal expression that can be solved by this predicate
#[derive(Debug)]
struct SargablePropPredicate {
    pub var: Variable,
    pub prop: ResolvedIrToken,
    pub value: Expr,
    pub solved_filter: FilterExprs,
}

// extract property predicates that can be used to index lookup
fn extract_sargable_predicate(
    arguments: &IndexSet<Variable>,
    predicate: &FilterExprs,
    target_var: &Variable,
) -> Vec<SargablePropPredicate> {
    let arguments_names = arguments
        .iter()
        .map(|x| x.name.clone())
        .collect::<IndexSet<VariableName>>();

    let mut result = vec![];
    for expr in predicate.iter() {
        if let Expr::FuncCall(func_call) = expr {
            match func_call.func.to_lowercase().as_str() {
                "eq" if func_call.args.len() == 2 => {
                    let (prop_access, value) = match (&func_call.args[0], &func_call.args[1]) {
                        (Expr::PropertyAccess(pa), val) => (pa, val),
                        (val, Expr::PropertyAccess(pa)) => (pa, val),
                        _ => continue,
                    };
                    if let Expr::VariableRef(var_ref) = prop_access.expr.as_ref()
                        && var_ref.name == target_var.name
                        && value.depend_only_on(&arguments_names)
                        && let IrToken::Resolved { name, token } = &prop_access.property
                    {
                        result.push(SargablePropPredicate {
                            var: target_var.clone(),
                            prop: ResolvedIrToken {
                                name: name.clone(),
                                id: *token,
                            },
                            value: value.clone(),
                            solved_filter: FilterExprs::from(expr.clone()),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    result
}
