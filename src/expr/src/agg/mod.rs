use crate::FunctionRegistry;

pub mod sum;

pub(crate) fn register(registry: &mut FunctionRegistry) {
    sum::register(registry);
}
