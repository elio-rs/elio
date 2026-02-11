use std::pin::Pin;
use std::sync::Arc;

use elio_common::catalog::IndexHint;
use elio_common::scalar::ScalarValue;
use elio_parser::ast::{self, QueryKind};
use elio_storage::transaction::Transaction;
use elio_storage::transaction::manager::TransactionMode;
use hashbrown::HashMap;

use crate::catalog::SessionCatalog;
use crate::database::Database;
use crate::database::error::Error;
use crate::database::result::{ExplainResultHandle, ResultHandle, TaskHandleBridge};
use crate::execution::QueryContext;
use crate::plan::session::{PlanLevel, PlannerCatalog, PlannerSession, PlannerToken, parse_statement, plan_query};
use crate::planner;

pub struct Session {
    db: Arc<Database>,
    sess_catalog: Arc<SessionCatalog>,
    // TODO(pgao): explicit transaction
    // active_tx: Option<Arc<Transaction>>,
}

impl Session {
    pub fn new(db: Arc<Database>) -> Self {
        let sess_catalog = SessionCatalog::new(db.catalog_store.clone(), db.token_store.clone());
        Self {
            db,
            sess_catalog: Arc::new(sess_catalog),
        }
    }

    pub fn derive_query_context(&self, tx: Arc<Transaction>) -> QueryContext {
        QueryContext {
            db: self.db.clone(),
            sess_catalog: self.sess_catalog.clone(),
            tx,
        }
    }
}

impl PlannerSession for QueryContext {
    fn derive_planner_context(&self) -> planner::PlannerContext {
        planner::PlannerContext::new(self)
    }

    fn catalog(&self) -> &dyn PlannerCatalog {
        todo!()
    }

    fn token_manager(&self) -> &dyn PlannerToken {
        todo!()
    }

    fn send_notification(&self, notification: String) {
        todo!()
    }
}

impl PlannerCatalog for QueryContext {
    fn resolve_function(&self, name: &str) -> Option<&crate::catalog::FunctionCatalog> {
        self.sess_catalog.get_function_by_name(name)
    }

    fn find_unique_index(
        &self,
        label_id: elio_common::LabelId,
        property_key_ids: &[elio_common::PropertyKeyId],
    ) -> Option<IndexHint> {
        // TODO(pgao): make it return Result
        self.sess_catalog
            .catalog_store()
            .find_unique_index(&self.tx, label_id, property_key_ids)
            .unwrap()
    }
}

impl PlannerToken for QueryContext {
    fn resolve_or_create_token(
        &self,
        token: &str,
        kind: elio_common::TokenKind,
    ) -> Result<elio_common::TokenId, crate::catalog::error::CatalogError> {
        self.token_manager().resolve_or_create_token(token, kind)
    }

    fn resolve_token(&self, token: &str, kind: elio_common::TokenKind) -> Option<elio_common::TokenId> {
        self.token_manager().resolve_token(token, kind)
    }
}

impl Session {
    pub async fn execute(
        &self,
        query: &str,
        _param: HashMap<String, ScalarValue>,
    ) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
        let ast = parse_statement(&query)?;
        let query_kind = ast.query_kind();
        let tx_mode = {
            match query_kind {
                QueryKind::Read => TransactionMode::ReadOnly,
                QueryKind::ReadWrite => TransactionMode::ReadWrite,
            }
        };

        let tx = self.db.transaction_manager().begin(tx_mode);

        let qctx = Arc::new(self.derive_query_context(tx));

        match ast {
            ast::Statement::Explain(explain) => handle_explain(qctx, &explain.query).await,
            ast::Statement::Query(regular_query) => handle_query(qctx, &regular_query).await,
            ast::Statement::CreateConstraint(constraint) => handle_create_constraint(qctx, &constraint).await,
            ast::Statement::DropConstraint(constraint) => handle_drop_constraint(qctx, &constraint).await,
        }
    }
}

async fn handle_explain(
    qctx: Arc<QueryContext>,
    query: &ast::RegularQuery,
) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
    let plan = plan_query(qctx.as_ref(), query, PlanLevel::Optimize)?;
    let explain_str = plan.explain();
    Ok(Box::pin(ExplainResultHandle::new(explain_str)))
}

async fn handle_query(qctx: Arc<QueryContext>, query: &ast::RegularQuery) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
    let plan = plan_query(qctx.as_ref(), query, PlanLevel::Optimize)?;
    // execute query
    let query_id = uuid::Uuid::new_v4().to_string().into();
    let handle = create_task(&self.exec_ctx, query_id, plan).await?;
    let bridge = TaskHandleBridge::new(handle.columns.clone(), handle.recv);
    Ok(Box::pin(bridge))
}

async fn handle_create_constraint(
    qctx: Arc<QueryContext>,
    constraint: &ast::CreateConstraint,
) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
    todo!()
    // ddl::create_constraint(self.exec_ctx.store(), constraint)?;
    // Ok(Box::pin(EmptyResultHandle::new(vec!["result".to_string()])))
}

async fn handle_drop_constraint(
    qctx: Arc<QueryContext>,
    constraint: &ast::DropConstraint,
) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
    todo!()
    // ddl::drop_constraint(self.exec_ctx.store(), constraint)?;
    // Ok(Box::pin(EmptyResultHandle::new(vec!["result".to_string()])))
}
