use crate::function::ScalarFunctionRegistry;

// pub mod sig;

pub mod compare;
pub mod convert;
pub mod list;
pub mod op_arith;
pub mod op_bool;
pub mod op_unary;
pub mod path;
pub mod temporal;

pub(crate) fn register(registry: &mut ScalarFunctionRegistry) {
    op_bool::register(registry);
    compare::register(registry);
    temporal::register(registry);
    op_arith::register(registry);
    op_unary::register(registry);
    list::register(registry);
    convert::register(registry);
}
