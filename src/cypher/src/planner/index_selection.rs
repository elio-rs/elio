//! Index selection logic for query optimization
//!
//! This module analyzes filters and determines if a unique index can be used
//! to directly lookup nodes instead of scanning all nodes.

use std::sync::Arc;

use elio_common::variable::VariableName;
use elio_common::{IrToken, LabelId, PropertyKeyId};
use indexmap::IndexMap;

use crate::expr::{Expr, FilterExprs, HasLabel};
use crate::plan_context::PlanContext;
use crate::session::IndexHint;

/// Information extracted from a filter that can potentially use an index
#[derive(Debug)]
pub struct IndexCandidate {
    /// The variable (node) that has the filter
    pub variable: VariableName,
    pub label_name: String,
    pub label_id: LabelId,
    pub property_names: Vec<String>,
    pub property_key_ids: Vec<PropertyKeyId>,
    /// constant property values
    pub property_values: Vec<Expr>,
    /// The matching index hint
    pub index_hint: IndexHint,
}

/// Analyze the filter to find index candidates
pub fn find_index_candidates(
    ctx: &Arc<PlanContext>,
    filter: &FilterExprs,
    node_var: &VariableName,
) -> Option<IndexCandidate> {
    // Extract label and property conditions for this variable
    let mut label_id: Option<LabelId> = None;
    let mut label_name: Option<String> = None;
    let mut property_conditions: IndexMap<PropertyKeyId, (String, Expr)> = IndexMap::new();

    for expr in filter.iter() {
        extract_index_info(
            ctx,
            expr,
            node_var,
            &mut label_id,
            &mut label_name,
            &mut property_conditions,
        );
    }

    // We need both a label and at least one property condition
    let label_id = label_id?;
    let label_name = label_name?;

    if property_conditions.is_empty() {
        return None;
    }

    // Check if there's an index for this combination
    let property_key_ids: Vec<PropertyKeyId> = property_conditions.keys().copied().collect();
    let index_hint = ctx.session().find_unique_index(label_id, &property_key_ids)?;

    // Build the result with properties in index order
    let mut property_names = Vec::new();
    let mut ordered_property_key_ids = Vec::new();
    let mut property_values = Vec::new();

    for prop_id in &index_hint.property_key_ids {
        if let Some((name, value)) = property_conditions.get(prop_id) {
            property_names.push(name.clone());
            ordered_property_key_ids.push(*prop_id);
            property_values.push(value.clone());
        } else {
            // Index property not found in filter - can't use this index
            return None;
        }
    }

    Some(IndexCandidate {
        variable: node_var.clone(),
        label_name,
        label_id,
        property_names,
        property_key_ids: ordered_property_key_ids,
        property_values,
        index_hint,
    })
}

/// Extract label and property equality conditions from a filter expression
fn extract_index_info(
    _ctx: &Arc<PlanContext>,
    expr: &Expr,
    target_var: &VariableName,
    label_id: &mut Option<LabelId>,
    label_name: &mut Option<String>,
    property_conditions: &mut IndexMap<PropertyKeyId, (String, Expr)>,
) {
    match expr {
        // HasLabel check: n:Person
        Expr::HasLabel(HasLabel { entity, label_or_rel }) => {
            if let Expr::VariableRef(var_ref) = entity.as_ref()
                && &var_ref.name == target_var
                && let IrToken::Resolved { name, token } = label_or_rel
            {
                *label_id = Some(*token);
                *label_name = Some(name.to_string());
            }
        }

        // Function call - look for equality: eq(n.prop, 'value')
        Expr::FuncCall(func_call) => {
            // Check for AND - recurse into both sides
            if func_call.func == "and" {
                for arg in &func_call.args {
                    extract_index_info(_ctx, arg, target_var, label_id, label_name, property_conditions);
                }
                return;
            }

            // Check for equality: eq(n.prop, value)
            if func_call.func == "eq" && func_call.args.len() == 2 {
                let (prop_access, value) = match (&func_call.args[0], &func_call.args[1]) {
                    (Expr::PropertyAccess(pa), val) => (pa, val),
                    (val, Expr::PropertyAccess(pa)) => (pa, val),
                    _ => return,
                };

                // Check if property access is on our target variable
                if let Expr::VariableRef(var_ref) = prop_access.expr.as_ref()
                    && &var_ref.name == target_var
                {
                    // Check if value is a constant (we can only use index for constant values)
                    if matches!(value, Expr::Constant(_))
                        && let IrToken::Resolved { name, token } = &prop_access.property
                    {
                        property_conditions.insert(*token, (name.to_string(), value.clone()));
                    }
                }
            }
        }

        _ => {}
    }
}

/// Remove conditions that are covered by the index from the filter
pub fn remove_index_conditions(filter: &FilterExprs, candidate: &IndexCandidate) -> FilterExprs {
    let remaining: Vec<Expr> = filter
        .iter()
        .filter(|expr| !is_covered_by_index(expr, candidate))
        .cloned()
        .collect();
    FilterExprs::from_iter(remaining)
}

/// Check if an expression is covered by the index lookup
fn is_covered_by_index(expr: &Expr, candidate: &IndexCandidate) -> bool {
    match expr {
        Expr::HasLabel(HasLabel { entity, label_or_rel }) => {
            if let Expr::VariableRef(var_ref) = entity.as_ref()
                && var_ref.name == candidate.variable
                && let IrToken::Resolved { token, .. } = label_or_rel
            {
                return *token == candidate.label_id;
            }
            false
        }

        Expr::FuncCall(func_call) => {
            if func_call.func == "eq" && func_call.args.len() == 2 {
                let prop_access = match (&func_call.args[0], &func_call.args[1]) {
                    (Expr::PropertyAccess(pa), Expr::Constant(_)) => Some(pa),
                    (Expr::Constant(_), Expr::PropertyAccess(pa)) => Some(pa),
                    _ => None,
                };

                if let Some(pa) = prop_access
                    && let Expr::VariableRef(var_ref) = pa.expr.as_ref()
                    && var_ref.name == candidate.variable
                    && let IrToken::Resolved { token, .. } = &pa.property
                {
                    return candidate.property_key_ids.contains(token);
                }
            }
            false
        }

        _ => false,
    }
}
