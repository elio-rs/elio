use std::collections::HashSet;
use std::ops::Range;

use elio_common::data_type::DataType;
use elio_common::schema::Variable;
use elio_common::variable::VariableName;
use elio_common::{IrToken, TokenKind};
use elio_parser::ast::{self, NodePattern, RelationshipPattern};
use indexmap::IndexSet;

use crate::binder::BindContext;
use crate::binder::expr::{ExprContext, bind_expr};
use crate::binder::label_expr::bind_label_expr;
use crate::binder::query::ClauseKind;
use crate::binder::scope::{Scope, ScopeItem};
use crate::plan::error::PlanError;
use crate::plan::expr::property_access::PropertyAccess;
use crate::plan::expr::{Expr, ExprNode, FilterExprs, PathStep, ProjectPath};
use crate::plan::ir::node_connection::{
    NodeBinding, NodeConnection, PatternLength, QuantifiedPathPattern, RelPattern, Repetition, VariableGrouping,
};
use crate::plan::ir::path_pattern::{ChainPattern, PathPattern, SingleNode};

#[derive(Debug, Clone)]
pub struct PatternContext<'a> {
    pub bctx: &'a BindContext,
    pub clause: ClauseKind,
    // context name, used for error messages
    pub name: &'a str,
    // true on allow update in this contex
    pub allow_update: bool,
    // TODO(pgao): refactor this, we can have an reject flags according to different pattern context
    // true on quantified path pattern not allowed
    pub reject_qpp: bool,
    // true on reject named path pattern
    pub reject_named_path: bool,
    // true on reject selective path pattenr
    pub reject_selective: bool,
}

impl<'a> PatternContext<'a> {
    pub fn derive_expr_context(&self, scope: &'a Scope, name: &'a str) -> ExprContext<'a> {
        ExprContext {
            bctx: self.bctx,
            scope,
            name,
            sema_flags: Default::default(),
        }
    }
}

pub(crate) fn bind_pattern(
    pctx: &PatternContext,
    // imported variables should be put in scope
    mut scope: Scope,
    pattern: &[ast::PatternPart],
) -> Result<(Vec<PathPatternWithExtra>, Scope), PlanError> {
    let mut paths = vec![];
    for part in pattern {
        let (path, extra, new_scope) = bind_pattern_part(pctx, scope, part)?;
        scope = new_scope;
        paths.push(PathPatternWithExtra { pattern: path, extra });
    }
    Ok((paths, scope))
}

pub struct PathPatternWithExtra {
    pub pattern: PathPattern,
    pub extra: PathPatternExtra,
}

