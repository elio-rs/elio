use std::sync::Arc;

use elio_common::order::ColumnOrder;
use elio_common::schema::Schema;

use super::*;
use crate::plan::plan_node::plan_base::PlanBase;
use crate::plan::plan_node::{InnerNode, PlanExpr, PlanNode};

#[derive(Debug, Clone)]
pub struct Sort {
    pub base: PlanBase,
    pub(crate) inner: SortInner,
}

impl Sort {
    pub fn new(inner: SortInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Sort {
    type Inner = SortInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![(
            "items",
            Pretty::Array(
                self.inner
                    .items
                    .iter()
                    .map(|x| Pretty::from(x.to_string()))
                    .collect_vec(),
            ),
        )];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("Sort", fields, children)
    }
}

#[derive(Debug, Clone)]
pub struct SortInner {
    pub input: Box<PlanExpr>,
    pub items: Vec<ColumnOrder>,
}
impl SortInner {
    fn build_schema(&self) -> Arc<Schema> {
        self.input.schema()
    }
}

impl InnerNode for SortInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.build_schema(), self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
