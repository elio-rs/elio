use super::*;

/// Produce results
///   - materialize virtual node to node
///   - change output column order
#[derive(Debug, Clone)]
pub struct ProduceResult {
    pub base: PlanBase,
    pub(crate) inner: ProduceResultInner,
}

impl ProduceResult {
    pub fn new(inner: ProduceResultInner) -> Self {
        let base = inner.build_base();
        Self { base, inner }
    }
}

impl PlanNode for ProduceResult {
    type Inner = ProduceResultInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![("return_columns", Pretty::from(self.inner.return_columns.join(",")))];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record("ProduceResult", fields, children)
    }
}

#[derive(Debug, Clone)]
pub struct ProduceResultInner {
    pub input: Box<PlanExpr>,
    // output column in this order
    pub return_columns: Vec<VariableName>,
}

impl ProduceResultInner {
    fn build_schema(&self) -> Arc<Schema> {
        let input_schema = self.input.schema();
        let mut schema = Schema::empty();
        for var in self.return_columns.iter() {
            schema.add_column(Variable {
                name: var.clone(),
                typ: input_schema
                    .column_by_name(var)
                    .expect("column in return list must exist in input schema")
                    .typ
                    .materialize(),
            });
        }
        Arc::new(schema)
    }
}

impl InnerNode for ProduceResultInner {
    fn build_base(&self) -> PlanBase {
        let schema = self.build_schema();
        PlanBase::new(schema, self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
