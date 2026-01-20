use std::collections::VecDeque;

use elio_common::data_type::DataType;
use elio_common::schema::Variable;
use elio_common::variable::VariableName;
use indexmap::IndexSet;
use itertools::Itertools;
use pretty_xmlish::{Pretty, XmlNode};

use crate::binder::pattern::PathPatternWithExtra;
use crate::expr::FilterExprs;
use crate::ir::mutating_pattern::{CreatePattern, MutatingPattern};
use crate::ir::node_connection::{ExhaustiveNodeConnection, RelPattern};
use crate::ir::path_pattern::{PathPattern, SelectivePathPattern, SingleNode};
use crate::pretty_utils::pretty_display_iter;

#[derive(Default)]
pub struct QueryGraph {
    // node patterns
    pub nodes: IndexSet<VariableName>,
    // node connections
    pub rels: IndexSet<RelPattern>,
    // selective path patterns
    selective_paths: Vec<SelectivePathPattern>,
    // predicate, i.e. post filter
    pub filter: FilterExprs,
    // optional matches
    pub optional_matches: Vec<QueryGraph>,
    // mutating patterns
    pub mutating_patterns: Vec<MutatingPattern>,
    // imported variables as query graph inputs
    // imported may contain node/rels that does not exists in current qg's nodes and resl
    // TODO(pgao): just use variable name?
    // when the datatype is needed?
    imported: IndexSet<Variable>,
    // outer referenced variables, this is used in the context of subquery
    outer: IndexSet<VariableName>,
    // path projection
}

impl QueryGraph {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn add_path_pattern(&mut self, path: &PathPatternWithExtra) {
        let PathPatternWithExtra { pattern, extra } = path;
        match pattern {
            PathPattern::SingleNode(SingleNode { variable: node }) => self.add_node(node),
            PathPattern::NodeConnections(node_connections) => node_connections.connections.iter().for_each(|nc| {
                self.add_node_connection(nc);
            }),
            PathPattern::SelectivePathPattern(selective_path_pattern) => {
                self.add_selective_path(selective_path_pattern);
            }
        }
        // add extra
        self.outer.extend(extra.outer.clone());
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

    pub fn add_node_connection(&mut self, conn: &ExhaustiveNodeConnection) {
        match conn {
            ExhaustiveNodeConnection::RelPattern(rel_pattern) => self.add_rel(rel_pattern),
            ExhaustiveNodeConnection::QuantifiedPathPattern(_quantified_path_pattern) => {
                unreachable!("not supported")
            }
        }
    }

    pub fn add_optional_qg(&mut self, qg: QueryGraph) {
        self.optional_matches.push(qg);
    }

    pub fn add_imported(&mut self, var: &Variable) {
        self.imported.insert(var.clone());
    }

    pub fn add_imported_set(&mut self, vars: &IndexSet<Variable>) {
        self.imported.extend(vars.clone());
    }

    pub fn merge(&mut self, other: QueryGraph) {
        other.nodes.iter().for_each(|n| self.add_node(n));
        other.rels.iter().for_each(|r| self.add_rel(r));
        other
            .selective_paths
            .iter()
            .for_each(|spp| self.add_selective_path(spp));
        self.filter = self.filter.clone().and(other.filter.clone());
        self.optional_matches.extend(other.optional_matches);
        self.mutating_patterns.extend(other.mutating_patterns);
        other.imported.iter().for_each(|v| {
            self.imported.insert(v.clone());
        });
        other.outer.iter().for_each(|v| {
            self.outer.insert(v.clone());
        });
    }

    pub fn add_filter(&mut self, filter: FilterExprs) {
        self.filter = self.filter.clone().and(filter);
    }

    pub fn add_create_pattern(&mut self, c: CreatePattern) {
        self.mutating_patterns.push(MutatingPattern::Create(c));
    }
}

impl QueryGraph {
    pub fn imported_variables(&self) -> IndexSet<VariableName> {
        self.imported.iter().map(|v| v.name.clone()).collect()
    }

    pub fn imported(&self) -> &IndexSet<Variable> {
        &self.imported
    }

    pub fn outputs(&self) -> IndexSet<Variable> {
        let mut vars = IndexSet::new();
        self.imported.iter().for_each(|v| {
            vars.insert(v.clone());
        });
        self.nodes.iter().for_each(|n| {
            vars.insert(Variable::new(n, &DataType::VirtualNode));
        });
        self.rels.iter().for_each(|v| {
            vars.insert(Variable::new(&v.variable, &DataType::Rel));
        });
        vars
    }

    // used for planing pattern without optional and update
    pub fn match_pattern_variables(&self) -> IndexSet<VariableName> {
        let mut vars = IndexSet::default();
        self.nodes.iter().for_each(|n| {
            vars.insert(n.clone());
        });
        self.rels.iter().for_each(|r| {
            r.path_elements().iter().for_each(|e| {
                vars.insert(e.variable().clone());
            })
        });
        // including imported variables
        vars.extend(self.imported.iter().map(|v| v.name.clone()));
        vars
    }

    pub fn used_variables(&self) -> IndexSet<Variable> {
        let mut vars = IndexSet::new();
        // match pattern
        for var in self.nodes.iter() {
            vars.insert(Variable::new(var, &DataType::VirtualNode));
        }
        for rel in self.rels.iter() {
            vars.insert(Variable::new(&rel.variable, &DataType::Rel));
        }
        // optional match pattern
        for qg in self.optional_matches.iter() {
            vars.extend(qg.used_variables());
        }
        // mutating pattern
        for mp in self.mutating_patterns.iter() {
            vars.extend(mp.used_variables());
        }
        // filter
        for e in self.filter.iter() {
            vars.extend(e.collect_variables());
        }
        vars
    }

