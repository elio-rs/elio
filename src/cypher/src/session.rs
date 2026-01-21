use std::fmt::Debug;
use std::sync::Arc;

use elio_catalog::FunctionCatalog;
use elio_catalog::error::CatalogError;
use elio_common::{LabelId, PropertyKeyId, TokenId, TokenKind};
use elio_parser::ast;
use elio_parser::parser::cypher_parser;

use crate::binder::query::bind_root_query;
use crate::error::PlanError;
use crate::ir::query::IrQueryRoot;
use crate::plan_context::PlanContext;
use crate::planner::{RootPlan, plan_root};

/// Index hint for query optimization
#[derive(Debug, Clone)]
pub struct IndexHint {
    /// Constraint/index name
    pub constraint_name: Arc<str>,
    /// Label ID
    pub label_id: LabelId,
    /// Property key IDs in the index
    pub property_key_ids: Vec<PropertyKeyId>,
}

/// SessionContext for Cypher Planner
pub trait PlannerSession: Debug + Send + Sync {
    // send notification
    fn derive_plan_context(self: Arc<Self>) -> Arc<PlanContext>;
    // catalog
    fn get_or_create_token(&self, token: &str, kind: TokenKind) -> Result<TokenId, CatalogError>;
    fn get_function_by_name(&self, name: &str) -> Option<&FunctionCatalog>;
    fn get_token_id(&self, token: &str, kind: TokenKind) -> Option<TokenId>;

    /// Find an applicable unique index for the given label and property keys.
    /// Returns Some(IndexHint) if a matching index exists, None otherwise.
    fn find_unique_index(&self, label_id: LabelId, property_key_ids: &[PropertyKeyId]) -> Option<IndexHint>;

    // TODO(impl send notification)
    fn send_notification(&self, notification: String);
}

// #[derive(Debug)]
// pub struct SessionContext {
//     pub catalog: Arc<Catalog>,
//     // notification_tx: UnboundedSender<String>,
// }

// impl SessionContext {
//     pub fn derive_plan_context(self: Arc<Self>) -> PlanContext {
//         PlanContext::new(self)
//     }
// }

pub fn parse_statement(stmt: &str) -> Result<ast::Statement, PlanError> {
    cypher_parser::statement(stmt).map_err(PlanError::parse_error)
}

pub fn plan_query(ctx: Arc<dyn PlannerSession>, query: &ast::RegularQuery) -> Result<RootPlan, PlanError> {
    // bind
    let ir = bind_root_query(ctx.clone(), query)?;
    // plan
    let plan = plan_root(ctx, &ir)?;
    Ok(plan)
}

pub fn bind_query(ctx: Arc<dyn PlannerSession>, query: &ast::RegularQuery) -> Result<IrQueryRoot, PlanError> {
    bind_root_query(ctx, query)
}