// - SimplePattern: bind and pull the filter into WHERE clause Return (Vec<NodeVar>, Vec<RelPattern>, Filter)
// - QuantifiedPathPattern: generate selective path pattern SemanticCheck: RejectNestedSelector Return
//   SelectivePathPattern
// - Path variable, generate path expression
//
// After binding, return
//   - PathPattern
//   - Filter(post filter)
//   - path variable
// return (PathPattern, PathName)
pub(crate) fn bind_pattern_part(
    pctx: &PatternContext,
    mut scope: Scope,
    pattern: &ast::PatternPart,
) -> Result<(PathPattern, PathPatternExtra, Scope), PlanError> {
    let ast::PatternPart {
        variable,
        selector,
        factors,
    } = pattern;

    // not supported selective path pattern
    if selector.is_selective() {
        return Err(PlanError::not_supported("Selective path pattern not supported"));
    }
    if variable.is_some() && pctx.reject_named_path {
        return Err(PlanError::not_supported("Path variable not supported"));
    }

    // note: quantified path pattern must be conjuncted with simple path patterns
    // so the factors must be like:
    // simple - (qpp - simple)*

    let (simple, quantified) = partition_factors(factors)?;

    // bind all simple
    let mut bound_simple = vec![];
    let mut bound_quantified = vec![];

    for path_pattern in simple.iter() {
        let (nodes, rels, extra, new_scope) = bind_simple_pattern(pctx, scope, path_pattern)?;
        scope = new_scope;
        bound_simple.push((nodes, rels, extra));
    }

    // bind all qpp with left and right
    for (i, qpp) in quantified.iter().enumerate() {
        let left = bound_simple[i].0.last().unwrap();
        let right = bound_simple[i + 1].0.first().unwrap();
        let (bound_qpp, extra, new_scope) = bind_quantified_path_pattern(pctx, scope, left, right, qpp)?;
        scope = new_scope;
        bound_quantified.push((bound_qpp, extra));
    }

    let (path, extra, steps) = if bound_quantified.is_empty() {
        // simple path pattern only, there should be only one simple pattern
        assert!(bound_simple.len() == 1);
        let (nodes, rels, extra) = bound_simple.into_iter().next().unwrap();

        let steps = {
            let mut steps = vec![];
            steps.push(PathStep::NodeStep(nodes.first().unwrap().clone()));

            for r in rels.iter() {
                steps.push(PathStep::SingleRelStep {
                    rel: r.variable.clone(),
                    direction: r.dir,
                    other: r.endpoints.1.clone(),
                });
            }
            steps
        };

        if rels.is_empty() {
            // single node path pattern
            let path = PathPattern::SingleNode(SingleNode {
                variable: nodes.first().unwrap().clone(),
            });
            (path, extra, steps)
        } else {
            // connections
            let connections = rels.into_iter().map(NodeConnection::RelPattern).collect();
            let path = PathPattern::Chain(ChainPattern { connections });
            (path, extra, steps)
        }
    } else {
        // quantified and simple path patterns mixed
        // simples
        let mut nodes = vec![];
        let mut conns = vec![];
        let mut extra = NodeConnectionExtra::empty();
        for (ns, rs, simple_extra) in bound_simple {
            nodes.extend(ns.clone());
            conns.extend(rs.iter().cloned().map(NodeConnection::RelPattern));
            extra = extra.merge(simple_extra);
        }

        // quantified
        for (qpp, qpp_extra) in bound_quantified {
            conns.push(NodeConnection::QuantifiedPathPattern(qpp));
            extra = extra.merge(qpp_extra);
        }

        (
            PathPattern::Chain(ChainPattern { connections: conns }),
            extra,
            // TODO(pgao): construct steps
            vec![],
        )
    };

    // named path
    let path_var = if let Some(name) = variable {
        let (var, is_outer) = bind_variable(pctx, &mut scope, Some(name), &DataType::VirtualPath)?;
        if is_outer {
            return Err(PlanError::semantic_err(
                "Named path pattern cannot reference outer variable".to_string(),
            ));
        }
        // bind path expression, add project path to scope
        let project_path = ProjectPath { steps }.into();
        let item = ScopeItem {
            symbol: Some(name.to_string()),
            variable: var.name.clone(),
            expr: HashSet::from_iter(vec![ast::Expr::new_variable(name.to_string())]),
            typ: var.typ.clone(),
            bound_expr: Some(project_path), // path expression
        };
        scope.add_item(item);
        Some(var)
    } else {
        None
    };

    let path_extra = PathPatternExtra {
        name: path_var,
        outer: extra.outer,
        post_filter: extra.post_filter,
    };

    Ok((path, path_extra, scope))
}

fn partition_factors(
    factors: &[ast::PathFactor],
) -> Result<(Vec<&ast::SimplePathPattern>, Vec<&ast::QuantifiedPathPattern>), PlanError> {
    let mut simple = vec![];
    let mut quantified = vec![];
    for (i, factor) in factors.iter().enumerate() {
        if i % 2 == 0 {
            if let ast::PathFactor::Simple(s) = factor {
                simple.push(s);
            } else {
                return Err(PlanError::semantic_err(
                    "Simple path pattern must be at even position in pattern part".to_string(),
                ));
            }
        } else if let ast::PathFactor::Quantified(q) = factor {
            quantified.push(q);
        } else {
            return Err(PlanError::semantic_err(
                "Quantified path pattern must be at odd position in pattern part".to_string(),
            ));
        }
    }
    Ok((simple, quantified))
}

