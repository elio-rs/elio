use super::*;

#[derive(Debug, Clone)]
pub struct Unwind {
    pub base: PlanBase,
    pub(crate) inner: UnwindInner,
}

impl Unwind {
    pub fn new(inner: UnwindInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnwindInner {
    pub input: Box<PlanExpr>,
    pub expr: Expr,
    pub variable: Variable,
}

impl InnerNode for UnwindInner {
    fn build_base(&self) -> PlanBase {
        let mut schema = self.input.schema().as_ref().clone();
        schema.add_column(self.variable.clone());
        PlanBase::new(schema.into(), self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}

impl PlanNode for Unwind {
    type Inner = UnwindInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![
            ("variable", Pretty::display(&self.inner.variable.name)),
            ("expr", self.inner.expr.pretty().into()),
        ];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("Unwind", fields, children)
    }
}
