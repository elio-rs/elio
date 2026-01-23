use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use async_stream::stream;
use elio_catalog::Catalog;
use elio_catalog::error::CatalogError;
use elio_common::array::chunk::DataChunk;
use elio_common::scalar::{Row, ScalarValue};
use elio_common::{LabelId, PropertyKeyId, TokenId, TokenKind};
use elio_cypher::plan_context::PlanContext;
use elio_cypher::session::{IndexHint, PlanLevel, PlannerSession, parse_statement, plan_query};
use elio_exec::error::ExecError;
use elio_exec::task::{ExecContext, create_task};
use elio_parser::ast;
use futures::Stream;
use futures::stream::BoxStream;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::ddl;
use crate::error::Error;
use crate::result::ResultHandle;

#[derive(Debug)]
pub struct Session {
    pub(crate) catalog: Arc<Catalog>,
    pub(crate) exec_ctx: Arc<ExecContext>,
}

impl Session {
    pub fn new(catalog: Arc<Catalog>, exec_ctx: Arc<ExecContext>) -> Self {
        Self { catalog, exec_ctx }
    }
}

// for Planner
impl PlannerSession for Session {
    fn derive_plan_context(self: Arc<Self>) -> Arc<PlanContext> {
        Arc::new(PlanContext::new(self))
    }

    fn get_or_create_token(&self, token: &str, kind: TokenKind) -> Result<TokenId, CatalogError> {
        Ok(self.catalog.get_or_create_token(token, kind)?)
    }

    fn get_function_by_name(&self, name: &str) -> Option<&elio_catalog::FunctionCatalog> {
        self.catalog.get_function_by_name(name)
    }

    fn get_token_id(&self, token: &str, kind: TokenKind) -> Option<TokenId> {
        self.catalog.get_token_id(token, kind)
    }

    fn find_unique_index(&self, label_id: LabelId, property_key_ids: &[PropertyKeyId]) -> Option<IndexHint> {
        // Query the constraint store to find a matching index
        let store = self.exec_ctx.store();
        let constraints = store.constraint_store().get_constraints_for_label(label_id).ok()?;

        // Look for a UNIQUE or NODE KEY constraint that matches the property keys
        for constraint in constraints {
            use elio_storage::constraint::ConstraintKind;
            if matches!(
                constraint.constraint_kind,
                ConstraintKind::Unique | ConstraintKind::NodeKey
            ) {
                // Check if all constraint properties are in the requested set
                // The constraint properties must be a subset of the filter properties
                // and the filter must have all constraint properties
                if constraint.property_key_ids.len() <= property_key_ids.len()
                    && constraint.property_key_ids.iter().all(|p| property_key_ids.contains(p))
                {
                    return Some(IndexHint {
                        constraint_name: constraint.name,
                        label_id: constraint.label_id,
                        property_key_ids: constraint.property_key_ids,
                    });
                }
            }
        }
        None
    }

    fn send_notification(&self, _notification: String) {
        todo!()
    }
}

impl Session {
    pub async fn execute(
        self: &Arc<Self>,
        query: String,
        _params: HashMap<String, ScalarValue>,
    ) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
        let ast = parse_statement(&query)?;
        match ast {
            ast::Statement::Explain(explain) => self.handle_explain(&explain.query).await,
            ast::Statement::Query(regular_query) => self.handle_query(&regular_query).await,
            ast::Statement::CreateConstraint(constraint) => self.handle_create_constraint(&constraint).await,
            ast::Statement::DropConstraint(constraint) => self.handle_drop_constraint(&constraint).await,
        }
    }

    async fn handle_explain(self: &Arc<Self>, query: &ast::RegularQuery) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
        let plan = plan_query(self.clone(), query, PlanLevel::Optimize)?;
        let explain_str = plan.explain();
        Ok(Box::pin(ExplainResultHandle::new(explain_str)))
    }

    async fn handle_query(self: &Arc<Self>, query: &ast::RegularQuery) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
        let plan = plan_query(self.clone(), query, PlanLevel::Optimize)?;
        // execute query
        let query_id = uuid::Uuid::new_v4().to_string().into();
        let handle = create_task(&self.exec_ctx, query_id, plan).await?;
        let bridge = TaskHandleBridge::new(handle.columns.clone(), handle.recv);
        Ok(Box::pin(bridge))
    }

    async fn handle_create_constraint(
        self: &Arc<Self>,
        constraint: &ast::CreateConstraint,
    ) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
        ddl::create_constraint(self.exec_ctx.store(), constraint)?;
        Ok(Box::pin(EmptyResultHandle::new(vec!["result".to_string()])))
    }

    async fn handle_drop_constraint(
        self: &Arc<Self>,
        constraint: &ast::DropConstraint,
    ) -> Result<Pin<Box<dyn ResultHandle>>, Error> {
        ddl::drop_constraint(self.exec_ctx.store(), constraint)?;
        Ok(Box::pin(EmptyResultHandle::new(vec!["result".to_string()])))
    }
}

pub struct TaskHandleBridge {
    pub stream: BoxStream<'static, Result<Row, Error>>,
    columns: Vec<String>,
}

impl TaskHandleBridge {
    pub fn new(columns: Vec<String>, mut data: UnboundedReceiver<Result<DataChunk, ExecError>>) -> Self {
        let s = Box::pin(stream! {
            while let Some(msg) = data.recv().await {
                match msg {
                    Ok(chunk) => {
                        for row_ref in chunk.iter() {
                            let row =
                            row_ref.into_iter().map(|x| x.map(|y| y.to_owned_scalar())).collect::<Row>();
                            yield Ok(row)
                        }
                    }
                    Err(e) =>{
                        yield Err(e.into())
                    }
                }
            }
        });

        Self { stream: s, columns }
    }
}

impl Stream for TaskHandleBridge {
    type Item = Result<Row, Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.stream.as_mut().poll_next(cx)
    }
}

impl ResultHandle for TaskHandleBridge {
    fn columns(&self) -> &[String] {
        &self.columns
    }
}

/// Empty result handle for DDL statements
pub struct EmptyResultHandle {
    columns: Vec<String>,
    done: bool,
}

impl EmptyResultHandle {
    pub fn new(columns: Vec<String>) -> Self {
        Self { columns, done: false }
    }
}

impl Stream for EmptyResultHandle {
    type Item = Result<Row, Error>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            Poll::Ready(None)
        } else {
            self.done = true;
            // Return a single row with "OK" message
            let row = vec![Some(ScalarValue::String("Constraint created/dropped".into()))];
            Poll::Ready(Some(Ok(row)))
        }
    }
}

impl ResultHandle for EmptyResultHandle {
    fn columns(&self) -> &[String] {
        &self.columns
    }
}

/// Result handle for EXPLAIN statements
pub struct ExplainResultHandle {
    columns: Vec<String>,
    explain: String,
    done: bool,
}

impl ExplainResultHandle {
    pub fn new(explain: String) -> Self {
        Self {
            columns: vec!["plan".to_string()],
            explain,
            done: false,
        }
    }
}

impl Stream for ExplainResultHandle {
    type Item = Result<Row, Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.done {
            std::task::Poll::Ready(None)
        } else {
            self.done = true;
            let row = vec![Some(ScalarValue::String(std::mem::take(&mut self.explain)))];
            std::task::Poll::Ready(Some(Ok(row)))
        }
    }
}

impl ResultHandle for ExplainResultHandle {
    fn columns(&self) -> &[String] {
        &self.columns
    }
}
