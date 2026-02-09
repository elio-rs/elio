use itertools::Itertools;

use crate::ast::Expr;

#[derive(Debug)]
pub struct ReturnItems {
    pub projection_kind: ProjectionKind,
    pub items: Vec<ReturnItem>,
}

impl std::fmt::Display for ReturnItems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut items: Vec<String> = self.items.iter().map(|x| x.to_string()).collect();
        if self.projection_kind == ProjectionKind::Additive {
            items.insert(0, "*".to_string());
        }
        write!(f, "{}", items.into_iter().join(", "))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProjectionKind {
    // do not include existing variables
    // WITH a,b,c,
    Free,
    // include existsing
    // WITH *, x, y
    Additive,
    // LET ...
    StrictlyAdditive,
}

impl ProjectionKind {
    pub fn include_existing(&self) -> bool {
        matches!(self, ProjectionKind::Additive | ProjectionKind::StrictlyAdditive)
    }
}

#[derive(Debug, Clone)]
pub struct ReturnItem {
    pub expr: Box<Expr>,
    pub alias: Option<String>,
}

impl std::fmt::Display for ReturnItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.expr)?;
        if let Some(alias) = &self.alias {
            write!(f, " AS {alias}")?;
        }
        Ok(())
    }
}
