//! database instance and connection

pub mod error;
pub mod result;
pub mod session;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use elio_storage::catalog::CatalogState;
use elio_storage::graph::GraphStore;
use elio_storage::kv::KvEngine;
use elio_storage::token::TokenStore;
use elio_storage::transaction::manager::TransactionManager;
use hashbrown::HashMap;

use crate::catalog::FunctionCatalog;
use crate::database::error::Error;
use crate::database::session::Session;
use crate::function::FUNCTION_REGISTRY;

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
    functions: HashMap<String, FunctionCatalog>,
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
        let functions = {
            let mut map = HashMap::new();
            for (name, def) in FUNCTION_REGISTRY.deref().name2def.iter() {
                let func = FunctionCatalog::new(name.to_string(), def.clone());
                map.insert(name.to_string(), func);
            }
            map
        };
        Ok(Arc::new(Self {
            graph_store,
            token_store,
            // catalog_state,
            transaction_manager,
            functions,
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

    pub fn get_function_by_name(&self, name: &str) -> Option<&FunctionCatalog> {
        self.functions.get(&name.trim().to_lowercase())
    }
}

impl Database {
    pub fn new_session(self: &Arc<Self>) -> Arc<Session> {
        Arc::new(Session::new(self.clone()))
    }
}
