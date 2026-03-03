use std::collections::VecDeque;

use elio_common::data_type::LogicalType;
use elio_common::schema::Variable;
use elio_common::variable::VariableName;
use indexmap::IndexSet;
use pretty_xmlish::{Pretty, XmlNode};

use crate::binder::pattern::PathPatternWithExtra;
use crate::plan::expr::FilterExprs;
use crate::plan::ir::node_connection::{NodeConnection, RelPattern};
use crate::plan::ir::path_pattern::{PathPattern, SelectivePathPattern, SingleNode};
use crate::plan::ir::query::Bindings;
use crate::plan::pretty_utils::pretty_display_iter;

#[derive(Default)]
pub struct QueryGraph {
    pub input_bindings: Bindings,
    // node patterns
    pub nodes: IndexSet<VariableName>,
    // node connections
    pub rels: IndexSet<RelPattern>,
    // selective path patterns
    selective_paths: Vec<SelectivePathPattern>,
    // predicate, i.e. post filter
    pub filter: FilterExprs,
    // outer referenced variables, this is used in the context of subquery
    // outer: IndexSet<VariableName>,
    // path projection
}

impl QueryGraph {
    pub fn new(input_bindings: Bindings) -> Self {
        Self {
            input_bindings,
            nodes: IndexSet::new(),
            rels: IndexSet::new(),
            selective_paths: vec![],
            filter: FilterExprs::empty(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.rels.is_empty() && self.selective_paths.is_empty() && self.filter.is_true()
    }

    pub fn with_input_bindings(&mut self, input_bindings: Bindings) {
        self.input_bindings = input_bindings;
    }

    pub fn add_path_pattern(&mut self, path: &PathPatternWithExtra) {
        let PathPatternWithExtra { pattern, extra } = path;
        match pattern {
            PathPattern::SingleNode(SingleNode { variable: node }) => self.add_node(node),
            PathPattern::Chain(node_connections) => node_connections.connections.iter().for_each(|nc| {
                self.add_node_connection(nc);
            }),
            PathPattern::Selective(selective_path_pattern) => {
                self.add_selective_path(selective_path_pattern);
            }
        }
        // add extra
        // self.outer.extend(extra.outer.clone());
        self.filter = self.filter.clone().and(extra.post_filter.clone());
    }

    pub fn add_node(&mut self, node: &VariableName) {
        self.nodes.insert(node.clone());
    }

    // add relationship endpoint nodes and the relationship itself
    pub fn add_rel(&mut self, rel: &RelPattern) {
        rel.endpoint_nodes().iter().for_each(|x| self.add_node(x));
        self.rels.insert(rel.clone());
    }

    pub fn add_selective_path(&mut self, spp: &SelectivePathPattern) {
        spp.endpoint_nodes().iter().for_each(|x| self.add_node(x));
        self.selective_paths.push(spp.clone());
    }

    pub fn add_node_connection(&mut self, conn: &NodeConnection) {
        match conn {
            NodeConnection::RelPattern(rel_pattern) => self.add_rel(rel_pattern),
            NodeConnection::QuantifiedPathPattern(_quantified_path_pattern) => {
                unreachable!("not supported")
            }
        }
    }

    pub fn merge(&mut self, other: QueryGraph) {
        other.nodes.iter().for_each(|n| self.add_node(n));
        other.rels.iter().for_each(|r| self.add_rel(r));
        other
            .selective_paths
            .iter()
            .for_each(|spp| self.add_selective_path(spp));
        self.filter = self.filter.clone().and(other.filter.clone());
        // other.outer.iter().for_each(|v| {
        // self.outer.insert(v.clone());
        // });
    }

    pub fn add_filter(&mut self, filter: FilterExprs) {
        self.filter = self.filter.clone().and(filter);
    }
}

impl QueryGraph {
    // input binding table
    pub fn input_bindings(&self) -> &Bindings {
        &self.input_bindings
    }

    pub fn input_binding_names(&self) -> IndexSet<VariableName> {
        self.input_bindings.iter().map(|v| v.name.clone()).collect()
    }

    // output binding table
    // TODO(pgao):this part can be freezed in QueryGraph, once the qg is finalized.
    pub fn output_bindings(&self) -> Bindings {
        let mut output_bidings = self.input_bindings.clone();
        for node in self.nodes.iter() {
            output_bidings.insert(Variable::new(node, &LogicalType::VIRTUAL_NODE));
        }
        for rel in self.rels.iter() {
            output_bidings.insert(Variable::new(&rel.variable, &LogicalType::REL));
        }
        output_bidings
    }

    // TODO(pgao): same as output_bindings
    pub fn output_binding_names(&self) -> IndexSet<VariableName> {
        self.output_bindings().iter().map(|v| v.name.clone()).collect()
    }

    pub fn used_variables(&self) -> IndexSet<Variable> {
        let mut vars = IndexSet::new();
        // match pattern
        for var in self.nodes.iter() {
            vars.insert(Variable::new(var, &LogicalType::VIRTUAL_NODE));
        }
        for rel in self.rels.iter() {
            vars.insert(Variable::new(&rel.variable, &LogicalType::REL));
        }
        // filter
        for e in self.filter.iter() {
            vars.extend(e.collect_variables());
        }
        vars
    }

    pub fn contains_node_connection(&self, nc: &NodeConnection) -> bool {
        match nc {
            NodeConnection::RelPattern(rel_pattern) => self.rels.contains(rel_pattern),
            NodeConnection::QuantifiedPathPattern(_quantified_path_pattern) => {
                unreachable!("not supported")
            }
        }
    }

    // partition the filter by input only filter and non-input only filter
    pub fn partition_filter_by_input_only(&self) -> (FilterExprs, FilterExprs) {
        let inputs = self.input_binding_names();
        let (inputs_only, non_inputs_only) = self.filter.clone().partition_by(|e| e.depend_only_on(&inputs));
        (inputs_only, non_inputs_only)
    }
}

impl QueryGraph {
    // partition the query graph by connected component
    // also partition the filter into component if it only depends on the solved variables.
    pub fn connected_component(&self) -> (Vec<QueryGraph>, FilterExprs) {
        let (input_only_filter, mut other_filter) = self.partition_filter_by_input_only();
        let mut visited = IndexSet::new();
        let mut components = vec![];

        // solve input_binding first
        if !self.input_bindings.is_empty() {
            // SAFETY: if input_bindings is empty, then input_only_filter will be empty
            // get qg
            // input_only_filter and other filters may be solved by qg
            let input_binding = self.input_bindings.first().unwrap();
            let mut qg = self.component_for_node(&input_binding.name, &mut visited);
            if !qg.rels.is_empty() {
                // if there's no relaltionships, which means this is an empty qg, we do nothing
                // since the planner will handle the case.
                qg.with_input_bindings(self.input_bindings.clone());
                qg.add_filter(input_only_filter);
                let qg_vars = qg.output_binding_names();
                let (solved, remaining): (Vec<_>, Vec<_>) =
                    other_filter.into_iter().partition(|e| e.depend_only_on(&qg_vars));
                other_filter = FilterExprs::from_iter(remaining);
                qg.add_filter(FilterExprs::from_iter(solved));
                components.push(qg);
            }
        }

        // solve the rest
        for node in self.nodes.iter() {
            if visited.contains(node) {
                continue;
            }
            let mut qg = self.component_for_node(node, &mut visited);
            qg.with_input_bindings(self.input_bindings.clone());
            let qg_vars = qg.output_binding_names();
            let (solved, remaining) = std::mem::take(&mut other_filter).partition_by(|e| e.depend_only_on(&qg_vars));
            other_filter = remaining;
            qg.add_filter(solved);
            components.push(qg);
        }

        // other_filter may not be empty for the case that the filter connects the components
        // which will be solved by CrossProduct+Filter or HashJoin

        (components, other_filter)
    }

    // find the connected component for the given node, and also populate visited
    fn component_for_node(&self, node: &VariableName, visited: &mut IndexSet<VariableName>) -> QueryGraph {
        assert!(!visited.contains(node));
        let mut qg = QueryGraph::new(Bindings::default());
        let mut to_visit = VecDeque::new();
        to_visit.push_back(node.clone());
        let input_bindings = self.input_bindings();
        let input_binding_names = self.input_binding_names();

        while let Some(node) = to_visit.pop_front() {
            if visited.contains(&node) {
                continue;
            }
            visited.insert(node.clone());
            let (ncs, nbrs) = self.connected_entities(&node);
            qg.add_node(&node);
            for nc in ncs {
                qg.add_node_connection(&nc);
            }
            for nb in nbrs {
                qg.add_node(&nb);
                to_visit.push_back(nb.clone());
                // handle inputs, which considered virtual node connections.
                // first of all, all input variables are connected
                // and in the following cases, we consider node and input variables connected
                // - qg contains node/rel works as input variables in original one
                // - in original qg's filter, (any of input variable) and current node act as input to filter expr
                // in either case, we should add inputs to qg
                if qg.input_bindings.is_empty() && qg.output_bindings().intersection(input_bindings).next().is_some()
                    || self.filter.iter().any(|e| {
                        let used_vars: IndexSet<VariableName> =
                            e.collect_variables().into_iter().map(|x| x.name.clone()).collect();
                        used_vars.contains(&nb) && used_vars.intersection(&input_binding_names).next().is_some()
                    })
                {
                    qg.with_input_bindings(input_bindings.clone());
                    // add node solved by inputs to to_visit
                    let solved_nodes: IndexSet<_> = self.nodes.intersection(&input_binding_names).collect();
                    solved_nodes.into_iter().for_each(|i| to_visit.push_back(i.clone()));
                }
            }
        }
        qg
    }

    // find the reachable entities(node connection and nodes) from the given node
    fn connected_entities(&self, node: &VariableName) -> (IndexSet<NodeConnection>, IndexSet<VariableName>) {
        // connected by rel
        let mut ncs = IndexSet::new();
        let mut nodes = IndexSet::new();
        for rel in self.rels.iter() {
            if rel.endpoint_nodes().contains(&node) {
                ncs.insert(NodeConnection::RelPattern(rel.clone()));
                nodes.insert(rel.other_node(node).clone());
            }
        }
        // TODO(pgao): maybe we should move connected by filter condition of inputs here?
        (ncs, nodes)
    }

    // one hop node connections
    pub fn connections(&self, node: &VariableName) -> impl DoubleEndedIterator<Item = &RelPattern> {
        self.rels
            .iter()
            .filter(move |rel| rel.endpoints.0 == *node || rel.endpoints.1 == *node)
    }
}

impl QueryGraph {
    pub fn xmlnode(&self) -> XmlNode<'_> {
        let mut fields = vec![];
        if !self.input_bindings.is_empty() {
            fields.push((
                "input_bindings",
                pretty_display_iter(self.input_bindings.iter().map(|x| x.name.clone())),
            ));
        };
        if !self.nodes.is_empty() {
            fields.push(("nodes", pretty_display_iter(self.nodes.iter())));
        };
        if !self.rels.is_empty() {
            fields.push(("rels", pretty_display_iter(self.rels.iter())));
        };
        if !self.filter.is_true() {
            fields.push(("filter", Pretty::display(&self.filter.pretty())));
        };
        XmlNode::simple_record("QueryGraph", fields, Default::default())
    }
}
