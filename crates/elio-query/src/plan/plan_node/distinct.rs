use super::*;
use crate::plan::pretty_utils::pretty_project_items;

#[derive(Debug, Clone)]
pub struct Distinct {
    pub base: PlanBase,
    pub(crate) inner: DistinctInner,
}

impl Distinct {
    pub fn new(inner: DistinctInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Distinct {
    type Inner = DistinctInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![(
            "group_exprs",
            pretty_project_items(self.inner.group_exprs.iter().map(|(k, v)| (k, v))),
        )];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("Distinct", fields, children)
    }
}

#[derive(Debug, Clone)]
pub struct DistinctInner {
    pub input: Box<PlanExpr>,
    pub group_exprs: Vec<(VariableName, Expr)>,
}

impl DistinctInner {
    pub fn retain<F>(&mut self, f: F)
    where
        F: Fn(&(VariableName, Expr)) -> bool,
    {
        self.group_exprs.retain(f);
    }

    pub fn map_exprs<F>(&mut self, mut f: F)
    where
        F: FnMut(Expr) -> Expr,
    {
        for (_, expr) in self.group_exprs.iter_mut() {
            let current = std::mem::replace(expr, Expr::boolean(true));
            *expr = f(current);
        }
    }

    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::empty();
        for (var, expr) in self.group_exprs.iter() {
            schema.add_column(Variable::new(var, &expr.typ()));
        }
        schema.into()
    }
}

impl InnerNode for DistinctInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.build_schema(), self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
