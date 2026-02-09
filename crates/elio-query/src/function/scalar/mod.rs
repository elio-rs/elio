pub mod sig;

use crate::function::FunctionRegistry;

pub mod compare; // gt/eq/lt/le/ge/ne
pub mod convert;
pub mod list;
pub mod op_arith;
pub mod op_bool; // and / or
pub mod op_unary;
pub mod path;
pub mod temporal;

pub(crate) fn register(registry: &mut FunctionRegistry) {
    op_bool::register(registry);
    compare::register(registry);
    temporal::register(registry);
    op_arith::register(registry);
    op_unary::register(registry);
    list::register(registry);
    convert::register(registry);
}
