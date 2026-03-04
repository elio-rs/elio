//! database instance and connection

pub mod error;
pub mod result;
pub mod session;
use std::path::Path;
use std::sync::Arc;

use elio_storage::catalog::CatalogState;
use elio_storage::graph::GraphStore;
use elio_storage::kv::KvEngine;
use elio_storage::token::TokenStore;
use elio_storage::transaction::manager::TransactionManager;

use crate::catalog::FunctionCatalogEntry;
use crate::database::error::Error;
use crate::database::session::Session;
use crate::function::{AGG_FUNCTION_REGISTRY, AggFunctionRegistry, SCALAR_FUNCTION_REGISTRY, ScalarFunctionRegistry};

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
    // catalog_state: Arc<CatalogState>,
    transaction_manager: Arc<TransactionManager>,
    // Currently we put functions here since they are static over the database lifetime
    // and have nothing todo with transaction.
    scalar_functions: ScalarFunctionRegistry,
    agg_functions: AggFunctionRegistry,
    #[allow(unused)]
    config: DatabaseConfig,
}

impl Database {
    pub fn open(config: &DatabaseConfig) -> Result<Arc<Database>, Error> {
        let kv_engine = Arc::new(KvEngine::open(&config.path)?);
        let token_store = Arc::new(TokenStore::new(kv_engine.clone())?);
        let graph_store = Arc::new(GraphStore::new(kv_engine.clone(), token_store.clone())?);
        let catalog_state = Arc::new(CatalogState::new(kv_engine.clone())?);
        let transaction_manager = Arc::new(TransactionManager::new(kv_engine.clone(), catalog_state.clone()));
        // initialize function registries
        let scalar_functions = SCALAR_FUNCTION_REGISTRY.clone();
        let agg_functions = AGG_FUNCTION_REGISTRY.clone();

        Ok(Arc::new(Self {
            graph_store,
            token_store,
            // catalog_state,
            transaction_manager,
            scalar_functions,
            agg_functions,
            config: config.clone(),
        }))
    }

    pub fn graph_store(&self) -> &Arc<GraphStore> {
        &self.graph_store
    }

    pub fn token_store(&self) -> &Arc<TokenStore> {
        &self.token_store
    }

    pub fn transaction_manager(&self) -> &Arc<TransactionManager> {
        &self.transaction_manager
    }

    pub fn get_function_by_name(&self, name: &str) -> Option<FunctionCatalogEntry> {
        self.scalar_functions
            .get_function(name)
            .map(|func| FunctionCatalogEntry::Scalar(func.clone()))
            .or_else(|| {
                self.agg_functions
                    .get_function(name)
                    .map(|func| FunctionCatalogEntry::Agg(func.clone()))
            })
    }
}

impl Database {
    pub fn new_session(self: &Arc<Self>) -> Arc<Session> {
        Arc::new(Session::new(self.clone()))
    }
}
