use elio_common::variable::VariableName;
use indexmap::IndexMap;
use pretty_xmlish::{Pretty, PrettyConfig, XmlNode};

use crate::ir::query_graph::QueryGraph;
use crate::ir::query_project::QueryProjection;

pub struct IrQueryRoot {
    pub inner: IrQuery,
    // mapping from variable name to output names
    // TODO(pgao): should we record the datatype here?
    pub names: IndexMap<String, VariableName>,
}

impl IrQueryRoot {
    pub fn explain(&self) -> String {
        let fields = vec![(
            "names",
            Pretty::Array(self.names.iter().map(|(k, _)| Pretty::display(k)).collect()),
        )];
        let children = vec![Pretty::Record(self.inner.xmlnode())];
        let tree = Pretty::simple_record("RootIR", fields, children);
        let mut config = PrettyConfig {
            indent: 3,
            width: 2048,
            need_boundaries: false,
            reduced_spaces: true,
        };
        let mut output = String::with_capacity(2048);
        config.unicode(&mut output, &tree);
        output
    }
}

pub struct IrQuery {
    pub queries: Vec<IrSingleQuery>,
    pub union_all: bool,
}

impl IrQuery {
    pub fn is_union(&self) -> bool {
        self.queries.len() > 1
    }

    pub fn xmlnode(&self) -> XmlNode<'_> {
        if self.is_union() {
            let fields = vec![
                (
                    "inputs",
                    Pretty::Array(self.queries.iter().map(|q| Pretty::Record(q.xmlnode())).collect()),
                ),
                ("distinct", Pretty::debug(&!self.union_all)),
            ];
            XmlNode::simple_record("UnionQuery", fields, Default::default())
        } else {
            self.queries[0].xmlnode()
        }
    }
}

#[derive(Default)]
pub struct IrSingleQuery {
    pub parts: Vec<IrSingleQueryPart>,
    // pub query_graph: QueryGraph,
    // pub query_project: QueryProjection,
    // pub tail: Option<Box<IrSingleQuery>>,
    // TODO(pgao): the interesting_order may be derived here.
    // pub interesting_order: OrderingChoice,
}

impl IrSingleQuery {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn xmlnode(&self) -> XmlNode<'_> {
        assert!(!self.parts.is_empty());
        let mut iter = self.parts.iter();
        let mut root = iter.next().unwrap().xmlnode();

        for nxt in iter {
            let mut node = nxt.xmlnode();
            node.children.push(Pretty::Record(root));
            root = node;
        }
        root
    }
}

#[derive(Default)]
pub struct IrSingleQueryPart {
    pub query_graph: QueryGraph,
    // for update and create clause, there may be no projection at the end
    // in this case, planner should generate an Empty PlanNode.
    pub query_project: Option<QueryProjection>,
}

impl IrSingleQueryPart {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn with_projection(&mut self, proj: QueryProjection) {
        self.query_project = Some(proj);
    }

    pub fn update_projection<F>(&mut self, f: F)
    where
        F: FnOnce(&mut QueryProjection),
    {
        if let Some(proj) = &mut self.query_project {
            f(proj);
        }
    }

    pub fn xmlnode(&self) -> XmlNode<'_> {
        let mut children = vec![];
        children.push(Pretty::Record(self.query_graph.xmlnode()));
        if let Some(proj) = &self.query_project {
            children.push(Pretty::Record(proj.xmlnode()));
        }
        XmlNode::simple_record("IrSingleQueryPart", vec![], children)
    }
}
