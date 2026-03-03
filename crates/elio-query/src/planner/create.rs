use elio_common::data_type::LogicalType;
use elio_common::schema::Variable;
use elio_common::store_types::RelDirection;
use itertools::Itertools;

use crate::plan::error::PlanError;
use crate::plan::expr::Expr;
use crate::plan::plan_node::{
    CreateNode, CreateNodeInner, CreateNodeItem, CreateRel, CreateRelInner, CreateRelItem, PlanExpr,
};
use crate::planner::PlannerContext;

pub(super) fn plan_create(
    ctx: &mut PlannerContext,
    mut root: Box<PlanExpr>,
    _create @ crate::plan::ir::CreatePattern { nodes, rels }: &crate::plan::ir::CreatePattern,
) -> Result<Box<PlanExpr>, PlanError> {
    if !nodes.is_empty() {
        root = plan_create_nodes(ctx, root, nodes)?;
    }

    if !rels.is_empty() {
        root = plan_create_rels(ctx, root, rels)?;
    }
    Ok(root)
}

fn plan_create_nodes(
    _ctx: &mut PlannerContext,
    root: Box<PlanExpr>,
    nodes: &[crate::plan::ir::CreateNode],
) -> Result<Box<PlanExpr>, PlanError> {
    let items = nodes
        .iter()
        .map(|node| CreateNodeItem {
            labels: node.labels.clone().into_iter().collect(),
            properties: Expr::from(node.properties.clone()).boxed(),
            variable: Variable::new(&node.variable.clone(), &LogicalType::NODE),
        })
        .collect_vec();
    let inner = CreateNodeInner {
        input: root,
        nodes: items,
    };

    Ok(PlanExpr::CreateNode(CreateNode::new(inner)).boxed())
}

fn plan_create_rels(
    _ctx: &mut PlannerContext,
    root: Box<PlanExpr>,
    rels: &[crate::plan::ir::CreateRel],
) -> Result<Box<PlanExpr>, PlanError> {
    let items = rels
        .iter()
        .map(|rel| {
            let (start, end) = {
                match rel.direction {
                    RelDirection::Out => (rel.left.clone(), rel.right.clone()),
                    RelDirection::In => (rel.right.clone(), rel.left.clone()),
                }
            };

            CreateRelItem {
                reltype: rel.reltype.clone(),
                start_node: Variable::new(&start, &LogicalType::NODE),
                end_node: Variable::new(&end, &LogicalType::NODE),
                properties: Expr::from(rel.properties.clone()).boxed(),
                variable: Variable::new(&rel.variable.clone(), &LogicalType::REL),
            }
        })
        .collect_vec();

    let inner = CreateRelInner {
        input: root,
        rels: items,
    };

    Ok(PlanExpr::CreateRel(CreateRel::new(inner)).boxed())
}
