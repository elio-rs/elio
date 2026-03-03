use std::collections::HashSet;

use elio_common::data_type::LogicalType;
use elio_common::store_types::RelDirection;
use elio_common::{EntityKind, IrToken, SemanticDirection, TokenKind};
use elio_parser::ast::{self, NodePattern, RelationshipPattern, UpdatePattern};

use crate::binder::BindContext;
use crate::binder::builder::IrSingleQueryBuilder;
use crate::binder::expr::{ExprContext, bind_map_expr_to_property_map};
use crate::binder::pattern::PatternContext;
use crate::binder::query::ClauseKind;
use crate::binder::scope::{Scope, ScopeItem};
use crate::plan::error::{PlanError, SemanticError};
use crate::plan::expr::CreateStruct;
use crate::plan::ir::MutatingPattern;
use crate::plan::ir::mutating_pattern::{CreateNode, CreatePattern, CreateRel};

pub fn bind_create(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    in_scope: Scope,
    _create @ ast::CreateClause { pattern }: &ast::CreateClause,
) -> Result<Scope, PlanError> {
    let pctx = PatternContext {
        bctx,
        clause: ClauseKind::Create,
        name: &_create.to_string(),
        allow_update: true,
        // create pattern does not allow qpp, selective
        // we do not support named path here
        reject_qpp: true,
        reject_named_path: false,
        reject_selective: true,
    };

    let mut scope = bind_create_pattern(&pctx, builder, in_scope, pattern)?;
    scope.remove_anonymous();
    Ok(scope)
}

/// Semantic
/// 1. Create pattern must not contain qpp
/// 2. Relationships must be typed and with single type
/// 3. Relationships must be directed
/// 4. Nodes cannot be created multiple times EXPLAIN CREATE (n:Actor), (n)-[r:B]->(c) RETURN *; this is valid EXPLAIN
///    CREATE (n:Actor), (n:Actor)-[r:B]->(c) RETURN *; this is invalid
fn bind_create_pattern(
    pctx: &PatternContext,
    builder: &mut IrSingleQueryBuilder,
    in_scope: Scope,
    _pattern @ UpdatePattern { patterns }: &ast::UpdatePattern,
) -> Result<Scope, PlanError> {
    // check qpp
    if patterns.iter().any(|x| x.contains_qpp()) {
        return Err(SemanticError::qpp_not_allowed("CREATE", &_pattern.to_string()).into());
    }

    let mut create_scope = Scope::empty();
    let mut create_nodes = vec![];
    let mut create_rels = vec![];

    let simples = patterns
        .iter()
        .map(|x| x.as_simple_patterns().unwrap())
        .collect::<Vec<_>>();

    for simple in simples.iter() {
        bind_create_part(
            pctx,
            &in_scope,
            &mut create_scope,
            &mut create_nodes,
            &mut create_rels,
            simple,
        )?;
    }

    let create_pattern = CreatePattern {
        nodes: create_nodes,
        rels: create_rels,
    };

    builder
        .tail_mut()
        .unwrap()
        .add_mutating_pattern(MutatingPattern::Create(create_pattern));
    let out_scope = in_scope.product(create_scope);
    Ok(out_scope)
}

