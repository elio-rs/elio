use std::sync::Arc;

use elio_common::IrToken;
use elio_common::schema::{Schema, Variable};

use super::*;
use crate::plan::expr::BoxedExpr;
use crate::plan::plan_node::plan_base::PlanBase;
use crate::plan::plan_node::{InnerNode, PlanExpr, PlanNode};

// Return Rel
#[derive(Clone, Debug)]
pub struct CreateRel {
    pub base: PlanBase,
    pub(crate) inner: CreateRelInner,
}

impl CreateRel {
    pub fn new(inner: CreateRelInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for CreateRel {
    type Inner = CreateRelInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![(
            "items",
            Pretty::Array(
                self.inner
                    .rels
                    .iter()
                    .map(|x| Pretty::Record(x.xmlnode()))
                    .collect_vec(),
            ),
        )];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("CreateRel", fields, children)
    }
}

#[derive(Clone, Debug)]
pub struct CreateRelItem {
    pub reltype: IrToken,
    pub start_node: Variable,
    pub end_node: Variable,
    pub properties: BoxedExpr,
    pub variable: Variable,
}

impl CreateRelItem {
    pub fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![
            ("variable", Pretty::from(self.variable.name.as_ref())),
            ("reltype", Pretty::from(self.reltype.to_string())),
            ("start_node", Pretty::from(self.start_node.name.as_ref())),
            ("end_node", Pretty::from(self.end_node.name.as_ref())),
            ("properties", Pretty::from(self.properties.pretty())),
        ];
        XmlNode::simple_record("CreateRelItem", fields, vec![])
    }
}

#[derive(Clone, Debug)]
pub struct CreateRelInner {
    pub(crate) input: Box<PlanExpr>,
    pub rels: Vec<CreateRelItem>,
}

impl CreateRelInner {
    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::from_arc(self.input.schema());
        for item in self.rels.iter() {
            schema.add_column(item.variable.clone());
        }
        schema.into()
    }
}

impl InnerNode for CreateRelInner {
    fn build_base(&self) -> PlanBase {
        let schema = self.build_schema();
        PlanBase::new(schema, self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
