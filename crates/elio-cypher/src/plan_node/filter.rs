use super::*;
use crate::expr::FilterExprs;

#[derive(Debug, Clone)]
pub struct Filter {
    pub base: PlanBase,
    pub(crate) inner: FilterInner,
}

impl Filter {
    pub fn new(inner: FilterInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Filter {
    type Inner = FilterInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![("condition", Pretty::from(self.inner.condition.pretty()))];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("Filter", fields, children)
    }
}

#[derive(Debug, Clone)]
pub struct FilterInner {
    pub input: Box<PlanExpr>,
    pub condition: FilterExprs,
}

impl FilterInner {
    fn build_schema(&self) -> Arc<Schema> {
        self.input.schema().clone()
    }
}

impl InnerNode for FilterInner {
    fn build_base(&self) -> PlanBase {
        let schema = self.build_schema();
        PlanBase::new(schema, self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
