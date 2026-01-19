use super::*;
use crate::plan_node::plan_base::PlanBase;
use crate::plan_node::{InnerNode, PlanExpr, PlanNode};

/// BlackHole will consume the input stream and return zero rows.
/// This is used in DML statements like CREATE, DELETE that they may have side-effects
/// and BlackHole will drive the side-effects.
#[derive(Debug, Clone)]
pub struct BlackHole {
    pub base: PlanBase,
    pub(crate) inner: BlackHoleInner,
}

impl BlackHole {
    pub fn new(inner: BlackHoleInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for BlackHole {
    type Inner = BlackHoleInner;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn xmlnode(&self) -> XmlNode<'_> {
        XmlNode::simple_record("BlackHole", vec![], vec![Pretty::Record(self.inner.input.xmlnode())])
    }
}

#[derive(Debug, Clone)]
pub struct BlackHoleInner {
    pub input: Box<PlanExpr>,
}

impl InnerNode for BlackHoleInner {
    fn build_base(&self) -> PlanBase {
        PlanBase::new(self.input.schema(), self.input.ctx())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }
}
