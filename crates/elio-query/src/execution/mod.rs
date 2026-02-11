use std::sync::Arc;

use elio_storage::transaction::Transaction;

use crate::catalog::SessionCatalog;
use crate::database::Database;
use crate::execution::task::EvalCtxImpl;

pub mod builder;
pub mod error;
pub mod executor;
pub mod expr;
pub mod panic;
pub mod task;

pub struct QueryContext {
    pub(crate) db: Arc<Database>,
    pub(crate) sess_catalog: Arc<SessionCatalog>,
    pub(crate) tx: Arc<Transaction>,
}

impl QueryContext {
    pub fn derive_eval_ctx(&self) -> EvalCtxImpl {
        EvalCtxImpl {
            catalog: self.sess_catalog.clone(),
            graph_store: self.db.graph_store().clone(),
            tx: self.tx.clone(),
        }
    }
}
