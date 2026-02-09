use elio_common::schema::Variable;
use elio_common::variable::VariableName;
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pretty_xmlish::{Pretty, PrettyConfig, XmlNode};

use crate::ir::MutatingPattern;
use crate::ir::query_graph::QueryGraph;
use crate::ir::query_project::QueryProjection;

// binding table variables
pub type Bindings = IndexSet<Variable>;

pub struct IrQueryRoot {
    pub inner: IrQuery,
    // mapping from variable name to output names
    pub output_names: IndexMap<String, VariableName>,
}

impl IrQueryRoot {
    pub fn explain(&self) -> String {
        let fields = vec![(
            "names",
            Pretty::Array(self.output_names.iter().map(|(k, _)| Pretty::display(k)).collect()),
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

// #[derive(Default)]
pub struct IrSingleQueryPart {
    pub input_binding: Bindings,
    pub match_pattern: Option<QueryGraph>,
    pub optional_match_patterns: Vec<QueryGraph>,
    pub mutating_patterns: Vec<MutatingPattern>,
    // for update and create clause, there may be no projection at the end
    // in this case, palnner should generate an Empty PlanNode.
    pub projection: Option<QueryProjection>,
}

impl IrSingleQueryPart {
    pub fn new(input_binding: Bindings) -> Self {
        Self {
            input_binding,
            match_pattern: None,
            optional_match_patterns: vec![],
            mutating_patterns: vec![],
            projection: None,
        }
    }

    // note: qg should handle the input_bindgs by itself
    pub fn with_match_pattern(&mut self, qg: QueryGraph) {
        self.match_pattern = Some(qg);
    }

    pub fn add_match_pattern(&mut self, qg: QueryGraph) {
        self.match_pattern = if let Some(existing) = self.match_pattern.take() {
            let mut existing = existing;
            existing.merge(qg);
            Some(existing)
        } else {
            Some(qg)
        };
    }

    // note: qg should handle the input_bindgs by itself
    pub fn add_optional_match_pattern(&mut self, qg: QueryGraph) {
        self.optional_match_patterns.push(qg);
    }

    // note: mp should handle the input_bindgs by itself
    pub fn add_mutating_pattern(&mut self, mp: MutatingPattern) {
        self.mutating_patterns.push(mp);
    }

    // note: proj should handle the input_bindgs by itself
    pub fn with_projection(&mut self, proj: QueryProjection) {
        self.projection = Some(proj);
    }

    #[inline]
    pub fn has_reading_pattern(&self) -> bool {
        self.match_pattern.is_some() || !self.optional_match_patterns.is_empty()
    }

    #[inline]
    pub fn has_mutating_pattern(&self) -> bool {
        !self.mutating_patterns.is_empty()
    }

    pub fn input_bindings(&self) -> &Bindings {
        &self.input_binding
    }

    pub fn update_projection<F>(&mut self, f: F)
    where
        F: FnOnce(&mut QueryProjection),
    {
        if let Some(proj) = &mut self.projection {
            f(proj);
        }
    }

    pub fn xmlnode(&self) -> XmlNode<'_> {
        let mut children = vec![];

        fn named_children<'a>(name: &'static str, children: Vec<Pretty<'a>>) -> Pretty<'a> {
            Pretty::simple_record(name, Default::default(), children)
        }
        // match pattern
        if let Some(qg) = &self.match_pattern {
            children.push(named_children("match_pattern", vec![Pretty::Record(qg.xmlnode())]));
        }

        // optional match patterns
        if !self.optional_match_patterns.is_empty() {
            children.push(named_children(
                "optional_match_patterns",
                self.optional_match_patterns
                    .iter()
                    .map(|qg| Pretty::Record(qg.xmlnode()))
                    .collect_vec(),
            ));
        }

        // mutating patterns
        if !self.mutating_patterns.is_empty() {
            children.push(named_children(
                "mutating_patterns",
                self.mutating_patterns
                    .iter()
                    .map(|mp| Pretty::Record(mp.xmlnode()))
                    .collect_vec(),
            ));
        }

        // projections
        if let Some(proj) = &self.projection {
            children.push(named_children("projection", vec![Pretty::Record(proj.xmlnode())]));
        }
        XmlNode::simple_record("IrSingleQueryPart", Default::default(), children)
    }
}
