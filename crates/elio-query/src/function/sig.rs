use std::fmt::Debug;
use std::sync::Arc;

use downcast_rs::Downcast;
use educe::Educe;
use elio_common::data_type::LogicalType;
use elio_parser::ast;

use crate::execution::builder::BuildError;
use crate::execution::expr::{AggFunctionExec, ScalarFunctionExec};
use crate::plan::error::PlanError;

/// Function specific data, for example percentile function needs an float value as argument.
pub trait FunctionData: Downcast + Debug + Send + Sync + 'static {
    fn clone_box(&self) -> Box<dyn FunctionData>;
    fn dyn_eq(&self, other: &dyn FunctionData) -> bool;
    fn dyn_hash(&self, state: &mut dyn std::hash::Hasher);
}

downcast_rs::impl_downcast!(FunctionData);

impl Clone for Box<dyn FunctionData> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl PartialEq for Box<dyn FunctionData> {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other.as_ref())
    }
}

impl Eq for Box<dyn FunctionData> {}

impl std::hash::Hash for Box<dyn FunctionData> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.dyn_hash(state)
    }
}

impl FunctionData for () {
    fn clone_box(&self) -> Box<dyn FunctionData> {
        Box::new(())
    }

    fn dyn_eq(&self, other: &dyn FunctionData) -> bool {
        other.as_any().is::<()>()
    }

    fn dyn_hash(&self, _state: &mut dyn std::hash::Hasher) {}
}

/// Used to bind and parse the function data from ast
pub type FunctionBindCallback = fn(args: &[ast::Expr]) -> Result<Box<dyn FunctionData>, PlanError>;
/// Used to build executable class.
pub type ScalarFunctionExecBuild =
    fn(function_data: Box<dyn FunctionData>) -> Result<Arc<dyn ScalarFunctionExec>, BuildError>;
/// Used to build executable class for aggregate function.
pub type AggFunctionExecBuild =
    fn(function_data: Box<dyn FunctionData>) -> Result<Arc<dyn AggFunctionExec>, BuildError>;

#[derive(Clone, Debug)]
pub struct ScalarFunctionSet {
    pub name: &'static str,
    //- impls
    pub functions: Vec<ScalarFunction>,
}

impl ScalarFunctionSet {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            functions: Vec::new(),
        }
    }

    pub fn add_function(&mut self, function: ScalarFunction) {
        self.functions.push(function);
    }
}

#[derive(Clone, Educe)]
#[educe(Debug, Hash, PartialEq, Eq)]
pub struct ScalarFunction {
    pub name: &'static str,
    //--- signature
    pub arguments: Vec<LogicalType>,
    pub varargs: Option<LogicalType>,
    pub return_type: LogicalType,
    //--- binding
    #[educe(Debug(ignore), Hash(ignore), PartialEq(ignore))]
    pub bind_callback: Option<FunctionBindCallback>,
    //--- execution
    #[educe(Debug(ignore), Hash(ignore), PartialEq(ignore))]
    pub execute_builder: ScalarFunctionExecBuild,
}

impl ScalarFunction {
    pub fn matches_with_null_coercion(
        &self,
        args: &[LogicalType],
        is_untyped_null: &[bool],
    ) -> Option<(LogicalType, Vec<LogicalType>)> {
        type_resolve::matches_with_null_coercion(&self.arguments, &self.return_type, args, is_untyped_null)
    }

    pub fn matches(&self, args: &[LogicalType]) -> Option<LogicalType> {
        type_resolve::matches(&self.arguments, &self.return_type, args)
    }
}

#[derive(Clone, Debug)]
pub struct AggFunctionSet {
    pub name: &'static str,
    pub functions: Vec<AggFunction>,
}

impl AggFunctionSet {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            functions: Vec::new(),
        }
    }

    pub fn add_function(&mut self, function: AggFunction) {
        self.functions.push(function);
    }
}

#[derive(Educe, Clone)]
#[educe(Debug, Hash, PartialEq, Eq)]
pub struct AggFunction {
    pub name: &'static str,
    //--- signature
    pub arguments: Vec<LogicalType>,
    pub varargs: Option<LogicalType>,
    pub return_type: LogicalType,
    //--- binding
    #[educe(Debug(ignore), Hash(ignore), PartialEq(ignore))]
    pub bind_callback: Option<FunctionBindCallback>,
    //--- execution
    #[educe(Debug(ignore), Hash(ignore), PartialEq(ignore))]
    pub execute_builder: AggFunctionExecBuild,
}

impl AggFunction {
    pub fn matches_with_null_coercion(
        &self,
        args: &[LogicalType],
        is_untyped_null: &[bool],
    ) -> Option<(LogicalType, Vec<LogicalType>)> {
        type_resolve::matches_with_null_coercion(&self.arguments, &self.return_type, args, is_untyped_null)
    }

    pub fn matches(&self, args: &[LogicalType]) -> Option<LogicalType> {
        type_resolve::matches(&self.arguments, &self.return_type, args)
    }
}

