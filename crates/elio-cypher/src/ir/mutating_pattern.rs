use std::collections::HashSet;

use elio_common::IrToken;
use elio_common::data_type::DataType;
use elio_common::schema::Variable;
use elio_common::store_types::RelDirection;
use elio_common::variable::VariableName;
use indexmap::IndexSet;
use pretty_xmlish::XmlNode;

use crate::expr::{BoxedExpr, CreateStruct};
use crate::pretty_utils::pretty_display_iter;

pub enum MutatingPattern {
    Create(CreatePattern),
}

impl MutatingPattern {
    pub fn used_variables(&self) -> IndexSet<Variable> {
        let mut vars = IndexSet::new();
        match self {
            MutatingPattern::Create(create_pattern) => {
                create_pattern.nodes.iter().for_each(|n| {
                    vars.insert(Variable::new(&n.variable, &DataType::Node));
                });
                create_pattern.rels.iter().for_each(|r| {
                    vars.insert(Variable::new(&r.variable, &DataType::Rel));
                });
            }
        }
        vars
    }

    pub fn xmlnode(&self) -> XmlNode<'_> {
        match self {
            MutatingPattern::Create(create_pattern) => create_pattern.xmlnode(),
        }
    }
}

/// semantic:
//   - nodes are ok to reference previous patterns in MATCH clause
//   - rels must not reference previous patterns, and should only be defined here.
pub struct CreatePattern {
    pub nodes: Vec<CreateNode>,
    pub rels: Vec<CreateRel>,
}

impl CreatePattern {
    pub fn xmlnode(&self) -> XmlNode<'_> {
        let fields = vec![
            ("nodes", pretty_display_iter(self.nodes.iter())),
            ("rels", pretty_display_iter(self.rels.iter())),
        ];
        XmlNode::simple_record("CreatePattern", fields, vec![])
    }
}

pub struct CreateNode {
    pub variable: VariableName,
    // labels are conjuncted with AND
    pub labels: HashSet<IrToken>,
    // CREATE (a:{name: "Bob"})
    // properties: (name, "bob")
    pub properties: CreateStruct,
}

impl std::fmt::Display for CreateNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({var}){labels} {properties}",
            var = self.variable,
            labels = if self.labels.is_empty() {
                "".to_string()
            } else {
                format!(
                    ":{}",
                    self.labels.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(":")
                )
            },
            // TODO(pgao): avoid the clone here
            properties = BoxedExpr::from(self.properties.clone()).pretty(),
        )
    }
}

/// Relationship vairables MUST NOT reference previous pattern.
/// Relationship variables must be defined in CreatePattern scope.
pub struct CreateRel {
    pub variable: VariableName,
    pub left: VariableName,
    pub right: VariableName,
    pub reltype: IrToken,
    pub direction: RelDirection,
    pub properties: CreateStruct,
}

impl std::fmt::Display for CreateRel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (ldir, rdir) = match self.direction {
            RelDirection::Out => ("-", "->"),
            RelDirection::In => ("<-", "-"),
        };
        write!(
            f,
            "({left}){ldir}[{var}:{reltype}]{rdir}({right}) {properties}",
            left = self.left,
            var = self.variable,
            reltype = self.reltype,
            right = self.right,
            // TODO(pgao): avoid the clone here
            properties = BoxedExpr::from(self.properties.clone()).pretty(),
        )
    }
}

impl CreateRel {
    // Return (start, end) node
    pub fn start_end_nodes(&self) -> (&VariableName, &VariableName) {
        if matches!(self.direction, RelDirection::Out) {
            (&self.left, &self.right)
        } else {
            (&self.right, &self.left)
        }
    }
}
