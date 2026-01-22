use educe::{self, Educe};
use elio_common::variable::{PathElement, VariableName};
use elio_common::{IrToken, SemanticDirection};
use indexmap::IndexSet;

use crate::expr::FilterExprs;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct RelPattern {
    pub variable: VariableName,
    pub endpoints: (VariableName, VariableName),
    pub dir: SemanticDirection,
    pub types: Vec<IrToken>,
    pub length: PatternLength,
}

impl RelPattern {
    pub fn endpoint_nodes(&self) -> Vec<&VariableName> {
        vec![&self.endpoints.0, &self.endpoints.1]
    }

    pub fn left(&self) -> &VariableName {
        &self.endpoints.0
    }

    pub fn right(&self) -> &VariableName {
        &self.endpoints.1
    }

    pub fn other_node(&self, node: &VariableName) -> &VariableName {
        if node == self.left() { self.right() } else { self.left() }
    }

    // variables that consturct to the path
    pub fn path_elements(&self) -> Vec<PathElement> {
        vec![
            PathElement::Node(self.left().clone()),
            PathElement::Rel(self.variable.clone()),
            PathElement::Node(self.right().clone()),
        ]
    }
}

impl std::fmt::Display for RelPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (ldir, rdir) = if self.dir == SemanticDirection::Outgoing {
            ("-", "->")
        } else if self.dir == SemanticDirection::Incoming {
            ("<-", "-")
        } else {
            ("<-", "->")
        };
        let var = &self.variable;
        write!(
            f,
            "({lnode}){ldir}[{var}:{length}]{rdir}({rnode})",
            lnode = self.left(),
            rnode = self.right(),
            length = self.length
        )
    }
}

// For Quantified path pattern
// (x) ( (a)--(b)--(c) ){1,3} (y)
// left node biding is inner(a) --> outer(x)
// right node binding is inner(c) --> outer(y)
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct NodeBinding {
    pub inner: VariableName,
    pub outer: VariableName,
}

#[derive(Educe)]
#[educe(Clone, Hash, Eq, PartialEq)]
pub struct QuantifiedPathPattern {
    pub left_binding: NodeBinding,
    pub right_binding: NodeBinding,
    pub rels: Vec<RelPattern>,
    // this filter works when expanding each hop
    // pre-filter
    pub filter: FilterExprs,
    pub repetition: Repetition,
    #[educe(Hash(ignore))]
    pub node_grouping: IndexSet<VariableGrouping>,
    #[educe(Hash(ignore))]
    pub rel_grouping: IndexSet<VariableGrouping>,
}

impl QuantifiedPathPattern {
    pub fn endpoint_nodes(&self) -> Vec<&VariableName> {
        vec![&self.left_binding.outer, &self.right_binding.outer]
    }
}

// Node connection can be a single rel pattern or a quantified path pattern
#[derive(Clone, Hash, PartialEq, Eq)]
pub enum NodeConnection {
    RelPattern(RelPattern),
    QuantifiedPathPattern(QuantifiedPathPattern),
}

impl NodeConnection {
    pub fn endpoint_nodes(&self) -> Vec<&VariableName> {
        match self {
            NodeConnection::RelPattern(rel) => rel.endpoint_nodes(),
            NodeConnection::QuantifiedPathPattern(qp) => qp.endpoint_nodes(),
        }
    }

    pub fn path_elements(&self) -> Vec<PathElement> {
        match self {
            NodeConnection::RelPattern(rel) => rel.path_elements(),
            NodeConnection::QuantifiedPathPattern(_) => todo!("qpp support"),
        }
    }
}

impl From<RelPattern> for NodeConnection {
    fn from(value: RelPattern) -> Self {
        NodeConnection::RelPattern(value)
    }
}

impl From<QuantifiedPathPattern> for NodeConnection {
    fn from(value: QuantifiedPathPattern) -> Self {
        NodeConnection::QuantifiedPathPattern(value)
    }
}

pub enum Selector {
    /// Any k paths
    AnyK(i64),
    // shortest k paths
    ShortestK(i64),
    // TODO(pgao): Shortest K GROUPS
    ShortestKGroup(i64),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, derive_more::Display)]
pub enum PatternLength {
    #[display("")]
    Simple,
    #[display("*{}..{}", min, max.unwrap_or(usize::MAX))]
    Var { min: usize, max: Option<usize> },
}

impl PatternLength {
    pub fn is_simple(&self) -> bool {
        matches!(self, PatternLength::Simple)
    }

    pub fn as_range(&self) -> Option<(usize, Option<usize>)> {
        match self {
            PatternLength::Simple => None,
            PatternLength::Var { min, max } => Some((*min, *max)),
        }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Repetition {
    pub min: i64,
    // None means infinite
    // inclusive
    pub max: Option<i64>,
}
// For Quantified path pattern
// (x) ( (a)--(b)--(c) ){1,3} (y)
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct VariableGrouping {
    pub singleton: VariableName,
    pub group: VariableName,
}
