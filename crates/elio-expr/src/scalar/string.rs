//! String functions
//! - uppper
//! - lower

use std::collections::HashMap;

use elio_common::array::ArrayImpl;
use elio_common::array::chunk::DataChunk;

use crate::define_function;
use crate::error::EvalError;
use crate::func::sig::FuncDef;
use crate::impl_::EvalCtx;
use crate::impl_::func_executor::UnaryExecutor;

fn string_upper(s: &str) -> String {
    s.to_uppercase()
}

fn string_lower(s: &str) -> String {
    s.to_lowercase()
}

fn upper_function(input: &DataChunk, _ctx: &dyn EvalCtx) -> Result<ArrayImpl, EvalError> {
    let res = UnaryExecutor::execute_simple::<String, String, _>(input.column(0), string_upper)?;
    Ok(res)
}

fn lower_function(input: &DataChunk, _ctx: &dyn EvalCtx) -> Result<ArrayImpl, EvalError> {
    let res = UnaryExecutor::execute_simple::<String, String, _>(input.column(0), string_lower)?;
    Ok(res)
}

// register function here

pub(crate) fn register(registry: &mut HashMap<String, FuncDef>) {
    // lower
    let lower = define_function!(
        name: "lower",
        impls: [
            {
                args: [String],
                ret: String,
                func: lower_function,
            }
        ],
        is_agg: false
    );
    // upper
    let upper = define_function!(
        name: "upper",
        impls: [
            {
                args: [String],
                ret: String,
                func: upper_function,
            }
        ],
        is_agg: false
    );

    registry.insert(lower.name.clone(), lower);
    registry.insert(upper.name.clone(), upper);
}
