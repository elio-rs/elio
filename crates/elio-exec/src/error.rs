use std::backtrace::Backtrace;

use elio_common::array::PhysicalType;
use elio_expr::error::EvalError;
use elio_storage::error::GraphStoreError;

use crate::builder::BuildError;

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
        actual: PhysicalType,
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
    pub fn type_mismatch<T1: ToString, T2: ToString>(context: T1, expected: T2, actual: PhysicalType) -> Self {
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
