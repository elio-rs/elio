use derive_more::Display;
use itertools::Itertools;

use crate::ast::RegularQuery;

#[derive(Debug, Display)]
pub enum Statement {
    // Analyze
    #[display("{}", _0)]
    Explain(Box<Explain>),
    #[display("{}", _0)]
    Query(Box<RegularQuery>),
    #[display("{}", _0)]
    CreateConstraint(Box<CreateConstraint>),
    #[display("{}", _0)]
    DropConstraint(Box<DropConstraint>),
}

/// EXPLAIN query
#[derive(Debug, Display)]
#[display("EXPLAIN {}", query)]
pub struct Explain {
    pub query: RegularQuery,
}

/// CREATE CONSTRAINT constraint_name [IF NOT EXISTS]
/// FOR (var:Label) | ()-[var:REL_TYPE]-()
/// REQUIRE property_expr IS UNIQUE | NODE KEY | NOT NULL
#[derive(Debug)]
pub struct CreateConstraint {
    pub name: String,
    pub if_not_exists: bool,
    pub entity: ConstraintEntity,
    pub constraint_type: ConstraintType,
}

impl std::fmt::Display for CreateConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CREATE CONSTRAINT {}", self.name)?;
        if self.if_not_exists {
            write!(f, " IF NOT EXISTS")?;
        }
        write!(f, " FOR {} REQUIRE {}", self.entity, self.constraint_type)
    }
}

/// The entity that the constraint applies to
#[derive(Debug)]
pub enum ConstraintEntity {
    Node { variable: String, label: String },
    Relationship { variable: String, rel_type: String },
}

impl std::fmt::Display for ConstraintEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintEntity::Node { variable, label } => {
                write!(f, "({}:{})", variable, label)
            }
            ConstraintEntity::Relationship { variable, rel_type } => {
                write!(f, "()-[{}:{}]-()", variable, rel_type)
            }
        }
    }
}

/// The type of constraint
#[derive(Debug)]
pub enum ConstraintType {
    /// Single or multiple properties must be unique
    Unique { properties: Vec<PropertyRef> },
    /// Composite key constraint (properties form a unique key)
    NodeKey { properties: Vec<PropertyRef> },
    /// Property must not be null
    NotNull { property: PropertyRef },
}

impl std::fmt::Display for ConstraintType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintType::Unique { properties } => {
                let props = properties.iter().map(|p| p.to_string()).join(", ");
                if properties.len() > 1 {
                    write!(f, "({}) IS UNIQUE", props)
                } else {
                    write!(f, "{} IS UNIQUE", props)
                }
            }
            ConstraintType::NodeKey { properties } => {
                let props = properties.iter().map(|p| p.to_string()).join(", ");
                write!(f, "({}) IS NODE KEY", props)
            }
            ConstraintType::NotNull { property } => {
                write!(f, "{} IS NOT NULL", property)
            }
        }
    }
}

/// Reference to a property on a variable, e.g., `p.name`
#[derive(Debug, Display)]
#[display("{}.{}", variable, property)]
pub struct PropertyRef {
    pub variable: String,
    pub property: String,
}

/// DROP CONSTRAINT constraint_name [IF EXISTS]
#[derive(Debug)]
pub struct DropConstraint {
    pub name: String,
    pub if_exists: bool,
}

impl std::fmt::Display for DropConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DROP CONSTRAINT {}", self.name)?;
        if self.if_exists {
            write!(f, " IF EXISTS")?;
        }
        Ok(())
    }
}
