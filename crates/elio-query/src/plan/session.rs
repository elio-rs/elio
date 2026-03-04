use elio_common::catalog::IndexHint;
use elio_common::{LabelId, PropertyKeyId, TokenId, TokenKind};
use elio_parser::ast;
use elio_parser::parser::cypher_parser;

use crate::binder::query::bind_root_query;
use crate::catalog::FunctionCatalogEntry;
use crate::catalog::error::CatalogError;
use crate::optimizer::optimize_query;
use crate::plan::error::PlanError;
use crate::plan::ir::query::IrQueryRoot;
use crate::planner::{PlannerContext, RootPlan, plan_root};

/// Session used by the planner.
pub trait PlannerSession {
    fn derive_planner_context(&'_ self) -> PlannerContext<'_>;
    fn catalog(&self) -> &dyn PlannerCatalog;
    fn token_manager(&self) -> &dyn PlannerToken;
    fn send_notification(&self, notification: String);
}

/// Catalog information used by planner
pub trait PlannerCatalog {
    fn resolve_function(&self, name: &str) -> Option<FunctionCatalogEntry>;
    fn find_unique_index(&self, label_id: LabelId, property_key_ids: &[PropertyKeyId]) -> Option<IndexHint>;
}

/// Token Management used by planner
pub trait PlannerToken {
    fn resolve_or_create_token(&self, token: &str, kind: TokenKind) -> Result<TokenId, CatalogError>;
    fn resolve_token(&self, token: &str, kind: TokenKind) -> Option<TokenId>;
}

pub fn parse_statement(stmt: &str) -> Result<ast::Statement, PlanError> {
    cypher_parser::statement(stmt).map_err(PlanError::parse_error)
}

#[derive(Clone, Copy)]
pub enum PlanLevel {
    Plan,
    Optimize,
}

pub fn plan_query(
    ctx: &dyn PlannerSession,
    query: &ast::RegularQuery,
    level: PlanLevel,
) -> Result<RootPlan, PlanError> {
    // bind
    let ir = bind_root_query(ctx, query)?;
    // plan
    let mut planner_ctx = ctx.derive_planner_context();
    let plan = plan_root(&mut planner_ctx, &ir)?;
    if matches!(level, PlanLevel::Plan) {
        return Ok(plan);
    }
    // optimize
    let plan = optimize_query(&mut planner_ctx, plan)?;
    Ok(plan)
}

pub fn bind_query(ctx: &dyn PlannerSession, query: &ast::RegularQuery) -> Result<IrQueryRoot, PlanError> {
    bind_root_query(ctx, query)
}
