use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use elio_storage::catalog::CatalogStore;

use crate::function::FUNCTION_REGISTRY;

pub mod error;
pub mod func;
pub mod index;
pub use func::FunctionCatalog;

/// Catalog contains
///  - Registered functions
///  - Token to TokenId Mapping
///  - #TODO(pgao): Constraints
///  - #TODO(pgao): index
pub struct SessionCatalog {
    // durable catalog store
    catalog_store: Arc<CatalogStore>,
    // functions
    functions: HashMap<String, FunctionCatalog>,
}

impl std::fmt::Debug for SessionCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Catalog").finish()
    }
}

impl SessionCatalog {
    pub fn new(catalog_store: Arc<CatalogStore>) -> Self {
        Self {
            catalog_store,
            functions: {
                let mut map = HashMap::new();
                for (name, def) in FUNCTION_REGISTRY.deref().name2def.iter() {
                    let func = FunctionCatalog::new(name.to_string(), def.clone());
                    map.insert(name.to_string(), func);
                }
                map
            },
        }
    }

    pub fn get_function_by_name(&self, name: &str) -> Option<&FunctionCatalog> {
        self.functions.get(&name.trim().to_lowercase())
    }

    pub fn catalog_store(&self) -> &Arc<CatalogStore> {
        &self.catalog_store
    }
}
