use std::sync::Arc;

use elio_common::IrToken;
use elio_common::schema::{Schema, Variable};

use super::*;
use crate::plan::expr::BoxedExpr;
use crate::plan::plan_node::plan_base::PlanBase;
use crate::plan::plan_node::{InnerNode, PlanExpr, PlanNode};

// Return Node
#[derive(Clone, Debug)]
pub struct CreateNode {
    pub base: PlanBase,
    pub inner: CreateNodeInner,
}

impl CreateNode {
    pub fn new(inner: CreateNodeInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for CreateNode {
    type Inner = CreateNodeInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![(
            "items",
            Pretty::Array(
                self.inner
                    .nodes
                    .iter()
                    .map(|x| Pretty::Record(x.xmlnode()))
                    .collect_vec(),
            ),
        )];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("CreateNode", fields, children)
    }
}

#[derive(Clone, Debug)]
pub struct CreateNodeInner {
    pub input: Box<PlanExpr>,
    pub nodes: Vec<CreateNodeItem>,
}

#[derive(Debug, Clone)]
pub struct CreateNodeItem {
    pub labels: Vec<IrToken>,
    pub properties: BoxedExpr,
    pub variable: Variable,
}

impl CreateNodeItem {
    pub fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![
            ("variable", Pretty::display(&self.variable.name)),
            (
                "labels",
                Pretty::Array(self.labels.iter().map(|x| Pretty::from(x.to_string())).collect_vec()),
            ),
            ("properties", Pretty::from(self.properties.pretty())),
        ];
        XmlNode::simple_record("CreateNodeItem", fields, vec![])
    }
}

impl CreateNodeInner {
    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::from_arc(self.input.schema());
        for node in &self.nodes {
            schema.add_column(node.variable.clone());
        }
        schema.into()
    }
}
impl InnerNode for CreateNodeInner {
    fn build_base(&self) -> PlanBase {
        let schema = self.build_schema();
        PlanBase::new(schema, self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
