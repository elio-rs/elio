use elio_common::schema::Variable;

use crate::expr::Expr;

#[derive(Default, Hash, Debug, Clone, Eq, PartialEq)]
pub struct FilterExprs {
    // conjuncted by AND
    exprs: Vec<Expr>,
}

impl FilterExprs {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_true(&self) -> bool {
        self.exprs.is_empty()
    }

    pub fn push(&mut self, expr: Expr) {
        self.exprs.push(expr);
    }

    pub fn and(mut self, other: Self) -> Self {
        self.exprs.extend(other.exprs);
        self.normalize()
    }

    pub fn or(self, other: Self) -> Self {
        let lhs: Expr = self.into();
        let rhs: Expr = other.into();
        let or = lhs.or(rhs);
        let mut ret = Self::empty();
        ret.exprs.push(or);
        ret.normalize()
    }

    // TODO(pgao): remove false
    fn normalize(self) -> Self {
        self
    }

    pub fn partition_by(self, f: impl Fn(&Expr) -> bool) -> (Self, Self) {
        let (lhs, rhs): (Vec<_>, Vec<_>) = self.exprs.into_iter().partition(f);
        (Self::from_iter(lhs), Self::from_iter(rhs))
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Expr> {
        self.exprs.iter()
    }

    pub fn pretty(&self) -> String {
        self.exprs.iter().map(|e| e.pretty()).collect::<Vec<_>>().join(" AND ")
    }

    pub fn diff(&self, other: &Self) -> Self {
        Self::from_iter(self.exprs.iter().filter(|expr| !other.exprs.contains(expr)).cloned())
    }
}

impl From<FilterExprs> for Expr {
    fn from(val: FilterExprs) -> Self {
        val.exprs.into_iter().fold(Expr::boolean(true), |acc, e| acc.and(e))
    }
}

impl From<Expr> for FilterExprs {
    fn from(val: Expr) -> Self {
        let mut ret = Self::empty();
        ret.exprs.push(val);
        ret
    }
}

impl FromIterator<Expr> for FilterExprs {
    fn from_iter<T: IntoIterator<Item = Expr>>(iter: T) -> Self {
        let mut ret = Self::empty();
        ret.exprs.extend(iter);
        ret
    }
}

impl IntoIterator for FilterExprs {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = Expr;

    fn into_iter(self) -> Self::IntoIter {
        self.exprs.into_iter()
    }
}

pub struct ProjectItem {
    pub variable: Variable,
    pub expr: Box<Expr>,
}