#[derive(Clone)]
struct NodeConnectionExtra {
    pub outer: IndexSet<VariableName>,
    pub post_filter: FilterExprs,
}

impl NodeConnectionExtra {
    pub fn empty() -> Self {
        Self {
            outer: Default::default(),
            post_filter: FilterExprs::empty(),
        }
    }

    pub fn merge(self, other: Self) -> Self {
        let mut outer = self.outer;
        for v in other.outer {
            outer.insert(v);
        }
        let post_filter = self.post_filter.and(other.post_filter);
        Self { outer, post_filter }
    }
}

pub struct PathPatternExtra {
    pub name: Option<Variable>,        // named path or not
    pub outer: IndexSet<VariableName>, // outer references used by this pattern
    pub post_filter: FilterExprs,      // post filter after match this pattern
}

fn bind_simple_pattern(
    pctx: &PatternContext,
    // current scope only record introduced scope items
    mut scope: Scope,
    _simple @ ast::SimplePathPattern { nodes, relationships }: &ast::SimplePathPattern,
) -> Result<(Vec<VariableName>, Vec<RelPattern>, NodeConnectionExtra, Scope), PlanError> {
    let mut ir_nodes = vec![];
    let mut ir_rels = vec![];
    let mut filter = FilterExprs::empty();
    let mut outer = IndexSet::default();

    for NodePattern {
        variable,
        label_expr,
        properties,
        predicate,
    } in nodes
    {
        // bind variable
        let (var, is_outer) = bind_variable(pctx, &mut scope, variable.as_deref(), &DataType::VirtualNode)?;
        if is_outer {
            outer.insert(var.name.clone());
        }

        // pre-filter
        let mut node_filter = FilterExprs::empty();

        // bind label expr
        if let Some(label_expr) = label_expr {
            let label_expr = bind_label_expr(pctx, Expr::from_variable(&var).boxed(), label_expr)?;
            node_filter.push(*label_expr);
        }

        // bind properties
        if let Some(props) = properties {
            let props = bind_properties(pctx, &scope, &var, props)?;
            node_filter = node_filter.and(props);
        }
        // TODO(pgao): bind predicate, in parser we do not support predicate right now
        if predicate.is_some() {
            return Err(PlanError::not_supported("predicate in pattern not supported"));
        }

        filter = filter.and(node_filter);
        ir_nodes.push(var.name);
    }

    for (
        i,
        RelationshipPattern {
            variable,
            label_expr,
            properties,
            predicate,
            length,
            direction,
        },
    ) in relationships.iter().enumerate()
    {
        let (var, is_outer) = bind_variable(pctx, &mut scope, variable.as_deref(), &DataType::Rel)?;

        if is_outer {
            outer.insert(var.name.clone());
        }
        // for relationship type, label expr should either be single reltype or reltype conjuncted with OR or NONE
        // bind label expr
        let mut reltypes: Vec<IrToken> = vec![];
        if let Some(label_expr) = label_expr {
            let rel_types = label_expr.extract_relationship_types().ok_or_else(|| {
                PlanError::semantic_err("relationship type must be a single reltype or reltype conjuncted with OR")
            })?;
            for rtype in rel_types {
                let token = pctx.bctx.resolve_token(&rtype, TokenKind::RelationshipType);
                reltypes.push(token);
            }
        }

        let mut rel_filter = FilterExprs::empty();
        // bind properties
        if let Some(props) = properties {
            let props = bind_properties(pctx, &scope, &var, props)?;
            rel_filter = rel_filter.and(props);
        }
        // TODO(pgao): bind predicate, in parser we do not support predicate right now
        if predicate.is_some() {
            return Err(PlanError::not_supported("predicate in pattern not supported"));
        }

        // bind length
        let length = match length {
            None => PatternLength::Simple,
            // if length is None, then it is *1..
            Some(None) => PatternLength::Var { min: 1, max: None },
            Some(Some(Range { start, end })) => PatternLength::Var {
                min: *start,
                max: if *end == usize::MAX { None } else { Some(*end) },
            },
        };

        filter = filter.and(rel_filter);

        let rel = RelPattern {
            variable: var.name.clone(),
            endpoints: (ir_nodes[i].clone(), ir_nodes[i + 1].clone()),
            dir: *direction,
            types: reltypes,
            length,
        };
        ir_rels.push(rel);
    }

    // no repeated relationship check
    {
        let rel_set: HashSet<_> = ir_rels.iter().map(|x| x.variable.clone()).collect();
        if rel_set.len() != ir_rels.len() {
            return Err(PlanError::semantic_err(
                " repeated relationships not allowed in the path pattern".to_string(),
            ));
        }
    }

    Ok((
        ir_nodes,
        ir_rels,
        NodeConnectionExtra {
            outer,
            post_filter: filter,
        },
        scope,
    ))
}

