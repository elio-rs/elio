use super::*;
use crate::plan::pretty_utils::pretty_project_items;

#[derive(Debug, Clone)]
pub struct Project {
    pub base: PlanBase,
    pub(crate) inner: ProjectInner,
}

impl Project {
    pub fn new(inner: ProjectInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Project {
    type Inner = ProjectInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![(
            "exprs",
            pretty_project_items(self.inner.projections.iter().map(|(k, v)| (k, v))),
        )];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("Project", fields, children)
    }
}

#[derive(Debug, Clone)]
pub struct ProjectInner {
    pub input: Box<PlanExpr>,
    pub projections: Vec<(VariableName, Expr)>,
    // TODO(pgao): func deps
}

impl ProjectInner {
    // add all existing projects as pass through
    pub fn new_from_input(input: Box<PlanExpr>) -> Self {
        let existing = input
            .schema()
            .fields
            .iter()
            .map(|x| (x.name.clone(), Expr::from_variable(x)))
            .collect_vec();
        Self {
            input,
            projections: existing,
        }
    }

    // add an project without checking the variable name conflicit
    pub fn add_unchecked(&mut self, var: VariableName, expr: Expr) {
        self.projections.push((var, expr));
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: Fn(&(VariableName, Expr)) -> bool,
    {
        self.projections.retain(f);
    }

    pub fn map_exprs<F>(&mut self, mut f: F)
    where
        F: FnMut(Expr) -> Expr,
    {
        for (_, expr) in self.projections.iter_mut() {
            let current = std::mem::replace(expr, Expr::boolean(true));
            *expr = f(current);
        }
    }
}

impl ProjectInner {
    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::empty();
        for (var, expr) in &self.projections {
            schema.fields.push(Variable::new(var, &expr.typ()));
        }
        schema.into()
    }
}

impl InnerNode for ProjectInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.build_schema(), self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
