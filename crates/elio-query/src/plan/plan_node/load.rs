use educe::Educe;

use super::*;
use crate::plan::ir::query_project::LoadFormat;

#[derive(Debug, Clone)]
pub struct Load {
    pub base: PlanBase,
    pub(crate) inner: LoadInner,
}

impl Load {
    pub fn new(inner: LoadInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

// Format outputs:
//   - csv
//     - with header: output Struct type, but we do not know the struct keys in advance
//     - without header: output List type, but we do not know the list items in advance
//     so we just output Any datatype and put the type inference at runtime.
#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct LoadInner {
    #[educe(Debug(ignore))]
    pub ctx: Arc<PlanContext>,
    pub source_url: String,
    pub variable: VariableName,
    pub format: LoadFormat,
}

impl InnerNode for LoadInner {
    fn build_base(&self) -> PlanBase {
        let mut schema = Schema::empty();
        schema.add_column(Variable::new(&self.variable, &self.format.output_type()));
        PlanBase::new(schema.into(), self.ctx.clone())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![]
    }
}

impl PlanNode for Load {
    type Inner = LoadInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        XmlNode::simple_record(
            "Load",
            vec![
                ("source_url", Pretty::from(self.inner.source_url.as_str())),
                ("variable", Pretty::from(self.inner.variable.as_ref())),
                ("format", Pretty::Record(self.inner.format.xmlnode())),
            ],
            vec![],
        )
    }
}
