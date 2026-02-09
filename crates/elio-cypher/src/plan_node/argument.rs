use educe::Educe;
use itertools::Itertools;

use super::*;

#[derive(Debug, Clone)]
pub struct Argument {
    pub base: PlanBase,
    pub(crate) inner: ArgumentInner,
}

impl Argument {
    pub fn new(inner: ArgumentInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for Argument {
    type Inner = ArgumentInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![(
            "variables",
            Pretty::Array(
                self.inner
                    .variables
                    .iter()
                    .map(|x| Pretty::from(x.name.as_ref()))
                    .collect_vec(),
            ),
        )];
        XmlNode::simple_record("Argument", fields, Default::default())
    }
}

#[derive(Educe)]
#[educe(Debug, Clone)]
pub struct ArgumentInner {
    pub variables: Vec<Variable>,
    #[educe(Debug(ignore))]
    pub ctx: Arc<PlanContext>,
}

impl ArgumentInner {
    fn build_schema(&self) -> Arc<Schema> {
        Arc::new(self.variables.iter().cloned().collect::<Schema>())
    }
}

impl InnerNode for ArgumentInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.build_schema(), self.ctx.clone())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![]
    }
}
