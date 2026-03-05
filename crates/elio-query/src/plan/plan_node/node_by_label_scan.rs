use std::sync::Arc;

use educe::Educe;
use elio_common::IrToken;
use elio_common::data_type::LogicalType;
use elio_common::schema::{Schema, Variable};
use itertools::Itertools;
use pretty_xmlish::{Pretty, XmlNode};

use super::*;
use crate::plan::plan_context::PlanContext;
use crate::plan::plan_node::plan_base::PlanBase;

/// NodeByLabelScan scans nodes by label using a label index
#[derive(Debug, Clone)]
pub struct NodeByLabelScan {
    pub base: PlanBase,
    pub(crate) inner: NodeByLabelScanInner,
}

impl NodeByLabelScan {
    pub fn new(inner: NodeByLabelScanInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for NodeByLabelScan {
    type Inner = NodeByLabelScanInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let mut fields = vec![
            ("variable", Pretty::from(self.inner.variable.as_ref())),
            ("label", Pretty::from(self.inner.label.to_string())),
        ];
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
        XmlNode::simple_record("NodeByLabelScan", fields, Default::default())
    }
}

#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct NodeByLabelScanInner {
    pub variable: VariableName,
    pub label: IrToken,
    pub arguments: Option<Box<PlanExpr>>, // must be argument
    #[educe(Debug(ignore))]
    pub ctx: Arc<PlanContext>,
}

impl NodeByLabelScanInner {
    pub fn new(
        variable: VariableName,
        label: IrToken,
        arguments: Option<Box<PlanExpr>>,
        ctx: Arc<PlanContext>,
    ) -> Self {
        if let Some(ref input) = arguments {
            assert!(input.as_argument().is_some());
        }
        Self {
            variable,
            label,
            arguments,
            ctx,
        }
    }

    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::empty();
        schema.fields.push(Variable {
            name: self.variable.clone(),
            typ: LogicalType::VIRTUAL_NODE,
        });
        let optional_arguments = self.arguments.as_ref().map(|x| x.schema()).unwrap_or_default();
        schema.fields.extend(optional_arguments.iter().cloned());
        schema.into()
    }
}

impl InnerNode for NodeByLabelScanInner {
    fn build_base(&self) -> PlanBase {
        let schema = self.build_schema();
        PlanBase::new(schema, self.ctx.clone())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        self.arguments.as_ref().map(|x| vec![x.as_ref()]).unwrap_or_default()
    }
}
