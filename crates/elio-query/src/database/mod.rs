//! database instance and connection

pub mod error;
pub mod result;
pub mod session;
use std::path::Path;
use std::sync::Arc;

use elio_storage::catalog::CatalogStore;
use elio_storage::graph::GraphStore;
use elio_storage::kv::KvEngine;
use elio_storage::token::TokenStore;
use elio_storage::transaction::manager::TransactionManager;

use crate::database::error::Error;
use crate::database::session::Session;

#[derive(Clone)]
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
    token_store: Arc<TokenStore>,
    catalog_store: Arc<CatalogStore>,
    transaction_manager: Arc<TransactionManager>,
    config: DatabaseConfig,
}

impl Database {
    pub fn open(config: &DatabaseConfig) -> Result<Arc<Database>, Error> {
        let kv_engine = Arc::new(KvEngine::open(&config.path)?);
        let token_store = Arc::new(TokenStore::new(kv_engine.clone())?);
        let graph_store = Arc::new(GraphStore::new(kv_engine.clone(), token_store.clone())?);
        let catalog_store = Arc::new(CatalogStore::new(kv_engine.clone())?);
        let transaction_manager = Arc::new(TransactionManager::new(kv_engine.clone(), catalog_store.clone()));
        Ok(Arc::new(Self {
            graph_store,
            token_store,
            catalog_store,
            transaction_manager,
            config: config.clone(),
        }))
    }

    pub fn graph_store(&self) -> &Arc<GraphStore> {
        &self.graph_store
    }

    pub fn token_store(&self) -> &Arc<TokenStore> {
        &self.token_store
    }

    pub fn catalog_store(&self) -> &Arc<CatalogStore> {
        &self.catalog_store
    }

    pub fn transaction_manager(&self) -> &Arc<TransactionManager> {
        &self.transaction_manager
    }
}

impl Database {
    pub fn new_session(self: &Arc<Self>) -> Arc<Session> {
        Arc::new(Session::new(self.clone()))
    }
}
