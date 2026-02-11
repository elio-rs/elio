use std::pin::Pin;
use std::sync::Arc;

use elio_common::catalog::{ConstraintCatalogEntry, ConstraintKind, IndexHint};
use elio_common::scalar::ScalarValue;
use elio_common::{EntityKind, LabelId, PropertyKeyId, TokenKind};
use elio_parser::ast::{self, QueryKind};
use elio_storage::token::TokenStore;
use elio_storage::transaction::Transaction;
use elio_storage::transaction::manager::TransactionMode;
use hashbrown::HashMap;

use crate::catalog::{FunctionCatalog, SessionCatalog};
use crate::database::Database;
use crate::database::error::Error;
use crate::database::result::{EmptyResultHandle, ExplainResultHandle, ResultHandle, TaskHandleBridge};
use crate::execution::QueryContext;
use crate::execution::ddl::build_index;
use crate::execution::task::create_task;
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
        let sess_catalog = SessionCatalog::new(db.catalog_store.clone());
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
    fn derive_planner_context(&'_ self) -> planner::PlannerContext<'_> {
        planner::PlannerContext::new(self)
    }

    fn catalog(&self) -> &dyn PlannerCatalog {
        self
    }

    fn token_manager(&self) -> &dyn PlannerToken {
        self.token_store().as_ref()
    }

    fn send_notification(&self, _notification: String) {
        todo!()
    }
}

impl PlannerCatalog for QueryContext {
    fn resolve_function(&self, name: &str) -> Option<&FunctionCatalog> {
        self.sess_catalog.get_function_by_name(name)
    }

    fn find_unique_index(&self, label_id: LabelId, property_key_ids: &[PropertyKeyId]) -> Option<IndexHint> {
        self.sess_catalog
            .catalog_store()
            .find_unique_index(self.txn().as_ref(), label_id, property_key_ids)
            .unwrap()
    }
}

impl PlannerToken for TokenStore {
    fn resolve_or_create_token(
        &self,
        token: &str,
        kind: elio_common::TokenKind,
    ) -> Result<elio_common::TokenId, crate::catalog::error::CatalogError> {
        self.get_or_create_token(token, kind).map_err(|e| e.into())
    }

    fn resolve_token(&self, token: &str, kind: elio_common::TokenKind) -> Option<elio_common::TokenId> {
        self.get_token_id(token, kind)
    }
}

impl Session {
    pub async fn execute(
        &self,
        query: &str,
        _param: HashMap<String, ScalarValue>,
    ) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
        let ast = parse_statement(query)?;
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
    let handle = create_task(qctx, query_id, plan).await?;
    let bridge = TaskHandleBridge::new(handle.columns.clone(), handle.recv);
    Ok(Box::pin(bridge))
}

async fn handle_create_constraint(
    qctx: Arc<QueryContext>,
    stmt: &ast::CreateConstraint,
) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
    let catalog_store = qctx.sess_catalog().catalog_store().clone();
    let token_store = qctx.token_store().clone();
    let tx = qctx.txn();

    // check if constraint already exists
    if catalog_store.constraint_exists(tx.as_ref(), &stmt.name)? {
        if stmt.if_not_exists {
            return Ok(Box::pin(EmptyResultHandle::new(vec!["result".into()])));
        }
        return Err(Error::ConstraintAlreadyExists(stmt.name.clone()));
    }

    // resolve entity kind and label name
    let (entity_kind, label_name) = match &stmt.entity {
        ast::ConstraintEntity::Node { label, .. } => (EntityKind::Node, label.clone()),
        ast::ConstraintEntity::Relationship { rel_type, .. } => (EntityKind::Rel, rel_type.clone()),
    };

    // resolve constraint kind and property names
    let (constraint_kind, property_names) = match &stmt.constraint_type {
        ast::ConstraintType::Unique { properties } => {
            let names: Vec<String> = properties.iter().map(|p| p.property.clone()).collect();
            (ConstraintKind::Unique, names)
        }
        ast::ConstraintType::NodeKey { properties } => {
            let names: Vec<String> = properties.iter().map(|p| p.property.clone()).collect();
            (ConstraintKind::NodeKey, names)
        }
        ast::ConstraintType::NotNull { property } => (ConstraintKind::NotNull, vec![property.property.clone()]),
    };

    // resolve tokens
    let label_id = token_store.get_or_create_token(&label_name, TokenKind::Label)?;
    let property_key_ids: Vec<PropertyKeyId> = property_names
        .iter()
        .map(|name| token_store.get_or_create_token(name, TokenKind::PropertyKey))
        .collect::<Result<_, _>>()?;

    // build index for Unique/NodeKey constraints
    if matches!(constraint_kind, ConstraintKind::Unique | ConstraintKind::NodeKey) {
        build_index(&qctx, &constraint_kind, label_id, &property_key_ids, &property_names)?;
    }

    // register constraint in catalog
    let entry = ConstraintCatalogEntry {
        name: Arc::from(stmt.name.as_str()),
        entity_kind,
        label_or_rel_id: label_id,
        constraint_kind,
        property_key_ids,
    };
    catalog_store.put_constraint(tx.as_ref(), &entry)?;

    // ddl commits its own transaction
    tx.commit()?;

    Ok(Box::pin(EmptyResultHandle::new(vec!["result".into()])))
}

async fn handle_drop_constraint(
    qctx: Arc<QueryContext>,
    stmt: &ast::DropConstraint,
) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
    let catalog_store = qctx.sess_catalog().catalog_store().clone();
    let tx = qctx.txn();

    if !catalog_store.constraint_exists(tx.as_ref(), &stmt.name)? {
        if stmt.if_exists {
            return Ok(Box::pin(EmptyResultHandle::new(vec!["result".into()])));
        }
        return Err(Error::ConstraintNotFound(stmt.name.clone()));
    }

    catalog_store.delete_constraint(tx.as_ref(), &stmt.name)?;

    // ddl commits its own transaction
    tx.commit()?;

    Ok(Box::pin(EmptyResultHandle::new(vec!["result".into()])))
}