fn bind_quantified_path_pattern(
    pctx: &PatternContext,
    mut scope: Scope,
    left: &VariableName,
    right: &VariableName,
    _qpp @ ast::QuantifiedPathPattern {
        non_selective_part,
        quantifier,
        filter,
    }: &ast::QuantifiedPathPattern,
) -> Result<(QuantifiedPathPattern, NodeConnectionExtra, Scope), PlanError> {
    let mut inner_pctx = pctx.clone();
    let inner_scope = scope.clone();
    // quantified path pattern not allowed to be nested
    inner_pctx.reject_qpp = true;
    // quantified path pattern not allowed to have named path pattern
    inner_pctx.reject_named_path = true;
    inner_pctx.reject_selective = true;

    let (path, path_extra, inner_scope) = bind_pattern_part(&inner_pctx, inner_scope, non_selective_part)?;

    let rels = path
        .as_node_connections()
        .ok_or(PlanError::semantic_err("Node connections expected in QPP."))?
        .as_rels()
        .ok_or(PlanError::semantic_err("Only simple relationships allowed in QPP."))?;

    let PathPatternExtra {
        name,
        outer,
        post_filter: mut inner_filter, // this works as pre-filter in qpp
    } = path_extra;
    assert!(name.is_none(), "Named path not allowed in quantified path pattern.");
    assert!(
        outer.is_empty(),
        "Outer reference not allowed in quantified path pattern."
    );

    let left_binding = NodeBinding {
        inner: rels.first().unwrap().endpoints.0.clone(),
        outer: left.clone(),
    };
    let right_binding = NodeBinding {
        inner: rels.last().unwrap().endpoints.1.clone(),
        outer: right.clone(),
    };
    let repetition = match quantifier {
        ast::PatternQuantifier::Plus => Repetition { min: 1, max: None },
        ast::PatternQuantifier::Star => Repetition { min: 0, max: None },
        ast::PatternQuantifier::Fixed(n) => Repetition {
            min: *n as i64,
            max: Some(*n as i64),
        },
        ast::PatternQuantifier::Interval { lower, upper } => Repetition {
            min: lower.unwrap_or_default() as i64,
            max: upper.map(|x| x as i64),
        },
    };
    let node_grouping = {
        let nodes: IndexSet<VariableName> = rels
            .iter()
            .flat_map(|r| vec![r.endpoints.0.clone(), r.endpoints.1.clone()])
            .collect();
        nodes
            .into_iter()
            .map(|singleton| VariableGrouping {
                singleton: singleton.clone(),
                group: pctx.bctx.variable_generator.named(&singleton),
            })
            .collect()
    };
    let rel_grouping = {
        let rels: IndexSet<VariableName> = rels.iter().map(|r| r.variable.clone()).collect();
        rels.into_iter()
            .map(|singleton| VariableGrouping {
                singleton: singleton.clone(),
                group: pctx.bctx.variable_generator.named(&singleton),
            })
            .collect()
    };

    // bind filter
    // TODO(pgao): support variable grouping filters
    if let Some(filter) = filter {
        let ectx = pctx.derive_expr_context(&inner_scope, "QuantifiedPathPattern Filter");
        let expr = bind_expr(&ectx, &[], filter)?;
        if expr.typ() != DataType::Bool {
            return Err(PlanError::semantic_err(
                "QuantifiedPathPattern filter must be boolean expression".to_string(),
            ));
        }
        inner_filter.push(expr);
    };

    let qpp = QuantifiedPathPattern {
        left_binding,
        right_binding,
        rels,
        repetition,
        node_grouping,
        rel_grouping,
        filter: inner_filter,
    };

    let extra = NodeConnectionExtra {
        outer: Default::default(), // quantified path pattern do not allow reference outer variables
        post_filter: FilterExprs::empty(),
    };

    // add group variable in current scope
    for vg in qpp.node_grouping.iter() {
        let symbol = &inner_scope
            .resolve_variable(&vg.singleton)
            // safety: must be resolved, since do not allow implicit join in QPP
            .unwrap()
            .symbol;
        let item = ScopeItem::new_variable(vg.group.clone(), symbol.as_deref(), DataType::VirtualNode);
        scope.add_item(item);
    }
    for vg in qpp.rel_grouping.iter() {
        let symbol = &inner_scope
            .resolve_variable(&vg.singleton)
            // safety: must be resolved, since do not allow implicit join in QPP
            .unwrap()
            .symbol;
        let item = ScopeItem::new_variable(vg.group.clone(), symbol.as_deref(), DataType::Rel);
        scope.add_item(item);
    }

    Ok((qpp, extra, scope))
}

