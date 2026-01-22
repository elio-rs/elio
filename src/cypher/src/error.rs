use std::backtrace::Backtrace;

use elio_common::data_type::DataType;
use elio_storage::error::GraphStoreError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlanError {
    #[error("parse error")]
    ParserError(String, #[backtrace] Backtrace),
    #[error("meta access error")]
    MetaError(#[from] GraphStoreError, #[backtrace] Backtrace),
    #[error("{}", _0.message)]
    SemanticError(#[from] SemanticError, #[backtrace] Backtrace),
    #[error("{}", _0)]
    NotSupported(String),
    #[error("{}", _0)]
    BadPlan(String),
}

impl PlanError {
    pub fn parse_error<T: ToString>(msg: T) -> Self {
        Self::ParserError(msg.to_string(), Backtrace::capture())
    }

    #[deprecated]
    pub fn semantic_err<T: ToString>(msg: T) -> Self {
        Self::SemanticError(SemanticError::new(msg.to_string()), Backtrace::capture())
    }

    pub fn not_supported<T: ToString>(msg: T) -> Self {
        Self::NotSupported(msg.to_string())
    }

    pub fn bad_plan<T: ToString>(msg: T) -> Self {
        Self::BadPlan(msg.to_string())
    }
}

#[derive(Error, Clone, Debug)]
pub struct SemanticError {
    // TODO(pgao): add gql status here
    pub message: String,
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Expr
impl SemanticError {
    pub fn new(message: String) -> Self {
        Self { message }
    }

    pub fn variable_not_defined(name: &str, ctx: &str) -> Self {
        let msg = format!("Variable {} is not defined in {}", name, ctx);
        Self::new(msg)
    }

    pub fn agg_not_allowed(func: &str, expr: &str) -> Self {
        let msg = format!("Aggregation function {} is not allowed in expression {}", func, expr);
        Self::new(msg)
    }

    pub fn distinct_not_allowed(expr: &str) -> Self {
        let msg = format!("DISTINCT is not allowed in expression {}", expr);
        Self::new(msg)
    }

    pub fn invalid_literal(typ: &DataType, lit: &str) -> Self {
        let msg = format!("Invalid literal {} for type {}", lit, typ);
        Self::new(msg)
    }

    pub fn unknown_function(name: &str, ctx: &str) -> Self {
        let msg = format!("Unknown function {} in {}", name, ctx);
        Self::new(msg)
    }

    pub fn invalid_function_arg_types(func: &str, args: &[DataType], ctx: &str) -> Self {
        let msg = format!(
            "Invalid argument types {} for function {} in {}",
            args.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "),
            func,
            ctx
        );
        Self::new(msg)
    }

    pub fn invalid_filter_expr_type(typ: &DataType, ctx: &str) -> Self {
        let msg = format!("Filter expression must be a boolean, got {} in {}", typ, ctx);
        Self::new(msg)
    }

    pub fn invalid_path_step_type(name: &str, ctx: &str, expected: &str) -> Self {
        let msg = format!("Path step {} must be a {} in {}", name, expected, ctx);
        Self::new(msg)
    }
}

// clause semantics
impl SemanticError {
    pub fn return_item_must_be_aliased(expr: &str, ctx: &str) -> Self {
        let msg = format!("Return item {} must be aliased in {}", expr, ctx);
        Self::new(msg)
    }

    pub fn at_least_one_return_item(ctx: &str) -> Self {
        let msg = format!("At least one return item is required in {}", ctx);
        Self::new(msg)
    }

    pub fn invalid_pagination_offset_type(ctx: &str) -> Self {
        let msg = format!("Pagination offset must be an integer in {}", ctx);
        Self::new(msg)
    }

    pub fn invalid_pagination_limit_type(ctx: &str) -> Self {
        let msg = format!("Pagination limit must be an integer in {}", ctx);
        Self::new(msg)
    }

    pub fn invalid_union(ctx: &str) -> Self {
        let msg = format!("All subqueries in UNION should return same colum names {}", ctx);
        Self::new(msg)
    }
}

// pattern semantics
impl SemanticError {
    pub fn qpp_not_allowed(clause: &str, ctx: &str) -> Self {
        let msg = format!("Quantified path patterns cannot be used in {} at {}", clause, ctx);
        Self::new(msg)
    }

    pub fn invalid_create_entity(ctx: &str) -> Self {
        let msg = format!("Invalid CREATE entity in {}", ctx);
        Self::new(msg)
    }
}

// LOAD clause semantics
impl SemanticError {
    pub fn load_must_be_first(ctx: &str) -> Self {
        let msg = format!("LOAD clause must be the first clause in a query: {}", ctx);
        Self::new(msg)
    }

    pub fn variable_already_defined(name: &str, ctx: &str) -> Self {
        let msg = format!("Variable '{}' is already defined in {}", name, ctx);
        Self::new(msg)
    }

    pub fn unsupported_load_format(format: &str, ctx: &str) -> Self {
        let msg = format!("Unsupported LOAD format '{}' in {}", format, ctx);
        Self::new(msg)
    }

    pub fn invalid_option_type(option: &str, expected: &str, ctx: &str) -> Self {
        let msg = format!("Option '{}' must be a {} in {}", option, expected, ctx);
        Self::new(msg)
    }

    pub fn invalid_option_value(option: &str, expected: &str, ctx: &str) -> Self {
        let msg = format!("Option '{}' must be a {} in {}", option, expected, ctx);
        Self::new(msg)
    }

    pub fn unknown_load_option(option: &str, ctx: &str) -> Self {
        let msg = format!("Unknown LOAD option '{}' in {}", option, ctx);
        Self::new(msg)
    }
}

#[macro_export]
macro_rules! not_supported {
    ($msg:expr) => {
        Err(PlanError::not_supported($msg))
    };
}
