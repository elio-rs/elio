pub mod agg;
pub mod scalar;
pub mod sig;

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::function::sig::{AggFunctionSet, ScalarFunction, ScalarFunctionSet};

#[derive(Clone, Debug)]
pub struct ScalarFunctionRegistry {
    pub entries: HashMap<String, ScalarFunctionSet>,
}

impl Default for ScalarFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ScalarFunctionRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, func: ScalarFunctionSet) {
        self.entries.insert(func.name.to_string(), func);
    }

    pub fn get_function(&self, name: &str) -> Option<&ScalarFunctionSet> {
        self.entries.get(&name.to_lowercase())
    }

    pub fn get_and_function(&self) -> ScalarFunction {
        // NB: we assume there is only one and function
        self.get_function("and").unwrap().functions.first().unwrap().clone()
    }

    pub fn get_or_function(&self) -> ScalarFunction {
        // NB: we assume there is only one or function
        self.get_function("or").unwrap().functions.first().unwrap().clone()
    }

    pub fn get_eq_function(&self) -> ScalarFunction {
        // NB: we assume there is only one eq function
        self.get_function("eq").unwrap().functions.first().unwrap().clone()
    }
}

#[derive(Clone, Debug)]
pub struct AggFunctionRegistry {
    pub entries: HashMap<String, AggFunctionSet>,
}

impl AggFunctionRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, func: AggFunctionSet) {
        self.entries.insert(func.name.to_string(), func);
    }

    pub fn get_function(&self, name: &str) -> Option<&AggFunctionSet> {
        self.entries.get(&name.to_lowercase())
    }
}

impl Default for AggFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub static SCALAR_FUNCTION_REGISTRY: LazyLock<ScalarFunctionRegistry> = LazyLock::new(|| {
    let mut registry = ScalarFunctionRegistry::new();
    scalar::register(&mut registry);
    registry
});

pub static AGG_FUNCTION_REGISTRY: LazyLock<AggFunctionRegistry> = LazyLock::new(|| {
    let mut registry = AggFunctionRegistry::new();
    agg::register(&mut registry);
    registry
});
