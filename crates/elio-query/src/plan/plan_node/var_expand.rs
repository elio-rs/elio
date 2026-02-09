use std::sync::Arc;

use super::*;
use crate::plan::expr::FilterExprs;
use crate::plan::ir::node_connection::RelPattern;

#[derive(Debug, Clone)]
pub struct VarExpand {
    pub base: PlanBase,
    pub(crate) inner: VarExpandInner,
}

impl VarExpand {
    pub fn new(inner: VarExpandInner) -> Self {
        Self {
            base: inner.build_base(),
            inner,
        }
    }
}

impl PlanNode for VarExpand {
    type Inner = VarExpandInner;

    crate::impl_plan_inner!();

    fn xmlnode(&self) -> XmlNode<'_> {
        let mut fields = vec![
            ("from", Pretty::from(self.inner.from.as_ref())),
            ("to", Pretty::from(self.inner.to.as_ref())),
            ("rel_pattern", Pretty::display(&self.inner.rel_pattern)),
            ("path_mode", Pretty::display(&self.inner.path_mode)),
        ];
        if !self.inner.node_filter.is_true() {
            fields.push(("node_filter", Pretty::from(self.inner.node_filter.pretty())));
        }
        if !self.inner.rel_filter.is_true() {
            fields.push(("rel_filter", Pretty::from(self.inner.rel_filter.pretty())));
        }
        let name = match self.inner.kind {
            ExpandKind::All => "VarExpandAll",
            ExpandKind::Into => "VarExpandInto",
        };
        let children = vec![Pretty::Record(self.inner.input.xmlnode())];
        XmlNode::simple_record(name, fields, children)
    }
}

// (a)-[r*1..3]->(b)
// Expand all: given a/b, produce r and b/a, where r is an list of rels
// Expand into: given a and b, produce r, where r is an list of rels

#[derive(Debug, Clone)]
pub struct VarExpandInner {
    pub input: Box<PlanExpr>,
    pub from: VariableName,
    pub to: VariableName,
    pub rel_pattern: RelPattern,
    pub node_filter: FilterExprs,
    pub rel_filter: FilterExprs,
    pub kind: ExpandKind,
    pub path_mode: PathMode,
}

impl InnerNode for VarExpandInner {
    fn inputs(&self) -> Vec<&PlanExpr> {
        vec![&self.input]
    }

    // direction output schema = [input, rel, to]
    fn build_base(&self) -> PlanBase {
        let mut schema = Schema::from_arc(self.input.schema());
        schema.add_column(Variable::new(
            &self.rel_pattern.variable,
            &DataType::new_list(DataType::Rel),
        ));
        if matches!(self.kind, ExpandKind::All) {
            schema.add_column(Variable::new(&self.to, &DataType::VirtualNode));
        }
        PlanBase::new(Arc::new(schema), self.input.ctx())
    }
}
