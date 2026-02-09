use std::ops::Range;

use derive_more::Display;
use elio_common::SemanticDirection;
use itertools::{self, Itertools};

use crate::ast::{Expr, LabelExpr};

#[derive(Debug)]
pub struct MatchPattern {
    pub patterns: Vec<PatternPart>,
}

impl std::fmt::Display for MatchPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.patterns.iter().map(|x| x.to_string()).join(", "))
    }
}

#[derive(Debug)]
pub struct UpdatePattern {
    pub patterns: Vec<PatternPart>,
}

impl std::fmt::Display for UpdatePattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.patterns.iter().map(|x| x.to_string()).join(", "))
    }
}

#[derive(Default, Debug, Display, PartialEq, Eq)]
#[display("{}", _0)]
pub enum Selector {
    #[default]
    #[display("")]
    AllPaths, // ALL PATHS
    #[display("ANY {} PATHS", _0)]
    AnyPath(u32), // ANY <count> PATHS
    #[display("ALL SHORTEST PATHS")]
    AllShortest, // ALL SHORTEST PATHS
    #[display("ANY SHORTEST PATHS")]
    AnyShortestPath, // ANY SHORTEST PATHS
    #[display("SHORTEST {} PATHS", _0)]
    CountedShortestPath(u32),
    #[display("SHORTEST {} PATH GROUPS", _0)]
    CountedShortestGroup(u32),
}

impl Selector {
    pub fn is_selective(&self) -> bool {
        !matches!(self, Self::AllPaths)
    }

    pub fn is_counted(&self) -> bool {
        matches!(
            self,
            Self::AnyPath(_) | Self::CountedShortestPath(_) | Self::CountedShortestGroup(_)
        )
    }
}

#[derive(Debug)]
pub struct PatternPart {
    pub variable: Option<String>, // pattern part with name
    pub selector: Selector,
    pub factors: Vec<PathFactor>, // must start with simple and ends with simple
}

impl PatternPart {
    pub fn is_anonymous(&self) -> bool {
        self.variable.is_none()
    }

    pub fn contains_qpp(&self) -> bool {
        self.factors.iter().any(|x| x.is_qpp())
    }

    pub fn simple_patterns(&self) -> Vec<&SimplePathPattern> {
        self.factors.iter().filter_map(|x| x.as_simple_pattern()).collect()
    }

    pub fn as_simple_patterns(&self) -> Option<&SimplePathPattern> {
        if self.contains_qpp() {
            None
        } else {
            // if does not contain qpp, the part should exactly contain only one simple pattern
            self.simple_patterns().first().copied()
        }
    }
}

impl std::fmt::Display for PatternPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = self.variable.as_ref() {
            write!(f, "{name} = ")?;
        }
        if self.selector.is_selective() {
            write!(f, "{} ", self.selector)?;
        }
        write!(
            f,
            "{}",
            self.factors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("")
        )?;
        Ok(())
    }
}

#[derive(Debug, Display)]
#[display("{}", _0)]
pub enum PathFactor {
    #[display("{}", _0)]
    Simple(SimplePathPattern),
    #[display("{}", _0)]
    Quantified(QuantifiedPathPattern),
}

impl PathFactor {
    pub fn is_qpp(&self) -> bool {
        matches!(self, Self::Quantified(_))
    }

    pub fn as_simple_pattern(&self) -> Option<&SimplePathPattern> {
        match self {
            Self::Simple(p) => Some(p),
            Self::Quantified(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimplePathPattern {
    pub nodes: Vec<NodePattern>,
    pub relationships: Vec<RelationshipPattern>,
}

impl std::fmt::Display for SimplePathPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.nodes.iter().map(|x| x.to_string());
        let e = self.relationships.iter().map(|x| x.to_string());
        for x in n.interleave(e) {
            write!(f, "{x}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct QuantifiedPathPattern {
    pub non_selective_part: Box<PatternPart>,
    pub quantifier: PatternQuantifier,
    // this filter works inside the quantified path pattern
    pub filter: Option<Box<Expr>>,
}

impl std::fmt::Display for QuantifiedPathPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, " (")?;
        write!(f, "{}", self.non_selective_part)?;
        if let Some(filter) = self.filter.as_ref() {
            write!(f, " WHERE {filter}")?;
        }
        write!(f, ")")?;
        write!(f, "{} ", self.quantifier)?;
        Ok(())
    }
}

#[derive(Debug, Display)]
#[display("{}", _0)]
pub enum PatternQuantifier {
    #[display("+")]
    Plus, // {+}
    #[display("*")]
    Star, // {*}
    #[display("{{{}}}", _0)]
    Fixed(u32), // {n}
    #[display("{{{},{}}}", lower.map(|x| x.to_string()).unwrap_or_default(), upper.map(|x| x.to_string()).unwrap_or_default())]
    Interval { lower: Option<u32>, upper: Option<u32> }, // {n,m}
}

#[derive(Debug, Clone)]
pub struct NodePattern {
    pub variable: Option<String>,
    pub label_expr: Option<LabelExpr>,
    pub properties: Option<Expr>,
    pub predicate: Option<Expr>,
}

impl std::fmt::Display for NodePattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({}{}{})",
            self.variable.as_ref().map(|x| x.to_string()).unwrap_or_default(),
            self.label_expr.as_ref().map(|x| format!(":{x}")).unwrap_or_default(),
            self.properties.as_ref().map(|x| x.to_string()).unwrap_or_default()
        )
    }
}

#[derive(Default, Debug, Clone)]
pub struct RelationshipPattern {
    pub variable: Option<String>,
    pub label_expr: Option<LabelExpr>,
    pub properties: Option<Expr>,
    pub predicate: Option<Expr>,
    // (a)-[]->(b): None, no length
    // (a)-[*]->(b): Some(None), any length
    // (a)-[*1..3]->(b): Some(Some(1, 3)), min..max
    // (a)-[*1]->(b): Some(Some(1, 1)), length
    // (a)[*3..]->(b): Some(Some(3, None)), min..inf
    pub length: Option<Option<Range<usize>>>,
    pub direction: SemanticDirection,
}

impl RelationshipPattern {
    pub fn new() -> Self {
        Self {
            variable: None,
            label_expr: None,
            properties: None,
            predicate: None,
            length: None,
            direction: SemanticDirection::Both,
        }
    }
}

impl std::fmt::Display for RelationshipPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.direction {
            SemanticDirection::Outgoing => write!(f, "-[")?,
            SemanticDirection::Incoming => write!(f, "<-[")?,
            SemanticDirection::Both => { write!(f, "-[") }?,
        };
        write!(
            f,
            "{}",
            self.variable.as_ref().map(|x| x.to_string()).unwrap_or_default()
        )?;
        write!(
            f,
            "{}",
            self.label_expr.as_ref().map(|x| format!(":{x}")).unwrap_or_default()
        )?;
        match &self.length {
            Some(None) => write!(f, "*")?,
            Some(Some(range)) => write!(f, "*{}..{}", range.start, range.end)?,
            None => (),
        };
        write!(
            f,
            "{}",
            self.properties.as_ref().map(|x| x.to_string()).unwrap_or_default()
        )?;

        match self.direction {
            SemanticDirection::Outgoing => write!(f, "]->"),
            SemanticDirection::Incoming => write!(f, "]-"),
            SemanticDirection::Both => write!(f, "]-"),
        }
    }
}
