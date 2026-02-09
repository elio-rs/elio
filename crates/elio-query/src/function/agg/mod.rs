use crate::function::FunctionRegistry;

pub mod sum;

pub(crate) fn register(registry: &mut FunctionRegistry) {
    sum::register(registry);
}
