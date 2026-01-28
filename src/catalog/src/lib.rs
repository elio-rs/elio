use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use elio_expr::scalar::FUNCTION_REGISTRY;
use elio_storage::token::TokenStore;

pub mod error;
pub mod func;
pub use func::FunctionCatalog;

/// Catalog contains
///  - Registered functions
///  - Token to TokenId Mapping
///  - #TODO(pgao): Constraints
///  - #TODO(pgao): index
pub struct Catalog {
    // token store with cache
    token: Arc<TokenStore>,
    // functions
    functions: HashMap<String, FunctionCatalog>,
}

impl std::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Catalog").finish()
    }
}

impl Catalog {
    pub fn new(token: Arc<TokenStore>) -> Self {
        Self {
            token,
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
        self.functions.get(name)
    }
}

impl Deref for Catalog {
    type Target = TokenStore;

    fn deref(&self) -> &Self::Target {
        &self.token
    }
}
