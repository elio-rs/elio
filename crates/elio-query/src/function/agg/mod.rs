use crate::function::AggFunctionRegistry;

pub mod count;
pub mod sum;

pub(crate) fn register(registry: &mut AggFunctionRegistry) {
    count::register(registry);
    sum::register(registry);
}