fn bind_create_part(
    pctx: &PatternContext,
    in_scope: &Scope,
    create_scope: &mut Scope,
    create_nodes: &mut Vec<CreateNode>,
    create_rels: &mut Vec<CreateRel>,
    _simple @ ast::SimplePathPattern { nodes, relationships }: &ast::SimplePathPattern,
) -> Result<(), PlanError> {
    let mut new_nodes = vec![];
    let mut new_rels = vec![];
    let pattern_str = _simple.to_string();
    let ectx = pctx.derive_expr_context(in_scope, &pattern_str);

    for NodePattern {
        variable,
        label_expr,
        properties,
        predicate,
    } in nodes
    {
        // node must be labled
        // symbol must not defined in inscope
        // symbol must not be defeind in create_scope
        // predicate must be non
        let labels = bind_label_expr_for_create(&ectx, label_expr.as_ref(), &EntityKind::Node)?;
        let properties = bind_properties_for_create(&ectx, properties.as_ref())?;
        if predicate.is_some() {
            return Err(SemanticError::invalid_create_entity(&pattern_str).into());
        }

        // bind var
        let var_name = {
            if let Some(symbol) = variable.as_deref() {
                let (is_reference, var_name) = {
                    if let Some(item) = create_scope.resolve_symbol(symbol) {
                        (true, item.variable.clone())
                    } else if let Some(item) = in_scope.resolve_symbol(symbol) {
                        (true, item.variable.clone())
                    } else {
                        // add symbol to create_scope
                        let var_name = pctx.bctx.variable_generator.named(symbol);
                        let item = ScopeItem {
                            symbol: Some(symbol.to_owned()),
                            variable: var_name.clone(),
                            expr: Default::default(),
                            typ: LogicalType::NODE,
                            bound_expr: None,
                        };
                        create_scope.add_item(item);
                        (false, var_name)
                    }
                };

                if is_reference && (!labels.is_empty() || !properties.is_empty()) {
                    return Err(SemanticError::invalid_create_entity(&pattern_str).into());
                }

                if !is_reference && (labels.is_empty()) {
                    return Err(SemanticError::invalid_create_entity(&pattern_str).into());
                }
                var_name
            } else {
                // anonymous, new variable, label should not be empty
                if labels.is_empty() {
                    return Err(SemanticError::invalid_create_entity(&pattern_str).into());
                }

                // add variable to create_scope
                let var_name = pctx.bctx.variable_generator.unnamed();
                let item = ScopeItem {
                    symbol: None,
                    variable: var_name.clone(),
                    expr: Default::default(),
                    typ: LogicalType::NODE,
                    bound_expr: None,
                };
                create_scope.add_item(item);
                var_name
            }
        };

        let create = CreateNode {
            variable: var_name,
            labels: HashSet::from_iter(labels),
            properties,
        };
        new_nodes.push(create);
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
        let labels = bind_label_expr_for_create(&ectx, label_expr.as_ref(), &EntityKind::Rel)?;
        let properties = bind_properties_for_create(&ectx, properties.as_ref())?;
        // only can be directed
        if predicate.is_some() || length.is_some() || direction.is_both() {
            return Err(SemanticError::invalid_create_entity(&pattern_str).into());
        }

        // bind var
        let var_name = {
            if let Some(symbol) = variable.as_deref() {
                // check symbol not defined in inscope and create_scope
                if in_scope.resolve_symbol(symbol).is_some() || create_scope.resolve_symbol(symbol).is_some() {
                    return Err(SemanticError::invalid_create_entity(&pattern_str).into());
                }
                // add symbol to create_scope
                let var_name = pctx.bctx.variable_generator.named(symbol);
                let item = ScopeItem {
                    symbol: Some(symbol.to_owned()),
                    variable: var_name.clone(),
                    expr: Default::default(),
                    typ: LogicalType::REL,
                    bound_expr: None,
                };
                create_scope.add_item(item);
                var_name
            } else {
                // anonymous, label should not be empty
                if labels.is_empty() {
                    return Err(SemanticError::invalid_create_entity(&pattern_str).into());
                }
                // add variable to create_scope
                let var_name = pctx.bctx.variable_generator.unnamed();
                let item = ScopeItem {
                    symbol: None,
                    variable: var_name.clone(),
                    expr: Default::default(),
                    typ: LogicalType::REL,
                    bound_expr: None,
                };
                create_scope.add_item(item);
                var_name
            }
        };

        let direction = match direction {
            SemanticDirection::Outgoing => RelDirection::Out,
            SemanticDirection::Incoming => RelDirection::In,
            SemanticDirection::Both => {
                return Err(SemanticError::invalid_create_entity(&pattern_str).into());
            }
        };

        let create = CreateRel {
            variable: var_name,
            left: new_nodes[i].variable.clone(),
            right: new_nodes[i + 1].variable.clone(),
            reltype: labels.into_iter().next().unwrap(),
            direction,
            properties,
        };
        new_rels.push(create);
    }

    // remove reference node
    new_nodes.retain(|node| !node.labels.is_empty());

    create_nodes.extend(new_nodes);
    create_rels.extend(new_rels);

    Ok(())
}

fn bind_label_expr_for_create(
    ectx: &ExprContext,
    label_expr: Option<&ast::LabelExpr>,
    kind: &EntityKind,
) -> Result<Vec<IrToken>, PlanError> {
    // let label_expr = label_expr.ok_or(PlanError::from(SemanticError::invalid_create_entity(ectx.name)))?;

    let label_expr = match label_expr {
        Some(lexpr) => lexpr,
        None => {
            return Ok(vec![]);
        }
    };

    let labels = {
        if !label_expr.contains_only_and() {
            return Err(SemanticError::invalid_create_entity(ectx.name).into());
        } else {
            label_expr.leafs()
        }
    };

    let token_kind = match kind {
        EntityKind::Node => TokenKind::Label,
        EntityKind::Rel => TokenKind::RelationshipType,
    };

    let tokens: Vec<IrToken> = labels
        .iter()
        .map(|token| ectx.bctx.resolve_token(token, token_kind))
        .collect::<Vec<_>>();
    if matches!(kind, EntityKind::Rel) && tokens.len() != 1 {
        return Err(SemanticError::invalid_create_entity(ectx.name).into());
    }

    Ok(tokens)
}

fn bind_properties_for_create(ectx: &ExprContext, properties: Option<&ast::Expr>) -> Result<CreateStruct, PlanError> {
    let props = if let Some(ast::Expr::MapExpression { keys, values }) = properties {
        // do not allow referece outer scope
        bind_map_expr_to_property_map(ectx, &[], keys, values)?
    } else {
        vec![]
    };

    Ok(CreateStruct::new(props))
}
