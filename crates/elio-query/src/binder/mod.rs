use std::sync::Arc;

use elio_common::{IrToken, TokenKind};

use crate::binder::expr::ExprContext;
use crate::binder::scope::Scope;
use crate::catalog::FunctionCatalogEntry;
use crate::plan::session::PlannerSession;
use crate::plan::variable::VariableGenerator;
mod builder;
pub mod create;
pub mod expr;
pub mod label_expr;
pub mod load;
pub mod match_;
pub mod pattern;
pub mod project_body;
pub mod query;
pub mod scope;

/// Context to bind a query
pub struct BindContext<'a> {
    pub sctx: &'a dyn PlannerSession,
    // TODO(pgao): seems outer_scopes is not needed?
    pub outer_scopes: Vec<Scope>,
    pub variable_generator: Arc<VariableGenerator>,
    // TODO(pgao): semantic context like disable some semantics
}

impl<'a> BindContext<'a> {
    pub fn new(sctx: &'a dyn PlannerSession) -> Self {
        Self {
            sctx,
            outer_scopes: Vec::new(),
            variable_generator: Arc::new(VariableGenerator::default()),
        }
    }

    pub fn derive_expr_context(&'a self, scope: &'a Scope, name: &'a str) -> ExprContext<'a> {
        ExprContext {
            bctx: self,
            scope,
            name,
            sema_flags: Default::default(),
        }
    }
}

impl<'a> BindContext<'a> {
    pub fn session(&self) -> &'a dyn PlannerSession {
        self.sctx
    }

    pub fn resolve_function(&self, name: &str) -> Option<FunctionCatalogEntry> {
        self.session().catalog().resolve_function(name)
    }

    pub fn resolve_token(&self, token: &str, token_kind: TokenKind) -> IrToken {
        match self.session().token_manager().resolve_token(token, token_kind) {
            Some(token_id) => IrToken::Resolved {
                name: token.to_owned().into(),
                token: token_id,
            },
            None => IrToken::Unresolved(token.to_owned().into()),
        }
    }
}
