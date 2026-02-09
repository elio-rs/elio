use super::*;

#[derive(Debug, Clone)]
pub struct Pagination {
    pub base: PlanBase,
    pub(crate) inner: PaginationInner,
}

impl Pagination {
    pub fn new(inner: PaginationInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Pagination {
    type Inner = PaginationInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![
            ("offset", Pretty::from(self.inner.offset.to_string())),
            ("limit", Pretty::from(self.inner.limit.to_string())),
        ];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("Pagination", fields, children)
    }
}

#[derive(Debug, Clone)]
pub struct PaginationInner {
    pub input: Box<PlanExpr>,
    pub offset: u64,
    pub limit: u64,
}

impl InnerNode for PaginationInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.input.schema(), self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
