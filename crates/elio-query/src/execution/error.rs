use std::backtrace::Backtrace;
use std::fmt::Display;

use elio_common::data_type::LogicalType;
use elio_storage::error::GraphStoreError;

use crate::execution::builder::BuildError;

#[derive(thiserror::Error, Debug)]
pub enum EvalError {
    #[error("get or create token failed {0}")]
    GetOrCreateTokenError(String),
    #[error("mapb error {0}")]
    MapbError(String),
    #[error("type error {0}")]
    TypeError(String),
    #[error("field not found {0}")]
    FieldNotFound(String),
    #[error("materialize node failed {0}")]
    MaterializeNodeError(String),
    #[error("invalid argument in {context}, expected {expected}, actual {actual}")]
    InvalidArgument {
        context: String,
        expected: String,
        actual: String,
    },
    #[error("arithmetic overflow, op: {op}, args: {args:?}")]
    ArithmeticOverflow { op: String, args: Vec<String> },
}

impl EvalError {
    pub fn get_or_create_token_error(key: &str) -> Self {
        Self::GetOrCreateTokenError(key.to_string())
    }

    pub fn mapb_error(msg: &str) -> Self {
        Self::MapbError(msg.to_string())
    }

    pub fn type_error<T: Display>(msg: T) -> Self {
        Self::TypeError(msg.to_string())
    }

    pub fn field_not_found<T: Display>(msg: T) -> Self {
        Self::FieldNotFound(msg.to_string())
    }

    pub fn materialize_node_error<T: Display>(msg: T) -> Self {
        Self::MaterializeNodeError(msg.to_string())
    }

    pub fn invalid_argument<T1: Display, T2: Display, T3: Display>(context: T1, expected: T2, actual: T3) -> Self {
        Self::InvalidArgument {
            context: context.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    }

    pub fn arithmetic_overflow<T1: Display, T2: Display>(op: T1, args: Vec<T2>) -> Self {
        Self::ArithmeticOverflow {
            op: op.to_string(),
            args: args.into_iter().map(|x| x.to_string()).collect(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ExecError {
    #[error("executor build error: {0}")]
    BuildError(#[from] BuildError, #[backtrace] Backtrace),
    #[error("Store error: {0}")]
    StoreError(#[from] GraphStoreError, #[backtrace] Backtrace),
    #[error("Eval error: {0}")]
    EvalError(#[from] EvalError, #[backtrace] Backtrace),
    #[error("IO error: {0}")]
    IoError(String, #[backtrace] Backtrace),
    #[error("type mismatch in {}, expected {:?}, actual {:?}", 0, 1, 2)]
    TypeMismatch {
        context: String,
        expected: String,
        actual: LogicalType,
        trace: Backtrace,
    },
    #[error("channel error: {0}")]
    ChannelError(String, #[backtrace] Backtrace),
    #[error("constraint violation: {constraint} - {reason}")]
    ConstraintViolation {
        constraint: String,
        reason: String,
        #[backtrace]
        trace: Backtrace,
    },
    #[error("panic: {0}")]
    Panic(String, #[backtrace] Backtrace),
}

impl ExecError {
    pub fn type_mismatch<T1: ToString, T2: ToString>(context: T1, expected: T2, actual: LogicalType) -> Self {
        Self::TypeMismatch {
            context: context.to_string(),
            expected: expected.to_string(),
            actual,
            trace: Backtrace::capture(),
        }
    }

    pub fn io_error<T: ToString>(msg: T) -> Self {
        Self::IoError(msg.to_string(), Backtrace::capture())
    }

    pub fn panic<T: ToString>(msg: T) -> Self {
        Self::Panic(msg.to_string(), Backtrace::capture())
    }
}