#[macro_export]
macro_rules! scalar_function {
    // [args] -> return_type, exec_builder
    ($name:expr, [$($arg:expr),* $(,)?] -> $ret:expr, $exec:expr) => {
        $crate::function::sig::ScalarFunction {
            name: $name,
            arguments: vec![$($arg),*],
            varargs: None,
            return_type: $ret,
            bind_callback: None,
            execute_builder: $exec,
        }
    };
    // [args] varargs(type) -> return_type, exec_builder
    ($name:expr, [$($arg:expr),* $(,)?] varargs($va:expr) -> $ret:expr, $exec:expr) => {
        $crate::function::sig::ScalarFunction {
            name: $name,
            arguments: vec![$($arg),*],
            varargs: Some($va),
            return_type: $ret,
            bind_callback: None,
            execute_builder: $exec,
        }
    };
    // [args] -> return_type, bind_callback, exec_builder
    ($name:expr, [$($arg:expr),* $(,)?] -> $ret:expr, $bind:expr, $exec:expr) => {
        $crate::function::sig::ScalarFunction {
            name: $name,
            arguments: vec![$($arg),*],
            varargs: None,
            return_type: $ret,
            bind_callback: Some($bind),
            execute_builder: $exec,
        }
    };
    // [args] varargs(type) -> return_type, bind_callback, exec_builder
    ($name:expr, [$($arg:expr),* $(,)?] varargs($va:expr) -> $ret:expr, $bind:expr, $exec:expr) => {
        $crate::function::sig::ScalarFunction {
            name: $name,
            arguments: vec![$($arg),*],
            varargs: Some($va),
            return_type: $ret,
            bind_callback: Some($bind),
            execute_builder: $exec,
        }
    };
}

#[macro_export]
macro_rules! agg_function {
    // [args] -> return_type, exec_builder
    ($name:expr, [$($arg:expr),* $(,)?] -> $ret:expr, $exec:expr) => {
        $crate::function::sig::AggFunction {
            name: $name,
            arguments: vec![$($arg),*],
            varargs: None,
            return_type: $ret,
            bind_callback: None,
            execute_builder: $exec,
        }
    };
    // [args] varargs(type) -> return_type, exec_builder
    ($name:expr, [$($arg:expr),* $(,)?] varargs($va:expr) -> $ret:expr, $exec:expr) => {
        $crate::function::sig::AggFunction {
            name: $name,
            arguments: vec![$($arg),*],
            varargs: Some($va),
            return_type: $ret,
            bind_callback: None,
            execute_builder: $exec,
        }
    };
    // [args] -> return_type, bind_callback, exec_builder
    ($name:expr, [$($arg:expr),* $(,)?] -> $ret:expr, $bind:expr, $exec:expr) => {
        $crate::function::sig::AggFunction {
            name: $name,
            arguments: vec![$($arg),*],
            varargs: None,
            return_type: $ret,
            bind_callback: Some($bind),
            execute_builder: $exec,
        }
    };
    // [args] varargs(type) -> return_type, bind_callback, exec_builder
    ($name:expr, [$($arg:expr),* $(,)?] varargs($va:expr) -> $ret:expr, $bind:expr, $exec:expr) => {
        $crate::function::sig::AggFunction {
            name: $name,
            arguments: vec![$($arg),*],
            varargs: Some($va),
            return_type: $ret,
            bind_callback: Some($bind),
            execute_builder: $exec,
        }
    };
}

mod type_resolve {
    use super::*;
    pub fn matches_with_null_coercion(
        arguments: &[LogicalType],
        return_type: &LogicalType,
        provided: &[LogicalType],
        is_untyped_null: &[bool],
    ) -> Option<(LogicalType, Vec<LogicalType>)> {
        if arguments.len() != provided.len() || arguments.len() != is_untyped_null.len() {
            return None;
        }

        let mut coerced_types = Vec::with_capacity(provided.len());

        for (i, func_arg) in arguments.iter().enumerate() {
            let arg_type = if is_untyped_null[i] {
                // For untyped null, infer type from function signature
                func_arg.clone()
            } else {
                // Use original type
                provided[i].clone()
            };

            // Check if the coerced type matches the function signature
            if !matches_type(&arg_type, func_arg) {
                return None;
            }

            coerced_types.push(arg_type);
        }

        Some((return_type.clone(), coerced_types))
    }

    pub fn matches(
        arguments: &[LogicalType],
        return_type: &LogicalType,
        provided: &[LogicalType],
    ) -> Option<LogicalType> {
        if arguments.len() != provided.len() {
            return None;
        }
        for (i, provided) in provided.iter().enumerate() {
            if !matches_type(provided, &arguments[i]) {
                return None;
            }
        }
        Some(return_type.clone())
    }

    fn matches_type(provided: &LogicalType, expected: &LogicalType) -> bool {
        if provided == expected {
            return true;
        }
        if *expected == LogicalType::ANY {
            return true;
        }
        if let Some(inner) = expected.as_list() {
            return provided.is_list() && matches_type(provided.as_list().unwrap(), inner);
        }
        false
    }
}
