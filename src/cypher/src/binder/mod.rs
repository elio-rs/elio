use std::sync::Arc;

use elio_common::{IrToken, TokenKind};
use elio_expr::scalar::sig::FuncDef;

use crate::binder::expr::ExprContext;
use crate::binder::scope::Scope;
use crate::session::PlannerSession;
use crate::variable::VariableGenerator;
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
#[derive(Debug, Clone)]
pub struct BindContext {
    pub sctx: Arc<dyn PlannerSession>,
    // TODO(pgao): seems outer_scopes is not needed?
    pub outer_scopes: Vec<Scope>,
    pub variable_generator: Arc<VariableGenerator>,
    // TODO(pgao): semantic context like disable some semantics
}

impl BindContext {
    pub fn new(sctx: Arc<dyn PlannerSession>) -> Self {
        Self {
            sctx,
            outer_scopes: Vec::new(),
            variable_generator: Arc::new(VariableGenerator::default()),
        }
    }

    pub fn derive_expr_context<'a>(&'a self, scope: &'a Scope, name: &'a str) -> ExprContext<'a> {
        ExprContext {
            bctx: self,
            scope,
            name,
            sema_flags: Default::default(),
        }
    }
}

impl BindContext {
    pub fn session(&self) -> &Arc<dyn PlannerSession> {
        &self.sctx
    }

    pub fn resolve_function(&self, name: &str) -> Option<&FuncDef> {
        self.session().get_function_by_name(name).map(|x| &x.func)
    }

    pub fn resolve_token(&self, token: &str, token_kind: TokenKind) -> IrToken {
        match self.session().get_token_id(token, token_kind) {
            Some(token_id) => IrToken::Resolved {
                name: token.to_owned().into(),
                token: token_id,
            },
            None => IrToken::Unresolved(token.to_owned().into()),
        }
    }
}
