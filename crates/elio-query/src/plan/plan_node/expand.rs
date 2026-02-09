use elio_common::{IrToken, SemanticDirection};

use super::*;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display)]
pub enum ExpandKind {
    #[default]
    #[display("All")]
    // input (a), output (a)-[r]-(b)
    All,
    #[display("Into")]
    // input (a), (b), output (a)-[r]-(b)
    Into,
}

// Output Rel and VirtualNode
#[derive(Clone, Debug)]
pub struct Expand {
    pub base: PlanBase,
    pub(crate) inner: ExpandInner,
}

impl Expand {
    pub fn new(inner: ExpandInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Expand {
    type Inner = ExpandInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![
            ("from", Pretty::from(self.inner.from.as_ref())),
            ("to", Pretty::from(self.inner.to.as_ref())),
            ("rel", Pretty::from(self.inner.rel.as_ref())),
            ("direction", Pretty::from(self.inner.direction.to_string())),
            (
                "types",
                Pretty::Array(
                    self.inner
                        .types
                        .iter()
                        .map(|x| Pretty::from(x.to_string()))
                        .collect_vec(),
                ),
            ),
        ];
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        let name = match self.inner.kind {
            ExpandKind::All => "ExpandAll",
            ExpandKind::Into => "ExpandInto",
        };
        XmlNode::simple_record(name, fields, children)
    }
}

#[derive(Clone, Debug)]
pub struct ExpandInner {
    pub input: Box<PlanExpr>,
    pub from: VariableName,
    pub to: VariableName,
    pub rel: VariableName,
    pub direction: SemanticDirection,
    pub types: Vec<IrToken>,
    pub kind: ExpandKind,
}

impl ExpandInner {
    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::from_arc(self.input.schema());
        match self.kind {
            ExpandKind::All => {
                // add [r, to] to output
                schema.fields.push(Variable::new(&self.rel, &DataType::Rel));
                schema.fields.push(Variable::new(&self.to, &DataType::VirtualNode));
            }
            // add [r] to output
            ExpandKind::Into => schema.fields.push(Variable::new(&self.rel, &DataType::Rel)),
        }
        schema.into()
    }
}

impl InnerNode for ExpandInner {
    fn build_base(&self) -> PlanBase {
        let schema = self.build_schema();
        PlanBase::new(schema, self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
