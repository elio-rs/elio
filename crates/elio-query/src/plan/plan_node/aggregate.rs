use super::*;
use crate::plan::pretty_utils::pretty_project_items;

// HashAggregate Node.
// TODO(pgao): add StreamingAgg
#[derive(Debug, Clone)]
pub struct Aggregate {
    pub base: PlanBase,
    pub(crate) inner: AggregateInner,
}

impl Aggregate {
    pub fn new(inner: AggregateInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Aggregate {
    type Inner = AggregateInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let mut fields = vec![];
        if !self.inner.group_by.is_empty() {
            fields.push((
                "group_by",
                pretty_project_items(self.inner.group_by.iter().map(|(k, v)| (k, v))),
            ));
        }
        if !self.inner.aggregate.is_empty() {
            fields.push((
                "aggregate",
                pretty_project_items(self.inner.aggregate.iter().map(|(k, v)| (k, v))),
            ));
        }
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("Aggregate", fields, children)
    }
}

#[derive(Debug, Clone)]
pub struct AggregateInner {
    pub input: Box<PlanExpr>,
    pub group_by: Vec<(VariableName, Expr)>,
    pub aggregate: Vec<(VariableName, Expr)>,
}

impl AggregateInner {
    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::empty();
        for (name, expr) in self.group_by.iter().chain(self.aggregate.iter()) {
            schema.fields.push(Variable::new(name, &expr.typ()));
        }
        schema.into()
    }
}

impl InnerNode for AggregateInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.build_schema(), self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
