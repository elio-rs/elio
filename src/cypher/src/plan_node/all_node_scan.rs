use educe::{self, Educe};
use itertools::Itertools;

use super::*;
// TODO(pgao): associate the catalog object here?
// seems we should have an logical plan here?

// Return VirtualNode
#[derive(Debug, Clone)]
pub struct AllNodeScan {
    pub base: PlanBase,
    pub(crate) inner: AllNodeScanInner,
}

impl AllNodeScan {
    pub fn new(inner: AllNodeScanInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for AllNodeScan {
    type Inner = AllNodeScanInner;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn xmlnode(&self) -> XmlNode<'_> {
        let mut fields = vec![("variable", Pretty::from(self.inner.variable.as_ref()))];
        if let Some(arguments) = self.inner.arguments.as_ref() {
            fields.push((
                "arguments",
                Pretty::Array(
                    arguments
                        .schema()
                        .columns()
                        .iter()
                        .map(|x| Pretty::display(&x.name.clone()))
                        .collect_vec(),
                ),
            ));
        }
        XmlNode::simple_record("AllNodeScan", fields, Default::default())
    }
}

#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct AllNodeScanInner {
    pub variable: VariableName,
    pub arguments: Option<Box<PlanExpr>>, // must be argument
    #[educe(Debug(ignore))]
    pub ctx: Arc<PlanContext>,
}

impl AllNodeScanInner {
    pub fn new(variable: VariableName, arguments: Option<Box<PlanExpr>>, ctx: Arc<PlanContext>) -> Self {
        if let Some(ref input) = arguments {
            assert!(input.as_argument().is_some());
        }
        Self {
            variable,
            arguments,
            ctx,
        }
    }

    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::empty();
        schema.fields.push(Variable {
            name: self.variable.clone(),
            typ: DataType::VirtualNode,
        });
        let optional_arguments = self.arguments.as_ref().map(|x| x.schema()).unwrap_or_default();
        schema.fields.extend(optional_arguments.iter().cloned());
        schema.into()
    }
}

impl InnerNode for AllNodeScanInner {
    fn build_base(&self) -> PlanBase {
        let schema = self.build_schema();
        PlanBase::new(schema, self.ctx.clone())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        self.arguments.as_ref().map(|x| vec![x.as_ref()]).unwrap_or_default()
    }
}
