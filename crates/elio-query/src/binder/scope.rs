use std::collections::HashSet;

use elio_common::data_type::LogicalType;
use elio_common::schema::Variable;
use elio_common::variable::VariableName;
use elio_parser::ast;

use crate::plan::expr::{Expr, VariableRef};
use crate::plan::ir::query::Bindings;

#[derive(Debug, Clone)]
pub struct ScopeItem {
    // symbol in original query
    // none for anonymous variable
    pub symbol: Option<String>,
    // variable bound to this symbel
    pub variable: VariableName,
    // if expr is non empty,means the variable already be bound to some expression
    // if you want to reference the expression then please set bond_expr to Some
    // When binding project_body, it will replace the symbol to given bound_expr
    // If you do not want to reference the bound expression, please set bound_expr to
    // none, then when binding project_body, it will replace the symbol to VarRef.
    pub expr: HashSet<ast::Expr>,
    pub typ: LogicalType,
    // this is only for path expressions
    pub bound_expr: Option<Expr>,
}

impl ScopeItem {
    pub fn new_variable(variable: VariableName, symbol: Option<&str>, typ: LogicalType) -> Self {
        Self {
            symbol: symbol.map(|s| s.to_string()),
            variable,
            expr: HashSet::new(),
            typ,
            bound_expr: None,
        }
    }

    pub fn as_variable(&self) -> Variable {
        Variable::new(&self.variable, &self.typ)
    }

    pub fn as_expr(&self) -> Expr {
        // if bound_expr is Some, which means this is the path expression, then return the bound expr.
        // TODO(pgao): this is ugly
        self.bound_expr
            .clone()
            .unwrap_or_else(|| VariableRef::new_unchecked(self.variable.clone(), self.typ.clone()).into())
    }

    pub fn is_anonymous(&self) -> bool {
        self.symbol.is_none()
    }

    pub fn bound_expr(&self) -> Option<&Expr> {
        self.bound_expr.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub items: Vec<ScopeItem>,
}

impl Scope {
    pub fn empty() -> Self {
        Self { items: Vec::new() }
    }

    pub fn resolve_expr(&self, expr: &ast::Expr) -> Option<&ScopeItem> {
        self.items.iter().find(|item| item.expr.contains(expr))
    }

    pub fn resolve_symbol(&self, name: &str) -> Option<&ScopeItem> {
        self.items.iter().find(|item| item.symbol.as_deref() == Some(name))
    }

    pub fn resolve_variable(&self, variable: &VariableName) -> Option<&ScopeItem> {
        self.items.iter().find(|item| &item.variable == variable)
    }

    pub fn add_item(&mut self, item: ScopeItem) {
        self.items.push(item);
    }

    pub fn symbol_items(&self) -> impl Iterator<Item = &ScopeItem> {
        self.items.iter().filter(|item| item.symbol.is_some())
    }

    pub fn remove_anonymous(&mut self) {
        self.items.retain(|x| !x.is_anonymous());
    }

    pub fn retain(&mut self, condition: impl Fn(&ScopeItem) -> bool) {
        self.items.retain(|x| condition(x));
    }

    pub fn product(self, other: Self) -> Self {
        let mut items = self.items;
        items.extend(other.items);
        Self { items }
    }

    pub fn input_bindings(&self) -> Bindings {
        self.items.iter().map(|item| item.as_variable().clone()).collect()
    }
}
