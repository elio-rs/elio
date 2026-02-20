use std::sync::Arc;

use elio_storage::graph::GraphStore;
use elio_storage::token::TokenStore;
use elio_storage::transaction::Transaction;

use crate::database::Database;
use crate::execution::task::EvalCtxImpl;

pub mod builder;
pub mod ddl;
pub mod error;
pub mod executor;
pub mod expr;
pub mod panic;
pub mod task;

pub struct QueryContext {
    pub(crate) db: Arc<Database>,
    pub(crate) tx: Arc<Transaction>,
}

impl QueryContext {
    pub fn derive_eval_ctx(&self) -> EvalCtxImpl {
        EvalCtxImpl {
            token_store: self.token_store().clone(),
            graph_store: self.db.graph_store().clone(),
            tx: self.tx.clone(),
        }
    }

    pub fn token_store(&self) -> &Arc<TokenStore> {
        self.db.token_store()
    }

    pub fn graph_store(&self) -> &Arc<GraphStore> {
        self.db.graph_store()
    }

    pub fn txn(&self) -> &Arc<Transaction> {
        &self.tx
    }
}