    pub fn contains_node_connection(&self, nc: &ExhaustiveNodeConnection) -> bool {
        match nc {
            ExhaustiveNodeConnection::RelPattern(rel_pattern) => self.rels.contains(rel_pattern),
            ExhaustiveNodeConnection::QuantifiedPathPattern(_quantified_path_pattern) => {
                unreachable!("not supported")
            }
        }
    }

    // partition the filter by imported only filter and non-imported only filter
    pub fn partition_filter_by_imported_only(&self) -> (FilterExprs, FilterExprs) {
        let imported = self.imported_variables();
        let (imported_only, non_imported_only) = self.filter.clone().partition_by(|e| e.depend_only_on(&imported));
        (imported_only, non_imported_only)
    }
}

impl QueryGraph {
    // partition the query graph by connected component
    // also partition the filter into component if it only depends on the solved variables.
    pub fn connected_component(&self) -> (Vec<QueryGraph>, FilterExprs) {
        let (argument_only_filter, mut other_filter) = self.partition_filter_by_imported_only();
        let mut visited = IndexSet::new();
        let mut components = vec![];

        // solve argument first
        if !self.imported.is_empty() {
            // SAFETY: if imported is empty, then argument only filter will be empty
            // get qg
            // argument only filter and other filters may be solved by qg
            let arg = self.imported.first().unwrap();
            let mut qg = self.component_for_node(&arg.name, &mut visited);
            if !qg.rels.is_empty() {
                // if there's no relaltionships, which means this is an empty qg, we do nothing
                // since the planner will handle the case.
                qg.add_imported_set(&self.imported);
                qg.add_filter(argument_only_filter);
                let qg_vars = qg.match_pattern_variables();
                let (solved, remaining): (Vec<_>, Vec<_>) =
                    other_filter.into_iter().partition(|e| e.depend_only_on(&qg_vars));
                other_filter = FilterExprs::from_iter(remaining);
                qg.add_filter(FilterExprs::from_iter(solved));
                components.push(qg);
            }
        }

        // solve rest
        for node in self.nodes.iter() {
            if visited.contains(node) {
                continue;
            }
            let mut qg = self.component_for_node(node, &mut visited);
            qg.add_imported_set(&self.imported);
            let qg_vars = qg.match_pattern_variables();
            let (solved, remaining) = std::mem::take(&mut other_filter).partition_by(|e| e.depend_only_on(&qg_vars));
            other_filter = remaining;
            qg.add_filter(solved);
            components.push(qg);
        }
        // assert!(other_filter.is_true());

        (components, other_filter)
    }

    // find the connected component for the given node, and also populate visited
    fn component_for_node(&self, node: &VariableName, visited: &mut IndexSet<VariableName>) -> QueryGraph {
        assert!(!visited.contains(node));
        let mut qg = QueryGraph::empty();
        let mut to_visit = VecDeque::new();
        to_visit.push_back(node.clone());
        let imported = self.imported_variables();

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
                // handle argument, which considered virtual node connections.
                // first of all, all arguments are connected
                // and in the following cases, we consider node and imported variables connected
                // - qg contains node/rel works as imported variables in original one
                // - in original qg's filter, (any of imported variable) and current node act as input to filter expr
                // in either case, we should add argument to qg
                if qg.imported.is_empty() && qg.match_pattern_variables().intersection(&imported).next().is_some()
                    || self.filter.iter().any(|e| {
                        let used_vars: IndexSet<VariableName> =
                            e.collect_variables().into_iter().map(|x| x.name.clone()).collect();
                        used_vars.contains(&nb) && used_vars.intersection(&imported).next().is_some()
                    })
                {
                    qg.add_imported_set(&self.imported);
                    // add node solved by argument to to_visit
                    let solved_nodes: IndexSet<_> = self.nodes.intersection(&imported).collect();
                    solved_nodes.into_iter().for_each(|i| to_visit.push_back(i.clone()));
                }
            }
        }
        qg
    }

    // find the reachable entities(node connection and nodes) from the given node
    fn connected_entities(&self, node: &VariableName) -> (IndexSet<ExhaustiveNodeConnection>, IndexSet<VariableName>) {
        // connected by rel
        let mut ncs = IndexSet::new();
        let mut nodes = IndexSet::new();
        for rel in self.rels.iter() {
            if rel.endpoint_nodes().contains(&node) {
                ncs.insert(ExhaustiveNodeConnection::RelPattern(rel.clone()));
                nodes.insert(rel.other_node(node).clone());
            }
        }
        // TODO(pgao): maybe we should move connected by filter condition of arguments here?
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
        if !self.imported.is_empty() {
            fields.push((
                "imported",
                pretty_display_iter(self.imported.iter().map(|x| x.name.clone())),
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
        let mut children = vec![];
        if !self.optional_matches.is_empty() {
            let optional_matches = self
                .optional_matches
                .iter()
                .map(|x| Pretty::Record(x.xmlnode()))
                .collect_vec();
            children.push(Pretty::simple_record("optional_matches", vec![], optional_matches));
        };
        if !self.mutating_patterns.is_empty() {
            let mutating_patterns = self
                .mutating_patterns
                .iter()
                .map(|x| Pretty::Record(x.xmlnode()))
                .collect_vec();
            children.push(Pretty::simple_record("mutating_pattern", vec![], mutating_patterns));
        };

        XmlNode::simple_record("QueryGraph", fields, children)
    }
}
