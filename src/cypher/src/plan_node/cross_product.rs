use super::*;

#[derive(Debug, Clone)]
pub struct CrossProduct {
    pub base: PlanBase,
    pub(crate) inner: CrossProductInner,
}

impl CrossProduct {
    pub fn new(inner: CrossProductInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for CrossProduct {
    type Inner = CrossProductInner;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn xmlnode(&self) -> XmlNode<'_> {
        let children = self
            .inputs()
            .iter()
            .map(|x| x.xmlnode())
            .map(Pretty::Record)
            .collect_vec();
        XmlNode::simple_record("CrossProduct", vec![], children)
    }
}

// NB: left and right may have overlapping variables
// in the case of
// - Apply
//   - CrossProduct
//     - NodeScan
//     - Argument
//   - CrossProduct
//     - NodeScan
//     - Argument
// The argument variable will be the overlapping varaible
// and the output schema will be the union set of left and right,
// we keep the left side overlapping varaible as output.
#[derive(Debug, Clone)]
pub struct CrossProductInner {
    pub left: Box<PlanExpr>,
    pub right: Box<PlanExpr>,
}

impl CrossProductInner {
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

impl InnerNode for CrossProductInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.build_schema(), self.left.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.left, &self.right]
    }
}
