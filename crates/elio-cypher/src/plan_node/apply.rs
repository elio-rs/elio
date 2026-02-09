use super::*;

#[derive(Debug, Clone)]
pub struct Apply {
    pub base: PlanBase,
    pub(crate) inner: ApplyInner,
}

impl Apply {
    pub fn new(inner: ApplyInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Apply {
    type Inner = ApplyInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let children = self
            .inputs()
            .iter()
            .map(|x| x.xmlnode())
            .map(Pretty::Record)
            .collect_vec();
        XmlNode::simple_record("Apply", vec![], children)
    }
}

#[derive(Debug, Clone)]
pub struct ApplyInner {
    pub left: Box<PlanExpr>,
    pub right: Box<PlanExpr>,
    // TODO(pgao): shmld we put argument here?
}

impl ApplyInner {
    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::from_arc(self.left.schema());
        let right_schema = self.right.schema();
        let left_vars: std::collections::HashSet<_> = schema.fields.iter().map(|f| f.name.clone()).collect();

        for item in right_schema.fields.iter() {
            if !left_vars.contains(&item.name) {
                schema.fields.push(item.clone());
            }
        }
        schema.into()
    }
}

impl InnerNode for ApplyInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.build_schema(), self.left.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.left, &self.right]
    }
}
