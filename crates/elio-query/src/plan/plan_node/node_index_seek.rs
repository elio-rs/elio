//! NodeIndexSeek - Index-based node lookup
//!
//! This plan node uses a unique index to directly lookup nodes
//! instead of scanning all nodes and filtering.

use std::sync::Arc;

use educe::Educe;
use elio_common::ResolvedIrToken;
use elio_common::schema::{Schema, Variable};
use itertools::Itertools;
use pretty_xmlish::{Pretty, XmlNode};

use super::*;
use crate::plan::expr::Expr;
use crate::plan::plan_context::PlanContext;
use crate::plan::plan_node::plan_base::PlanBase;

/// NodeIndexSeek uses a unique index to lookup nodes by property values
#[derive(Clone)]
pub struct NodeIndexSeek {
    pub base: PlanBase,
    pub(crate) inner: NodeIndexSeekInner,
}

impl std::fmt::Debug for NodeIndexSeek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeIndexSeek")
            .field("base", &self.base)
            .field("inner", &self.inner)
            .finish()
    }
}

impl NodeIndexSeek {
    pub fn new(inner: NodeIndexSeekInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for NodeIndexSeek {
    type Inner = NodeIndexSeekInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let mut fields = vec![
            ("variable", Pretty::from(self.inner.variable.name.as_ref())),
            ("label", Pretty::from(self.inner.label.to_string())),
            ("constraint", Pretty::from(self.inner.constraint_name.as_ref())),
        ];

        let props = self
            .inner
            .prop_tokens
            .iter()
            .zip(self.inner.prop_values.iter())
            .map(|(name, val)| format!("{} = {}", name, val.pretty()))
            .collect_vec();
        fields.push((
            "properties",
            Pretty::Array(props.into_iter().map(Pretty::from).collect_vec()),
        ));

        XmlNode::simple_record("NodeIndexSeek", fields, Default::default())
    }
}

#[derive(Educe)]
#[educe(Clone, Debug)]
pub struct NodeIndexSeekInner {
    /// Output variable name for the node
    pub variable: Variable,
    pub label: ResolvedIrToken,
    pub constraint_name: Arc<str>,
    pub prop_tokens: Vec<ResolvedIrToken>,
    pub prop_values: Vec<Expr>,
    #[educe(Debug(ignore))]
    pub ctx: Arc<PlanContext>,
    pub arguments: Option<Box<PlanExpr>>, // must be argument
}

impl NodeIndexSeekInner {
    pub fn new(
        variable: Variable,
        label: ResolvedIrToken,
        constraint_name: Arc<str>,
        prop_tokens: Vec<ResolvedIrToken>,
        prop_values: Vec<Expr>,
        ctx: Arc<PlanContext>,
        arguments: Option<Box<PlanExpr>>,
    ) -> Self {
        if let Some(ref arguments) = arguments {
            assert!(
                arguments.as_argument().is_some(),
                "NodeIndexSeek input must be argument"
            );
        }
        Self {
            variable,
            label,
            constraint_name,
            prop_tokens,
            prop_values,
            ctx,
            arguments,
        }
    }

    fn build_schema(&self) -> Arc<Schema> {
        let mut schema = Schema::empty();
        schema.fields.push(self.variable.clone());
        if let Some(ref arguments) = self.arguments {
            schema.fields.extend(arguments.schema().columns().iter().cloned());
        }
        schema.into()
    }
}

impl InnerNode for NodeIndexSeekInner {
    fn build_base(&self) -> PlanBase {
        let schema = self.build_schema();
        PlanBase::new(schema, self.ctx.clone())
    }

    fn inputs(&self) -> Vec<&PlanExpr> {
        self.arguments.as_ref().map(|x| x.as_ref()).into_iter().collect()
    }
}
