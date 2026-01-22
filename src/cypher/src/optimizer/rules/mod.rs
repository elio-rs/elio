use std::sync::Arc;

use crate::optimizer::rule::OptimizationRule;

pub fn unnest_apply_rules() -> Vec<Arc<dyn OptimizationRule>> {
    Vec::new()
}
