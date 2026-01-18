pub mod sig;

pub mod compare; // gt/eq/lt/le/ge/ne
pub mod convert;
pub mod list;
pub mod op_arith;
pub mod op_bool; // and / or
pub mod op_unary;
pub mod path;
pub mod temporal;

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::func::sig::{FuncDef, FuncImpl};

// Global Function Registry
pub struct FunctionRegistry {
    pub name2def: HashMap<String, FuncDef>,
    pub id2impl: HashMap<String, sig::FuncImpl>,
}

impl FunctionRegistry {
    pub fn insert(&mut self, func: FuncDef) {
        for impl_ in func.impls.iter() {
            self.id2impl.insert(impl_.func_id.clone(), impl_.clone());
        }
        self.name2def.insert(func.name.clone(), func);
    }

    pub fn get_and_func_impl(&self) -> &FuncImpl {
        // SAFETY:
        //    there's only 1 and impl
        self.name2def.get("and").unwrap().impls.first().unwrap()
    }

    pub fn get_or_func_impl(&self) -> &FuncImpl {
        // SAFETY:
        //    there's only 1 or impl
        self.name2def.get("or").unwrap().impls.first().unwrap()
    }

    pub fn get_equal_func_impl(&self) -> &FuncImpl {
        // SAFETY:
        //    there's only 1 equal impl
        self.name2def.get("eq").unwrap().impls.first().unwrap()
    }

    pub fn get_func_impl(&self, func_id: &str) -> &FuncImpl {
        // SAFETY:
        //    only called in expression builder, planner will guarantee the function exists
        self.id2impl.get(func_id).unwrap()
    }
}

pub static FUNCTION_REGISTRY: LazyLock<FunctionRegistry> = LazyLock::new(|| {
    let mut registry = FunctionRegistry {
        name2def: HashMap::new(),
        id2impl: HashMap::new(),
    };

    // register scalar functions
    op_bool::register(&mut registry);
    compare::register(&mut registry);
    temporal::register(&mut registry);
    op_arith::register(&mut registry);
    op_unary::register(&mut registry);
    list::register(&mut registry);
    convert::register(&mut registry);

    // register agg functions

    registry
});
