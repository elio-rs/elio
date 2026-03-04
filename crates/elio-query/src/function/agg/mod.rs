use crate::function::AggFunctionRegistry;

pub mod sum;

pub(crate) fn register(registry: &mut AggFunctionRegistry) {
    sum::register(registry);
}