/// Return (Variable, is_outer)
/// When is_outer is true, means the pattern works inside an subquery
/// bind an symbol to an variable in scope
/// if symbol is none, create an anonymous variable
fn bind_variable(
    pctx: &PatternContext,
    scope: &mut Scope,
    name: Option<&str>, // None for anonymous variable
    typ: &DataType,     // expected data type
) -> Result<(Variable, bool), PlanError> {
    if let Some(name) = name {
        // named variable
        if let Some((variable, is_outer)) = resolve_variable(pctx, scope, name)? {
            Ok((variable, is_outer))
        } else {
            // introduce a new named variable
            let var_name = pctx.bctx.variable_generator.named(name);
            let item = ScopeItem::new_variable(var_name, Some(name), typ.clone());
            let var = item.as_variable();
            scope.add_item(item);
            Ok((var, false))
        }
    } else {
        // introduce an anonymous node variable
        let var_name = pctx.bctx.variable_generator.unnamed();
        let item = ScopeItem::new_variable(var_name, None, typ.clone());
        let var = item.as_variable();
        scope.add_item(item);
        Ok((var, false))
    }
}

// If bound, return (variable, is_outer)
fn resolve_variable(pctx: &PatternContext, scope: &Scope, name: &str) -> Result<Option<(Variable, bool)>, PlanError> {
    // find if variable already defined in in_scope
    // check in pctx for imported variables
    if let Some(item) = scope.resolve_symbol(name) {
        return Ok(Some((Variable::new(&item.variable, &item.typ), false)));
    }
    // check if this is an outer reference
    for scope in pctx.bctx.outer_scopes.iter() {
        if let Some(item) = scope.resolve_symbol(name) {
            return Ok(Some((Variable::new(&item.variable, &item.typ), true)));
        }
    }
    Ok(None)
}

// need to pass scope here since the properties expression may reference imported variables
fn bind_properties(
    pctx: &PatternContext,
    scope: &Scope,
    var: &Variable,
    props: &ast::Expr,
) -> Result<FilterExprs, PlanError> {
    let ectx = pctx.derive_expr_context(scope, "Variable Properties");
    let mut filter = FilterExprs::empty();
    if let ast::Expr::MapExpression { keys, values } = props {
        for (key, value) in keys.iter().zip(values.iter()) {
            let token = pctx.bctx.resolve_token(key, TokenKind::PropertyKey);
            let value = bind_expr(&ectx, &[], value)?;
            // TODO(pgao): maybe we can inference the properties here
            let prop = Expr::PropertyAccess(PropertyAccess::new_unchecked(
                Box::new(Expr::from_variable(var)),
                &token,
                &DataType::Any,
            ));
            let equal = prop.equal(value);
            filter.push(equal);
        }
        Ok(filter)
    } else {
        // this should be an parser error
        unreachable!()
    }
}
