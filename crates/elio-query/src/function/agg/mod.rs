use crate::function::AggFunctionRegistry;

pub mod collect;
pub mod count;
pub mod sum;

pub(crate) fn register(registry: &mut AggFunctionRegistry) {
    collect::register(registry);
    count::register(registry);
    sum::register(registry);
}
