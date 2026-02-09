use elio_common::variable::VariableName;

use crate::expr::FilterExprs;
use crate::ir::node_connection::{NodeConnection, RelPattern};

pub enum PathPattern {
    SingleNode(SingleNode),
    Chain(ChainPattern),
    Selective(SelectivePathPattern),
}

impl PathPattern {
    pub fn as_node_connections(self) -> Option<ChainPattern> {
        match self {
            PathPattern::Chain(node_connections) => Some(node_connections),
            _ => None,
        }
    }
}

// length 0 path pattern of single node
pub struct SingleNode {
    pub variable: VariableName,
}

// length 1 or more path pattern made of node connections
#[derive(Clone)]
pub struct ChainPattern {
    pub connections: Vec<NodeConnection>,
}

impl ChainPattern {
    pub fn endpoint_nodes(&self) -> Vec<&VariableName> {
        let mut nodes = Vec::new();
        for conn in &self.connections {
            nodes.extend(conn.endpoint_nodes());
        }
        nodes
    }
}

impl ChainPattern {
    pub fn as_rels(self) -> Option<Vec<RelPattern>> {
        let mut rels = Vec::new();
        for conn in self.connections {
            match conn {
                NodeConnection::RelPattern(rel) => rels.push(rel),
                _ => return None,
            }
        }
        Some(rels)
    }
}

#[derive(Clone)]
pub struct SelectivePathPattern {
    path_pattern: ChainPattern,
    _filter: FilterExprs,
    _selector: Selector,
}

impl SelectivePathPattern {
    pub fn endpoint_nodes(&self) -> Vec<&VariableName> {
        self.path_pattern.endpoint_nodes()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum Selector {
    /// Any k paths
    AnyK(i64),
    // shortest k paths
    ShortestK(i64),
    // TODO(pgao): Shortest K GROUPS
    ShortestKGroup(i64),
}
