//! database instance and connection

pub mod error;
use std::path::Path;
use std::sync::Arc;

use elio_storage::graph::GraphStore;
use elio_storage::token::TokenStore;

use crate::catalog::Catalog;
use crate::database::error::Error;

pub struct DatabaseConfig {
    path: Arc<str>,
}

impl DatabaseConfig {
    pub fn with_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path
                .as_ref()
                .to_str()
                .expect("database path must be valid utf-8 string")
                .into(),
        }
    }
}

pub struct Database {
    graph_store: Arc<GraphStore>,
    /// TODO(pgao): TransactionManager here
    token_store: Arc<TokenStore>,
    /// Durable catalog store
    catalog: Arc<Catalog>,
    config: DatabaseConfig,
}

impl Database {
    pub fn open(config: &DatabaseConfig) -> Result<Arc<Database>, Error> {
        let graph_store = GraphStore::open(&config.path)?;
        todo!()
    }
}
